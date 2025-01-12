// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Dashboard for E-INK screen

use core::convert::Infallible;
use core::fmt::Error as FmtError;
use core::fmt::Write as _;

use embedded_graphics::mono_font::iso_8859_1::FONT_10X20 as FONT;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;

use embedded_layout::align::Align;
use embedded_layout::layout::linear::LinearLayout;
use embedded_layout::prelude::horizontal;
use embedded_layout::prelude::vertical;
use embedded_layout::prelude::Chain;
use embedded_layout::View;

use uom::si::f32::Pressure;
use uom::si::f32::Ratio as Humidity;
use uom::si::f32::ThermodynamicTemperature as Temperature;
use uom::si::pressure::hectopascal;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature::degree_celsius;

use heapless::String;

use time::OffsetDateTime;

use waveshare_154bv2_rs::Color as TriColor;

use crate::Sample;

/// Style for black text
pub const BLACK_STYLE: MonoTextStyle<TriColor> = MonoTextStyleBuilder::new()
    .font(&FONT)
    .text_color(TriColor::Black)
    .background_color(TriColor::White)
    .build();

/// Style for chromatic text
pub const CHROMATIC_STYLE: MonoTextStyle<TriColor> = MonoTextStyleBuilder::new()
    .font(&FONT)
    .text_color(TriColor::Chromatic)
    .background_color(TriColor::White)
    .build();

/// Draw a dashboard
pub fn draw<DISPLAY>(
    display: &mut DISPLAY,
    now: &OffsetDateTime,
    sample: &Sample,
) -> Result<(), Error>
where
    DISPLAY: DrawTarget<Color = TriColor, Error = Infallible>,
{
    let display_area = display.bounding_box();
    let temperature = format_temperature(sample.temperature)?;
    let humidity = format_humidity(sample.humidity)?;
    let pressure = format_pressure(sample.pressure)?;
    let time = format_time(now)?;

    let temperature_layout = lay_out_measurement("Temperature: ", &temperature, " C");
    let humidity_layout = lay_out_measurement("Humidity: ", &humidity, " %");
    let pressure_layout = lay_out_measurement("Pressure: ", &pressure, " hPa");
    let time_layout = lay_out_update_time(&time);

    LinearLayout::vertical(
        Chain::new(temperature_layout)
            .append(humidity_layout)
            .append(pressure_layout)
            .append(time_layout),
    )
    .with_alignment(horizontal::Left)
    .arrange()
    .align_to(&display_area, horizontal::Left, vertical::Top)
    .draw(display)?;

    Ok(())
}

/// Lay out a measurement row
fn lay_out_measurement<'text>(
    label: &'text str,
    value: &'text str,
    unit: &'text str,
) -> impl Drawable<Color = TriColor> + View + 'text {
    LinearLayout::horizontal(
        Chain::new(Text::new(label, Point::zero(), BLACK_STYLE))
            .append(Text::new(value, Point::zero(), CHROMATIC_STYLE))
            .append(Text::new(unit, Point::zero(), BLACK_STYLE)),
    )
    .with_alignment(vertical::Center)
    .arrange()
}

/// Lay out the update time row
#[allow(
    clippy::needless_lifetimes,
    reason = "Lifetime annotation is actually needed"
)]
fn lay_out_update_time<'text>(now: &'text str) -> impl Drawable<Color = TriColor> + View + 'text {
    LinearLayout::horizontal(
        Chain::new(Text::new("Updated at ", Point::zero(), BLACK_STYLE)).append(Text::new(
            now,
            Point::zero(),
            CHROMATIC_STYLE,
        )),
    )
    .with_alignment(vertical::Center)
    .arrange()
}

/// Format a time as `HOUR:MINUTE`
fn format_time(now: &OffsetDateTime) -> Result<String<5>, Error> {
    let mut string: String<5> = String::new();
    write!(&mut string, "{:0>2}:{:0>2}", now.hour(), now.minute())?;

    Ok(string)
}

/// Format a temperature value
fn format_temperature(temperature: Temperature) -> Result<String<10>, FmtError> {
    let mut string: String<10> = String::new();
    write!(&mut string, "{:>3.1}", temperature.get::<degree_celsius>())?;
    Ok(string)
}

/// Format a humidity value
fn format_humidity(humidity: Humidity) -> Result<String<10>, FmtError> {
    let mut string: String<10> = String::new();
    write!(&mut string, "{:>5.0}", humidity.get::<percent>())?;
    Ok(string)
}

/// Format a pressure value
fn format_pressure(pressure: Pressure) -> Result<String<10>, FmtError> {
    let mut string: String<10> = String::new();
    write!(&mut string, "{:>5.1}", pressure.get::<hectopascal>())?;
    Ok(string)
}

/// An error
#[derive(Debug)]
pub enum Error {
    /// An impossible error existing only to satisfy the type system
    Impossible(Infallible),

    /// An error occurred while formatting a string
    Fmt(FmtError),
}

impl From<FmtError> for Error {
    fn from(error: FmtError) -> Self {
        Self::Fmt(error)
    }
}

impl From<Infallible> for Error {
    fn from(error: Infallible) -> Self {
        Self::Impossible(error)
    }
}
