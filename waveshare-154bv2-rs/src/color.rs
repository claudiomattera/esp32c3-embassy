// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files License-MIT.txt and License-Apache-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! A color that can have three values

use embedded_graphics_core::pixelcolor::raw::RawU2;
use embedded_graphics_core::pixelcolor::BinaryColor;
use embedded_graphics_core::pixelcolor::PixelColor;
use embedded_graphics_core::pixelcolor::Rgb888;
use embedded_graphics_core::prelude::RawData;

/// A tri-color
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Color {
    /// Black
    Black,

    /// Chromatic
    Chromatic,

    /// White
    White,

    /// Transparent
    Transparent,
}

impl PixelColor for Color {
    type Raw = RawU2;
}

impl From<RawU2> for Color {
    fn from(color: RawU2) -> Self {
        #[allow(clippy::match_same_arms)]
        match color.into_inner() {
            0b0000_0000 => Color::Black,
            0b0000_0001 => Color::White,
            0b0000_0010 => Color::Chromatic,
            0b0000_0011 => Color::Transparent,
            _ => Color::Transparent,
        }
    }
}

impl From<BinaryColor> for Color {
    fn from(color: BinaryColor) -> Self {
        match color {
            BinaryColor::On => Color::Black,
            BinaryColor::Off => Color::White,
        }
    }
}

impl From<Rgb888> for Color {
    fn from(color: Rgb888) -> Self {
        let binary_color: BinaryColor = color.into();
        binary_color.into()
    }
}

impl From<Color> for Rgb888 {
    fn from(color: Color) -> Self {
        match color {
            Color::Black => Self::new(0, 0, 0),
            Color::Chromatic => Self::new(255, 0, 0),
            Color::White => Self::new(255, 255, 255),
            Color::Transparent => Self::new(0, 255, 0),
        }
    }
}
