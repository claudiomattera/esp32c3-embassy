// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Main crate

#![no_std]
#![no_main]

use core::convert::Infallible;

use log::error;
use log::info;

use embassy_executor::Spawner;

use embassy_time::Delay;
use embassy_time::Duration;
use embassy_time::Timer;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;

use esp_alloc::heap_allocator;

use esp_hal::clock::CpuClock;
use esp_hal::dma::Dma;
use esp_hal::dma::DmaBufError;
use esp_hal::dma::DmaDescriptor;
use esp_hal::dma::DmaPriority;
use esp_hal::dma::DmaRxBuf;
use esp_hal::dma::DmaTxBuf;
use esp_hal::gpio::Input;
use esp_hal::gpio::Level;
use esp_hal::gpio::Output;
use esp_hal::gpio::Pull;
use esp_hal::i2c::master::Config as I2cConfig;
use esp_hal::i2c::master::I2c;
use esp_hal::init as initialize_esp_hal;
use esp_hal::prelude::*; // RateExtU32, main, ram
use esp_hal::rng::Rng;
use esp_hal::spi::master::Config as SpiConfig;
use esp_hal::spi::master::Spi;
use esp_hal::spi::master::SpiDma;
use esp_hal::spi::SpiMode;
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::timer::systimer::Target;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::Async;
use esp_hal::Config as EspConfig;

use esp_hal_embassy::init as initialize_embassy;

use time::OffsetDateTime;

use heapless::HistoryBuffer;
use heapless::String;

use embedded_hal_bus::spi::ExclusiveDevice;

use esp_backtrace as _;

use static_cell::StaticCell;

mod logging;
use self::logging::setup as setup_logging;

mod sensor;
use self::sensor::sample_task as sample_sensor_task;

mod dashboard;

mod display;
use self::display::update_task as update_display_task;

mod cell;
use self::cell::SyncUnsafeCell;

mod clock;
use self::clock::Clock;
use self::clock::Error as ClockError;

mod http;
use self::http::Client as HttpClient;

mod domain;
use self::domain::Reading;
use self::domain::Sample;

mod random;
use self::random::RngWrapper;

mod sleep;
use self::sleep::enter_deep as enter_deep_sleep;

mod wifi;
use self::wifi::connect as connect_to_wifi;
use self::wifi::Error as WifiError;
use self::wifi::STOP_WIFI_SIGNAL;

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

/// Size of heap for dynamically-allocated memory
const HEAP_MEMORY_SIZE: usize = 72 * 1024;

/// A channel between sensor sampler and display updater
static CHANNEL: StaticCell<Channel<NoopRawMutex, Reading, 3>> = StaticCell::new();

/// Size of SPI DMA descriptors
const DESCRIPTORS_SIZE: usize = 8 * 3;

/// Descriptors for SPI DMA
static DESCRIPTORS: StaticCell<[DmaDescriptor; DESCRIPTORS_SIZE]> = StaticCell::new();

/// RX descriptors for SPI DMA
static RX_DESCRIPTORS: StaticCell<[DmaDescriptor; DESCRIPTORS_SIZE]> = StaticCell::new();

/// Size of SPI DMA buffers
const BUFFERS_SIZE: usize = 8 * 3;

/// Buffer for SPI DMA
static BUFFER: StaticCell<[u8; BUFFERS_SIZE]> = StaticCell::new();

/// RX Buffer for SPI DMA
static RX_BUFFER: StaticCell<[u8; BUFFERS_SIZE]> = StaticCell::new();

/// Stored boot count between deep sleep cycles
///
/// This is a statically allocated variable and it is placed in the RTC Fast
/// memory, which survives deep sleep.
#[ram(rtc_fast)]
static BOOT_COUNT: SyncUnsafeCell<u32> = SyncUnsafeCell::new(0);

/// Stored history between deep sleep cycles
///
/// This is a statically allocated variable and it is placed in the RTC Fast
/// memory, which survives deep sleep.
#[ram(rtc_fast)]
static HISTORY: SyncUnsafeCell<HistoryBuffer<(OffsetDateTime, Sample), 96>> =
    SyncUnsafeCell::new(HistoryBuffer::new());

/// Main task
#[main]
async fn main(spawner: Spawner) {
    setup_logging();

    // SAFETY:
    // This is the only place where a mutable reference is taken
    let boot_count: Option<&'static mut _> = unsafe { BOOT_COUNT.get().as_mut() };
    // SAFETY:
    // This is pointing to a valid value
    let boot_count: &'static mut _ = unsafe { boot_count.unwrap_unchecked() };
    info!("Current boot count = {boot_count}");
    *boot_count += 1;

    // SAFETY:
    // This is the only place where a mutable reference is taken
    let history: Option<&'static mut _> = unsafe { HISTORY.get().as_mut() };
    // SAFETY:
    // This is pointing to a valid value
    let history: &'static mut _ = unsafe { history.unwrap_unchecked() };

    if let Err(error) = main_fallible(&spawner, history).await {
        error!("Error while running firmware: {error:?}");
    }
}

/// Main task that can return an error
async fn main_fallible(
    spawner: &Spawner,
    history: &'static mut HistoryBuffer<(OffsetDateTime, Sample), 96>,
) -> Result<(), Error> {
    let peripherals = initialize_esp_hal({
        let mut config = EspConfig::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    heap_allocator!(HEAP_MEMORY_SIZE);

    let systimer = SystemTimer::new(peripherals.SYSTIMER).split::<Target>();
    initialize_embassy(systimer.alarm0);

    let rng = Rng::new(peripherals.RNG);

    let clock = if let Some(clock) = Clock::from_rtc_memory() {
        info!("Clock loaded from RTC memory");
        clock
    } else {
        let ssid = String::<32>::try_from(WIFI_SSID).map_err(|()| Error::ParseCredentials)?;
        let password =
            String::<64>::try_from(WIFI_PASSWORD).map_err(|()| Error::ParseCredentials)?;

        info!("Connect to WiFi");
        let timg0 = TimerGroup::new(peripherals.TIMG0);
        let stack = connect_to_wifi(
            spawner,
            timg0,
            rng,
            peripherals.WIFI,
            peripherals.RADIO_CLK,
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

    info!("Turn off cold LED");
    let mut cold_led = Output::new(peripherals.GPIO18, Level::High);
    cold_led.set_low();

    info!("Create I²C bus");
    let sda = peripherals.GPIO1;
    let scl = peripherals.GPIO2;

    let i2c_config = I2cConfig {
        frequency: 25_u32.kHz(),
        ..Default::default()
    };
    let i2c = I2c::new(peripherals.I2C0, i2c_config)
        .with_sda(sda)
        .with_scl(scl)
        .into_async();

    info!("Create SPI bus");
    let spi_config = SpiConfig {
        frequency: 25_u32.kHz(),
        mode: SpiMode::Mode0,
        ..Default::default()
    };
    let spi_bus = Spi::new_with_config(peripherals.SPI2, spi_config)
        .with_sck(peripherals.GPIO6)
        .with_mosi(peripherals.GPIO7)
        .into_async();

    info!("Wrap SPI bus in a SPI DMA");
    let descriptors: &'static mut _ = DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);
    let rx_descriptors: &'static mut _ =
        RX_DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);

    let buffer: &'static mut _ = BUFFER.init([0; BUFFERS_SIZE]);
    let rx_buffer: &'static mut _ = RX_BUFFER.init([0; BUFFERS_SIZE]);

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.channel0.configure(false, DmaPriority::Priority0);

    let spi_dma: SpiDma<'_, Async> = spi_bus.with_dma(dma_channel);

    let tx_buffers = DmaTxBuf::new(descriptors, buffer)?;
    let rx_buffers = DmaRxBuf::new(rx_descriptors, rx_buffer)?;
    let spi_dma_bus = spi_dma.with_buffers(rx_buffers, tx_buffers);

    info!("Create PIN for SPI Chip Select");
    let cs = Output::new(peripherals.GPIO8, Level::High);

    info!("Create additional PINs");
    let busy = Input::new(peripherals.GPIO9, Pull::Up);
    let rst = Output::new(peripherals.GPIO10, Level::Low);
    let dc = Output::new(peripherals.GPIO19, Level::Low);

    info!("Create SPI device");
    let spi_device = ExclusiveDevice::new(spi_dma_bus, cs, Delay);

    info!("Create channel");
    let channel: &'static mut _ = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();

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
    enter_deep_sleep(peripherals.LPWR, DEEP_SLEEP_DURATION.into());
}

/// An error
#[derive(Debug)]
enum Error {
    /// An impossible error existing only to satisfy the type system
    Impossible(Infallible),

    /// Error while parsing SSID or password
    ParseCredentials,

    /// An error within WiFi operations
    #[expect(unused, reason = "Never read directly")]
    Wifi(WifiError),

    /// An error within clock operations
    #[expect(unused, reason = "Never read directly")]
    Clock(ClockError),

    /// An error within creation of DMA buffers
    #[expect(unused, reason = "Never read directly")]
    DmaBuffer(DmaBufError),
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

impl From<DmaBufError> for Error {
    fn from(error: DmaBufError) -> Self {
        Self::DmaBuffer(error)
    }
}
