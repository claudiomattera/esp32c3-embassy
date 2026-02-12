// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Functions and task for WiFi connection

use alloc::string::ToString as _;

use log::debug;
use log::error;
use log::info;

use embassy_executor::Spawner;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use esp_radio::init as initialize_wifi;
use esp_radio::wifi::new as new_wifi;
use esp_radio::wifi::sta_state;
use esp_radio::wifi::ClientConfig;
use esp_radio::wifi::Config as WifiConfig;
use esp_radio::wifi::ModeConfig;
use esp_radio::wifi::WifiController;
use esp_radio::wifi::WifiDevice;
use esp_radio::wifi::WifiError as EspWifiError;
use esp_radio::wifi::WifiEvent;
use esp_radio::wifi::WifiStaState;
use esp_radio::Controller;
use esp_radio::InitializationError as WifiInitializationError;

use embassy_net::new as new_network_stack;
use embassy_net::Config;
use embassy_net::DhcpConfig;
use embassy_net::Runner;
use embassy_net::Stack;
use embassy_net::StackResources;

use embassy_time::Duration;
use embassy_time::Timer;

use esp_hal::peripherals::WIFI;
use esp_hal::rng::Rng;

use heapless::String;

use static_cell::StaticCell;

use rand_core::Rng as _;

use crate::RngWrapper;

/// Static cell for network stack resources
static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

/// Static cell for WiFi controller
static RADIO_CONTROLLER: StaticCell<Controller<'static>> = StaticCell::new();

/// Signal to request to stop WiFi
pub static STOP_WIFI_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Connect to WiFi
pub async fn connect(
    spawner: Spawner,
    rng: Rng,
    wifi: WIFI<'static>,
    (ssid, password): (String<32>, String<64>),
) -> Result<Stack<'static>, Error> {
    let mut rng_wrapper = RngWrapper::from(rng);
    let seed = rng_wrapper.next_u64();
    debug!("Use random seed 0x{seed:016x}");

    let radio_controller: &'static mut _ = RADIO_CONTROLLER.init(initialize_wifi()?);

    let (controller, interfaces) = new_wifi(radio_controller, wifi, WifiConfig::default())?;
    let wifi_interface = interfaces.sta;

    let config = Config::dhcpv4(DhcpConfig::default());

    debug!("Initialize network stack");
    let stack_resources: &'static mut _ = STACK_RESOURCES.init(StackResources::new());
    let (stack, runner) = new_network_stack(wifi_interface, config, stack_resources, seed);

    spawner.must_spawn(connection(controller, ssid, password));
    spawner.must_spawn(net_task(runner));

    debug!("Wait for network link");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    debug!("Wait for IP address");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Connected to WiFi with IP address {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    Ok(stack)
}

/// Task for ongoing network processing
#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await;
}

/// Task for WiFi connection
///
/// This will wrap [`connection_fallible()`] and trap any error.
#[embassy_executor::task]
async fn connection(controller: WifiController<'static>, ssid: String<32>, password: String<64>) {
    if let Err(error) = connection_fallible(controller, ssid, password).await {
        error!("Cannot connect to WiFi: {error:?}");
    }
}

/// Fallible task for WiFi connection
async fn connection_fallible(
    mut controller: WifiController<'static>,
    ssid: String<32>,
    password: String<64>,
) -> Result<(), Error> {
    debug!("Start connection");
    debug!("Device capabilities: {:?}", controller.capabilities());
    loop {
        if let WifiStaState::Connected = sta_state() {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await;
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = ModeConfig::Client(
                ClientConfig::default()
                    .with_ssid(ssid.to_string())
                    .with_password(password.to_string()),
            );
            controller.set_config(&client_config)?;

            debug!("Starting WiFi controller");
            controller.start_async().await?;
            debug!("WiFi controller started");
        }

        debug!("Connect to WiFi network");

        match controller.connect_async().await {
            Ok(()) => {
                debug!("Connected to WiFi network");

                debug!("Wait for request to stop wifi");
                STOP_WIFI_SIGNAL.wait().await;
                info!("Received signal to stop wifi");
                controller.stop_async().await?;
                break;
            }
            Err(error) => {
                error!("Failed to connect to WiFi network: {error:?}");
                Timer::after(Duration::from_millis(5000)).await;
            }
        }
    }

    info!("Leave connection task");
    Ok(())
}

/// Error within WiFi connection
#[derive(Debug)]
pub enum Error {
    /// Error during WiFi initialization
    WifiInitialization(#[expect(unused, reason = "Never read directly")] WifiInitializationError),

    /// Error during WiFi operation
    Wifi(#[expect(unused, reason = "Never read directly")] EspWifiError),
}

impl From<WifiInitializationError> for Error {
    fn from(error: WifiInitializationError) -> Self {
        Self::WifiInitialization(error)
    }
}

impl From<EspWifiError> for Error {
    fn from(error: EspWifiError) -> Self {
        Self::Wifi(error)
    }
}
