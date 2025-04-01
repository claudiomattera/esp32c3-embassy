// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

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
use embassy_sync::channel::Sender;

use esp_alloc::heap_allocator;

use esp_hal::clock::CpuClock;
use esp_hal::dma::DmaBufError;
use esp_hal::dma::DmaChannel0;
use esp_hal::dma::DmaDescriptor;
use esp_hal::dma::DmaRxBuf;
use esp_hal::dma::DmaTxBuf;
use esp_hal::gpio::GpioPin;
use esp_hal::gpio::Input;
use esp_hal::gpio::InputConfig;
use esp_hal::gpio::Level;
use esp_hal::gpio::Output;
use esp_hal::gpio::OutputConfig;
use esp_hal::gpio::Pull;
use esp_hal::i2c::master::Config as I2cConfig;
use esp_hal::i2c::master::ConfigError as I2cConfigError;
use esp_hal::i2c::master::I2c;
use esp_hal::init as initialize_esp_hal;
use esp_hal::peripherals::I2C0;
use esp_hal::peripherals::RADIO_CLK;
use esp_hal::peripherals::SPI2;
use esp_hal::peripherals::TIMG0;
use esp_hal::peripherals::WIFI;
use esp_hal::ram;
use esp_hal::rng::Rng;
use esp_hal::spi::master::Config as SpiConfig;
use esp_hal::spi::master::ConfigError as SpiConfigError;
use esp_hal::spi::master::Spi;
use esp_hal::spi::master::SpiDma;
use esp_hal::spi::Mode as SpiMode;
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::Async;
use esp_hal::Blocking;
use esp_hal::Config as EspConfig;

use esp_hal_embassy::init as initialize_embassy;
use esp_hal_embassy::main;

use time::OffsetDateTime;

use heapless::HistoryBuffer;
use heapless::String;

use embedded_hal_bus::spi::ExclusiveDevice;

use esp_backtrace as _;

use static_cell::StaticCell;

mod adafruitio;

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

    if let Err(error) = main_fallible(spawner, history).await {
        error!("Error while running firmware: {error:?}");
    }
}

/// Main task that can return an error
async fn main_fallible(
    spawner: Spawner,
    history: &'static mut HistoryBuffer<(OffsetDateTime, Sample), 96>,
) -> Result<(), Error> {
    let peripherals = initialize_esp_hal(EspConfig::default().with_cpu_clock(CpuClock::max()));

    heap_allocator!(size: HEAP_MEMORY_SIZE);

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    initialize_embassy(timg1.timer0);

    let rng = Rng::new(peripherals.RNG);

    let clock = load_clock(
        spawner,
        peripherals.TIMG0,
        peripherals.WIFI,
        peripherals.RADIO_CLK,
        rng,
    )
    .await?;

    info!("Now is {}", clock.now()?);

    info!("Turn off cold LED");
    let mut cold_led = Output::new(peripherals.GPIO18, Level::High, OutputConfig::default());
    cold_led.set_low();

    info!("History contains {} elements", history.len());

    info!("Setup display task");
    let sender = setup_display_task(
        spawner,
        DisplayPeripherals {
            sclk: peripherals.GPIO6,
            mosi: peripherals.GPIO7,
            cs: peripherals.GPIO8,
            busy: peripherals.GPIO9,
            rst: peripherals.GPIO10,
            dc: peripherals.GPIO19,
            spi2: peripherals.SPI2,
            dma: peripherals.DMA_CH0,
        },
        history,
    )?;

    info!("Setup sensor task");
    setup_sensor_task(
        spawner,
        SensorPeripherals {
            sda: peripherals.GPIO1,
            scl: peripherals.GPIO2,
            i2c0: peripherals.I2C0,
            rng,
        },
        clock.clone(),
        sender,
    )?;

    info!("Stay awake for {}s", AWAKE_PERIOD.as_secs());
    Timer::after(AWAKE_PERIOD).await;

    clock.save_to_rtc_memory(DEEP_SLEEP_DURATION);
    enter_deep_sleep(peripherals.LPWR, DEEP_SLEEP_DURATION.into());
}

/// Load clock from RTC memory of from server
async fn load_clock(
    spawner: Spawner,
    timg0: TIMG0,
    wifi: WIFI,
    radio_clk: RADIO_CLK,
    rng: Rng,
) -> Result<Clock, Error> {
    let clock = if let Some(clock) = Clock::from_rtc_memory() {
        info!("Clock loaded from RTC memory");
        clock
    } else {
        let ssid = String::<32>::try_from(WIFI_SSID).map_err(|()| Error::ParseCredentials)?;
        let password =
            String::<64>::try_from(WIFI_PASSWORD).map_err(|()| Error::ParseCredentials)?;

        info!("Connect to WiFi");
        let timg0 = TimerGroup::new(timg0);
        let stack = connect_to_wifi(spawner, timg0, rng, wifi, radio_clk, (ssid, password)).await?;

        info!("Synchronize clock from server");
        let mut http_client = HttpClient::new(stack, RngWrapper::from(rng));
        let clock = Clock::from_server(&mut http_client).await?;

        info!("Request to disconnect wifi");
        STOP_WIFI_SIGNAL.signal(());

        clock
    };

    Ok(clock)
}

/// Peripherals used by the display
struct DisplayPeripherals {
    /// SPI sclk
    sclk: GpioPin<6>,

    /// SPI mosi
    mosi: GpioPin<7>,

    /// SPI chip selection
    cs: GpioPin<8>,

    /// Busy signal
    busy: GpioPin<9>,

    /// Reset signal
    rst: GpioPin<10>,

    /// Command signal
    dc: GpioPin<19>,

    /// SPI interface
    spi2: SPI2,

    /// DMA channel
    dma: DmaChannel0,
}

/// Setup display task
fn setup_display_task(
    spawner: Spawner,
    peripherals: DisplayPeripherals,
    history: &'static mut HistoryBuffer<(OffsetDateTime, Sample), 96>,
) -> Result<Sender<'static, NoopRawMutex, (OffsetDateTime, Sample), 3>, Error> {
    info!("Create SPI bus");
    let spi_config = SpiConfig::default()
        .with_frequency(Rate::from_khz(25_u32))
        .with_mode(SpiMode::_0);
    let spi_bus = Spi::new(peripherals.spi2, spi_config)?
        .with_sck(peripherals.sclk)
        .with_mosi(peripherals.mosi);

    info!("Wrap SPI bus in a SPI DMA");
    let descriptors: &'static mut _ = DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);
    let rx_descriptors: &'static mut _ =
        RX_DESCRIPTORS.init([DmaDescriptor::EMPTY; DESCRIPTORS_SIZE]);

    let buffer: &'static mut _ = BUFFER.init([0; BUFFERS_SIZE]);
    let rx_buffer: &'static mut _ = RX_BUFFER.init([0; BUFFERS_SIZE]);

    let spi_dma: SpiDma<'_, Blocking> = spi_bus.with_dma(peripherals.dma);
    let spi_dma: SpiDma<'_, Async> = spi_dma.into_async();

    let tx_buffers = DmaTxBuf::new(descriptors, buffer)?;
    let rx_buffers = DmaRxBuf::new(rx_descriptors, rx_buffer)?;
    let spi_dma_bus = spi_dma.with_buffers(rx_buffers, tx_buffers);

    info!("Create PIN for SPI Chip Select");
    let cs = Output::new(peripherals.cs, Level::High, OutputConfig::default());

    info!("Create additional PINs");
    let busy = Input::new(peripherals.busy, InputConfig::default().with_pull(Pull::Up));
    let rst = Output::new(peripherals.rst, Level::Low, OutputConfig::default());
    let dc = Output::new(peripherals.dc, Level::Low, OutputConfig::default());

    info!("Create SPI device");
    let spi_device = ExclusiveDevice::new(spi_dma_bus, cs, Delay)?;

    info!("Create channel");
    let channel: &'static mut _ = CHANNEL.init(Channel::new());
    let receiver = channel.receiver();
    let sender = channel.sender();

    info!("Spawn tasks");
    spawner.must_spawn(update_display_task(
        spi_device, busy, rst, dc, receiver, history,
    ));

    Ok(sender)
}

/// Peripherals used by the sensor
struct SensorPeripherals {
    /// I²C SDA pin
    sda: GpioPin<1>,

    /// I²C SCL pin
    scl: GpioPin<2>,

    /// I²C interface
    i2c0: I2C0,

    /// Random number generator
    rng: Rng,
}

/// Setup sensor task
fn setup_sensor_task(
    spawner: Spawner,
    peripherals: SensorPeripherals,
    clock: Clock,
    sender: Sender<'static, NoopRawMutex, (OffsetDateTime, Sample), 3>,
) -> Result<(), Error> {
    info!("Create I²C bus");
    let i2c_config = I2cConfig::default().with_frequency(Rate::from_khz(25_u32));
    let i2c = I2c::new(peripherals.i2c0, i2c_config)?
        .with_sda(peripherals.sda)
        .with_scl(peripherals.scl)
        .into_async();

    spawner.must_spawn(sample_sensor_task(
        i2c,
        peripherals.rng,
        sender,
        clock,
        SAMPLING_PERIOD,
    ));

    Ok(())
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

    /// An error within creation of SPI bus
    SpiConfig(SpiConfigError),

    /// An error within creation of I²C bus
    #[expect(unused, reason = "Never read directly")]
    I2cConfig(I2cConfigError),
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

impl From<SpiConfigError> for Error {
    fn from(error: SpiConfigError) -> Self {
        Self::SpiConfig(error)
    }
}

impl From<I2cConfigError> for Error {
    fn from(error: I2cConfigError) -> Self {
        Self::I2cConfig(error)
    }
}
