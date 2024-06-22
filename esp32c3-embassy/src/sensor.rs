// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Task for reading sensor value

use log::error;
use log::info;
use log::warn;

use embassy_time::Delay;
use embassy_time::Duration;
use embassy_time::Timer;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Sender;

use esp_hal::i2c::Error as I2cError;
use esp_hal::i2c::I2C;
use esp_hal::peripherals::I2C0;
use esp_hal::rng::Rng;
use esp_hal::Async;

use bme280_rs::AsyncBme280;
use bme280_rs::Configuration;
use bme280_rs::Oversampling;
use bme280_rs::Sample as Bme280Sample;
use bme280_rs::SensorMode;

use crate::clock::Clock;
use crate::clock::Error as ClockError;
use crate::domain::Error as DomainError;
use crate::domain::Reading;
use crate::domain::Sample;

/// Interval to wait for sensor warmup
const WARMUP_INTERVAL: Duration = Duration::from_millis(10);

/// Task for sampling sensor
#[embassy_executor::task]
pub async fn sample_task(
    i2c: I2C<'static, I2C0, Async>,
    mut rng: Rng,
    sender: Sender<'static, NoopRawMutex, Reading, 3>,
    clock: Clock,
    sampling_period: Duration,
) {
    info!("Create");
    let mut sensor = AsyncBme280::new(i2c, Delay);

    if let Err(error) = initialize(&mut sensor).await {
        warn!("Could not initialize sensor: {error:?}");
    }

    info!(
        "Waiting {}ms for configuration to be processed",
        WARMUP_INTERVAL.as_millis()
    );
    Timer::after(WARMUP_INTERVAL).await;

    loop {
        if let Err(error) = sample_and_send(&mut sensor, &mut rng, &sender, &clock).await {
            error!("Could not sample sensor: {error:?}");
        }

        let wait_interval = clock.duration_to_next_rounded_wakeup(sampling_period);
        info!("Wait {}s for next sample", wait_interval.as_secs());
        Timer::after(wait_interval).await;
    }
}

/// Sample sensor and send reading to receiver
async fn sample_and_send(
    sensor: &mut AsyncBme280<I2C<'static, I2C0, Async>, Delay>,
    rng: &mut Rng,
    sender: &Sender<'static, NoopRawMutex, Reading, 3>,
    clock: &Clock,
) -> Result<(), SensorError> {
    info!("Read sample");

    let now = clock.now()?;

    let sample_result = sensor
        .read_sample()
        .await
        .map_err(SensorError::I2c)
        .and_then(|sample: Bme280Sample| Ok(Sample::try_from(sample)?));
    let sample = sample_result.unwrap_or_else(|error| {
        error!("Cannot read sample: {error:?}");
        warn!("Use a random sample");

        Sample::random(rng)
    });

    let reading = (now, sample);
    sender.send(reading).await;

    Ok(())
}

/// Initialize sensor
async fn initialize(
    bme280: &mut AsyncBme280<I2C<'static, I2C0, Async>, Delay>,
) -> Result<(), I2cError> {
    info!("Initialize");
    bme280.init().await?;

    info!("Configure");
    bme280
        .set_sampling_configuration(
            Configuration::default()
                .with_temperature_oversampling(Oversampling::Oversample1)
                .with_pressure_oversampling(Oversampling::Oversample1)
                .with_humidity_oversampling(Oversampling::Oversample1)
                .with_sensor_mode(SensorMode::Normal),
        )
        .await?;
    Ok(())
}

/// Error within sensor sampling
#[derive(Debug)]
enum SensorError {
    /// Error from clock
    #[allow(unused)]
    Clock(ClockError),

    /// Error from domain
    #[allow(unused)]
    Domain(DomainError),

    /// Error from I²C bus
    #[allow(unused)]
    I2c(I2cError),
}

impl From<ClockError> for SensorError {
    fn from(error: ClockError) -> Self {
        Self::Clock(error)
    }
}

impl From<DomainError> for SensorError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

impl From<I2cError> for SensorError {
    fn from(error: I2cError) -> Self {
        Self::I2c(error)
    }
}
