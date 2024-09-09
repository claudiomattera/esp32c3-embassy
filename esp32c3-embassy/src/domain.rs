// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Domain types

use esp_hal::rng::Rng;

use uom::si::f32::Pressure;
use uom::si::f32::Ratio as Humidity;
use uom::si::f32::ThermodynamicTemperature as Temperature;

use time::OffsetDateTime;

use bme280_rs::Sample as Bme280Sample;

/// A sample
#[derive(Clone, Debug, Default)]
pub struct Sample {
    /// Temperature
    pub temperature: Temperature,

    /// Humidity
    pub humidity: Humidity,

    /// Pressure
    pub pressure: Pressure,
}

impl Sample {
    /// Construct a random sample
    #[expect(clippy::cast_precision_loss, reason = "Acceptable precision loss")]
    pub fn random(rng: &mut Rng) -> Self {
        let temperature_seed = rng.random() as f32 / u32::MAX as f32;
        let humidity_seed = rng.random() as f32 / u32::MAX as f32;
        let pressure_seed = rng.random() as f32 / u32::MAX as f32;

        let temperature = temperature_seed * (30.0 - 15.0) + 15.0;
        let humidity = humidity_seed * (80.0 - 20.0) + 20.0;
        let pressure = pressure_seed * (1010.0 - 990.0) + 990.0;

        Self::from((
            uom::si::f32::ThermodynamicTemperature::new::<
                uom::si::thermodynamic_temperature::degree_celsius,
            >(temperature),
            uom::si::f32::Ratio::new::<uom::si::ratio::percent>(humidity),
            uom::si::f32::Pressure::new::<uom::si::pressure::hectopascal>(pressure),
        ))
    }
}

impl From<(Temperature, Humidity, Pressure)> for Sample {
    fn from((temperature, humidity, pressure): (Temperature, Humidity, Pressure)) -> Self {
        Self {
            temperature,
            humidity,
            pressure,
        }
    }
}

impl TryFrom<Bme280Sample> for Sample {
    type Error = Error;

    fn try_from(sample: Bme280Sample) -> Result<Self, Self::Error> {
        let temperature = sample.temperature.ok_or(Self::Error::MissingMeasurement)?;
        let humidity = sample.humidity.ok_or(Self::Error::MissingMeasurement)?;
        let pressure = sample.pressure.ok_or(Self::Error::MissingMeasurement)?;
        Ok(Self {
            temperature,
            humidity,
            pressure,
        })
    }
}

/// A reading, i.e. a pair (time, sample)
pub type Reading = (OffsetDateTime, Sample);

/// An error
#[derive(Debug)]
pub enum Error {
    /// A measurement was missing
    MissingMeasurement,
}
