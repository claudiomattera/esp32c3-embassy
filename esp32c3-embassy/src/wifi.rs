// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Functions and task for WiFi connection

use log::{debug, error, info};

use embassy_executor::Spawner;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

use esp_wifi::{
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiError as EspWifiError,
        WifiEvent, WifiStaDevice, WifiState,
    },
    EspWifiInitFor, InitializationError as WifiInitializationError,
};

use embassy_net::{Config, DhcpConfig, Stack, StackResources};
use embassy_time::{Duration, Timer};

use esp_hal::{
    clock::Clocks,
    peripherals::{SYSTIMER, WIFI},
    system::RadioClockControl,
    systimer::SystemTimer,
    Rng,
};

use heapless::String;

use static_cell::StaticCell;

use rand_core::RngCore as _;

use crate::RngWrapper;

/// Static cell for network stack resources
static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();

/// Static cell for network stack
static STACK: StaticCell<Stack<WifiDevice<'static, WifiStaDevice>>> = StaticCell::new();

/// Signal to request to stop WiFi
pub static STOP_WIFI_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Connect to WiFi
pub async fn connect(
    spawner: &Spawner,
    systimer: SYSTIMER,
    rng: Rng,
    wifi: WIFI,
    radio_clock_control: RadioClockControl,
    clocks: &Clocks<'_>,
    (ssid, password): (String<32>, String<64>),
) -> Result<&'static Stack<WifiDevice<'static, WifiStaDevice>>, Error> {
    let mut rng_wrapper = RngWrapper::from(rng);
    let seed = rng_wrapper.next_u64();
    debug!("Use random seed 0x{seed:016x}");

    let init = esp_wifi::initialize(
        EspWifiInitFor::Wifi,
        SystemTimer::new(systimer).alarm0,
        rng,
        radio_clock_control,
        clocks,
    )?;

    let (wifi_interface, controller) = esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice)?;

    let config = Config::dhcpv4(DhcpConfig::default());

    debug!("Initialize network stack");
    let stack_resources: &'static mut _ = STACK_RESOURCES.init(StackResources::new());
    let stack: &'static mut _ =
        STACK.init(Stack::new(wifi_interface, config, stack_resources, seed));

    spawner.must_spawn(connection(controller, ssid, password));
    spawner.must_spawn(net_task(stack));

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
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await;
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
    debug!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        if esp_wifi::wifi::get_wifi_state() == WifiState::StaConnected {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await;
        }

        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: ssid.clone(),
                password: password.clone(),
                ..Default::default()
            });
            controller.set_configuration(&client_config)?;
            debug!("Starting WiFi controller");
            controller.start().await?;
            debug!("WiFi controller started");
        }

        debug!("Connect to WiFi network");

        match controller.connect().await {
            Ok(()) => {
                debug!("Connected to WiFi network");

                debug!("Wait for request to stop wifi");
                STOP_WIFI_SIGNAL.wait().await;
                info!("Received signal to stop wifi");
                controller.stop().await?;
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
    WifiInitialization(WifiInitializationError),

    /// Error during WiFi operation
    Wifi(EspWifiError),
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
