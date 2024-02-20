// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Main crate

#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(static_mut_refs)]

use core::convert::Infallible;

use log::{error, info};

use embassy_executor::Spawner;

use embassy_time::{Delay, Duration, Timer};

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Channel};

use esp_hal::{
    clock::ClockControl,
    dma::{Channel0, Dma, DmaDescriptor, DmaPriority},
    embassy,
    i2c::I2C,
    peripherals::{Peripherals, SPI2},
    prelude::{_esp_hal_system_SystemExt, _fugit_RateExtU32, entry, main, ram},
    spi::{
        master::{
            dma::{SpiDma, WithDmaSpi2},
            Spi,
        },
        FullDuplexMode, SpiMode,
    },
    timer::TimerGroup,
    Delay as EspDelay, Rng, IO,
};

use time::OffsetDateTime;

use heapless::{HistoryBuffer, String};

use embedded_hal_bus::spi::ExclusiveDevice;

use embedded_hal::digital::OutputPin;

use esp_backtrace as _;

use static_cell::StaticCell;

mod logging;
use self::logging::setup as setup_logging;

mod sensor;
use self::sensor::sample_task as sample_sensor_task;

mod dashboard;

mod display;
use self::display::update_task as update_display_task;

mod clock;
use self::clock::{Clock, Error as ClockError};

mod http;
use self::http::Client as HttpClient;

mod domain;
use self::domain::{Reading, Sample};

mod random;
use self::random::RngWrapper;

mod sleep;
use self::sleep::enter_deep as enter_deep_sleep;

mod wifi;
use self::wifi::{connect as connect_to_wifi, Error as WifiError, STOP_WIFI_SIGNAL};

mod worldtimeapi;

/// Period to wait between readings
const SAMPLING_PERIOD: Duration = Duration::from_secs(60);

/// Duration of deep sleep
const DEEP_SLEEP_DURATION: Duration = Duration::from_secs(300);

/// Period to wait before going to deep sleep
const AWAKE_PERIOD: Duration = Duration::from_secs(300);

/// SSID for WiFi network
const WIFI_SSID: &str = env!("WIFI_SSID");

/// Password for WiFi network
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

/// A channel between sensor sampler and display updater
static CHANNEL: StaticCell<Channel<NoopRawMutex, Reading, 3>> = StaticCell::new();

/// Size of SPI DMA descriptors
const DESCRIPTORS_SIZE: usize = 8 * 3;

/// Descriptors for SPI DMA
static DESCRIPTORS: StaticCell<[DmaDescriptor; DESCRIPTORS_SIZE]> = StaticCell::new();

/// RX descriptors for SPI DMA
static RX_DESCRIPTORS: StaticCell<[DmaDescriptor; DESCRIPTORS_SIZE]> = StaticCell::new();

/// Stored boot count between deep sleep cycles
///
/// This is a statically allocated variable and it is placed in the RTC Fast
/// memory, which survives deep sleep.
#[ram(rtc_fast)]
static mut BOOT_COUNT: u32 = 0;

/// Stored history between deep sleep cycles
///
/// This is a statically allocated variable and it is placed in the RTC Fast
/// memory, which survives deep sleep.
#[ram(rtc_fast)]
static mut HISTORY: HistoryBuffer<(OffsetDateTime, Sample), 96> = HistoryBuffer::new();

/// Main task
#[main]
async fn main(spawner: Spawner) {
    setup_logging();

    // SAFETY:
    // There is only one thread
    let boot_count = unsafe { &mut BOOT_COUNT };
    info!("Current boot count = {boot_count}");
    *boot_count += 1;

    if let Err(error) = main_fallible(&spawner).await {
        error!("Error while running firmware: {error:?}");
    }
}

/// Main task that can return an error
async fn main_fallible(spawner: &Spawner) -> Result<(), Error> {
    let peripherals = Peripherals::take();
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::max(system.clock_control).freeze();
    embassy::init(&clocks, TimerGroup::new(peripherals.TIMG0, &clocks));

    let rng = Rng::new(peripherals.RNG);

    let clock = if let Some(clock) = Clock::from_rtc_memory() {
        info!("Clock loaded from RTC memory");
        clock
    } else {
        let ssid = String::<32>::try_from(WIFI_SSID).map_err(|()| Error::ParseCredentials)?;
        let password =
            String::<64>::try_from(WIFI_PASSWORD).map_err(|()| Error::ParseCredentials)?;

        info!("Connect to WiFi");
        let stack = connect_to_wifi(
            spawner,
            peripherals.SYSTIMER,
            rng,
            peripherals.WIFI,
            system.radio_clock_control,
            &clocks,
            (ssid, password),
        )
        .await?;

        info!("Synchronize clock from server");
        let mut http_client = HttpClient::new(stack, RngWrapper::from(rng));
        let clock = Clock::from_server(&mut http_client).await?;

        info!("Request to disconnect wifi");
        STOP_WIFI_SIGNAL.signal(());

        clock
    };

    info!("Now is {}", clock.now()?);

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);

    info!("Turn off cold LED");
    let mut cold_led = io.pins.gpio18.into_push_pull_output();
    cold_led.set_low()?;

    info!("Create I²C bus");
    let sda = io.pins.gpio1;
    let scl = io.pins.gpio2;

    let i2c = I2C::new(peripherals.I2C0, sda, scl, 25_u32.kHz(), &clocks);

    info!("Create SPI bus");
    let spi_bus = Spi::new(peripherals.SPI2, 25_u32.kHz(), SpiMode::Mode0, &clocks)
        .with_sck(io.pins.gpio6)
        .with_mosi(io.pins.gpio7);

    info!("Wrap SPI bus in a SPI DMA");
    let descriptors: &'static mut _ = DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);
    let rx_descriptors: &'static mut _ =
        RX_DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.channel0;

    let spi_dma: SpiDma<'_, SPI2, Channel0, FullDuplexMode> = spi_bus.with_dma(
        dma_channel.configure(false, descriptors, rx_descriptors, DmaPriority::Priority0),
    );

    info!("Create PIN for SPI Chip Select");
    let cs = io.pins.gpio8.into_push_pull_output();

    info!("Create additional PINs");
    let busy = io.pins.gpio9.into_pull_up_input();
    let rst = io.pins.gpio10.into_push_pull_output();
    let dc = io.pins.gpio19.into_push_pull_output();

    info!("Create SPI device");
    let spi_device = ExclusiveDevice::new(spi_dma, cs, Delay);

    info!("Create channel");
    let channel: &'static mut _ = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();

    // SAFETY:
    // There is only one thread
    let history = unsafe { &mut HISTORY };
    info!("History contains {} elements", history.len());

    info!("Spawn tasks");
    spawner.must_spawn(sample_sensor_task(
        i2c,
        rng,
        sender,
        clock.clone(),
        SAMPLING_PERIOD,
    ));
    spawner.must_spawn(update_display_task(
        spi_device, busy, rst, dc, receiver, history,
    ));

    info!("Stay awake for {}s", AWAKE_PERIOD.as_secs());
    Timer::after(AWAKE_PERIOD).await;

    clock.save_to_rtc_memory(DEEP_SLEEP_DURATION);
    enter_deep_sleep(
        peripherals.LPWR,
        EspDelay::new(&clocks),
        DEEP_SLEEP_DURATION.into(),
    );
}

/// An error
#[derive(Debug)]
enum Error {
    /// An impossible error existing only to satisfy the type system
    Impossible(Infallible),

    /// Error while parsing SSID or password
    ParseCredentials,

    /// An error within WiFi operations
    #[allow(unused)]
    Wifi(WifiError),

    /// An error within clock operations
    #[allow(unused)]
    Clock(ClockError),
}

impl From<Infallible> for Error {
    fn from(error: Infallible) -> Self {
        Self::Impossible(error)
    }
}

impl From<WifiError> for Error {
    fn from(error: WifiError) -> Self {
        Self::Wifi(error)
    }
}

impl From<ClockError> for Error {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}
