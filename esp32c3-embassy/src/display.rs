// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Task for reporting sensor value on a WaveShare E-INK display

use log::{error, info};

use embassy_time::Delay;

use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Receiver};

use embedded_hal_bus::spi::ExclusiveDevice;

use embedded_hal_async::{delay::DelayNs, digital::Wait, spi::SpiDevice};

use embedded_hal::digital::OutputPin;

use time::OffsetDateTime;

use esp_hal::{
    dma::Channel0,
    gpio::{Gpio10, Gpio19, Gpio8, Gpio9, Input, Output, PullUp, PushPull},
    peripherals::SPI2,
    spi::{master::dma::SpiDma, FullDuplexMode},
};

use heapless::HistoryBuffer;

use uom::si::{pressure::hectopascal, ratio::percent, thermodynamic_temperature::degree_celsius};

use waveshare_154bv2_rs::{AsyncDisplay, Buffer, Error as DisplayError};

use crate::{
    dashboard::{draw as draw_dashboard, Error as DashboardError},
    domain::{Reading, Sample},
};

/// Task for displaying samples
#[embassy_executor::task]
pub async fn update_task(
    spi_device: ExclusiveDevice<
        SpiDma<'static, SPI2, Channel0, FullDuplexMode>,
        Gpio8<Output<PushPull>>,
        Delay,
    >,
    busy: Gpio9<Input<PullUp>>,
    rst: Gpio10<Output<PushPull>>,
    dc: Gpio19<Output<PushPull>>,
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
    #[allow(clippy::pattern_type_mismatch)]
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
    info!(" ┣ Temperature: {:.2} C", temperature);
    info!(" ┣ Humidity:    {:.2} %", humidity);
    info!(" ┗ Pressure:    {:.2} hPa", pressure);
}

/// An error
#[derive(Debug)]
enum ReportError {
    /// An error occurred while updating the display
    #[allow(unused)]
    Display(DisplayError),

    /// An error occurred while drawing the dashboard
    #[allow(unused)]
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
