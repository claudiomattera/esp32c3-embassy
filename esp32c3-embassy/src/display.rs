// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Task for reporting sensor value on a WaveShare E-INK display

use log::error;
use log::info;

use embassy_time::Delay;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Receiver;

use embedded_hal_bus::spi::ExclusiveDevice;

use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::OutputPin;

use time::OffsetDateTime;

use esp_hal::gpio::Input;
use esp_hal::gpio::Output;
use esp_hal::spi::master::SpiDmaBus;
use esp_hal::Async;

use heapless::HistoryBuffer;

use uom::si::pressure::hectopascal;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature::degree_celsius;

use waveshare_154bv2_rs::AsyncDisplay;
use waveshare_154bv2_rs::Buffer;
use waveshare_154bv2_rs::Error as DisplayError;

use crate::dashboard::draw as draw_dashboard;
use crate::dashboard::Error as DashboardError;
use crate::domain::Reading;
use crate::domain::Sample;

/// Task for displaying samples
#[embassy_executor::task]
pub async fn update_task(
    spi_device: ExclusiveDevice<SpiDmaBus<'static, Async>, Output<'static>, Delay>,
    busy: Input<'static>,
    rst: Output<'static>,
    dc: Output<'static>,
    receiver: Receiver<'static, NoopRawMutex, Reading, 3>,
    history: &'static mut HistoryBuffer<(OffsetDateTime, Sample), 96>,
) {
    info!("Create display");
    let mut display = AsyncDisplay::new_with_individual_writes(spi_device, busy, rst, dc, Delay);

    info!("Initialize display");
    if let Err(error) = display.initialize().await {
        error!(" Cannot initialize display: {error:?}");
        return;
    }

    loop {
        info!("Wait for message from sensor");
        let reading = receiver.receive().await;
        let now = reading.0;

        history.write(reading);

        if let Err(error) = report(&now, history, &mut display).await {
            error!("Could not report sample: {error:?}");
        }
    }
}

/// Report a new sample
async fn report<SPI, BUSY, RST, DC, DELAY>(
    now: &OffsetDateTime,
    history: &HistoryBuffer<Reading, 96>,
    display: &mut AsyncDisplay<SPI, BUSY, RST, DC, DELAY>,
) -> Result<(), ReportError>
where
    SPI: SpiDevice,
    BUSY: Wait,
    RST: OutputPin,
    DC: OutputPin,
    DELAY: DelayNs,
{
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "Allow to avoid complicate match expression"
    )]
    if let Some((_, sample)) = history.recent() {
        log_sample(sample);

        let mut buffer = Buffer::new();

        info!("Draw dashboard on buffer");
        draw_dashboard(&mut buffer, now, sample)?;

        info!("Draw buffer on display");
        display.draw_buffer(&buffer).await?;
    }

    Ok(())
}

/// Print a sample to log
fn log_sample(sample: &Sample) {
    let temperature = sample.temperature.get::<degree_celsius>();
    let humidity = sample.humidity.get::<percent>();
    let pressure = sample.pressure.get::<hectopascal>();

    info!("Received sample");
    info!(" ┣ Temperature: {temperature:.2} C");
    info!(" ┣ Humidity:    {humidity:.2} %");
    info!(" ┗ Pressure:    {pressure:.2} hPa");
}

/// An error
#[derive(Debug)]
enum ReportError {
    /// An error occurred while updating the display
    Display(#[expect(unused, reason = "Never read directly")] DisplayError),

    /// An error occurred while drawing the dashboard
    Dashboard(DashboardError),
}

impl From<DisplayError> for ReportError {
    fn from(error: DisplayError) -> Self {
        Self::Display(error)
    }
}

impl From<DashboardError> for ReportError {
    fn from(error: DashboardError) -> Self {
        Self::Dashboard(error)
    }
}
