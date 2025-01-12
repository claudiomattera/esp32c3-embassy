// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Underlying graphical buffer

use core::convert::Infallible;

use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::geometry::OriginDimensions;
use embedded_graphics_core::geometry::Point;
use embedded_graphics_core::geometry::Size;
use embedded_graphics_core::Pixel;

use crate::Color;

// /// Screen width
// const WIDTH: usize = 8;

// /// Screen height
// const HEIGHT: usize = 8;

// /// Screen size in bits
// const BIT_SIZE: usize = WIDTH * HEIGHT;

// /// Screen size in bits
// const BYTE_SIZE: usize = BIT_SIZE / 8;

/// A screen rotation
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Rotation {
    /// No rotation
    Rotate0,

    /// Clockwise rotation of 90 degrees
    Rotate90,

    /// Clockwise rotation of 180 degrees
    Rotate180,

    /// Clockwise rotation of 270 degrees
    Rotate270,
}

/// A buffer to draw tri-colors graphics
///
/// `WIDTH` and `HEIGHT` are the screen width and height in pixels, while
/// `BYTE_SIZE` is the screen size in bytes (width × height ÷ 8).
#[derive(Debug)]
pub struct Buffer<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize> {
    /// Buffer rotation
    rotation: Rotation,

    /// Black part of the buffer
    black: [u8; BYTE_SIZE],

    /// Chromatic part of the buffer
    chromatic: [u8; BYTE_SIZE],
}

impl<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize>
    Buffer<WIDTH, HEIGHT, BYTE_SIZE>
{
    /// Create a new graphical buffer
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rotation: Rotation::Rotate0,
            black: [255; BYTE_SIZE],
            chromatic: [255; BYTE_SIZE],
        }
    }

    /// Get the black part of the buffer
    #[must_use]
    pub fn black_buffer(&self) -> &[u8] {
        &self.black
    }

    /// Get the chromatic part of the buffer
    #[must_use]
    pub fn chromatic_buffer(&self) -> &[u8] {
        &self.chromatic
    }

    /// Set screen rotation
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.rotation = rotation;
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize> Default
    for Buffer<WIDTH, HEIGHT, BYTE_SIZE>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize> DrawTarget
    for Buffer<WIDTH, HEIGHT, BYTE_SIZE>
{
    type Error = Infallible;

    type Color = Color;

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        #[allow(clippy::pattern_type_mismatch)]
        let pixels = pixels.into_iter().filter(|Pixel(Point { x, y }, _color)| {
            *x >= 0_i32 && *x < WIDTH as i32 && *y >= 0_i32 && *y < HEIGHT as i32
        });

        for Pixel(Point { x, y }, color) in pixels {
            let (index, offset) = get_index_and_offset::<WIDTH>(x, y);
            if index >= BYTE_SIZE || offset >= 8 {
                continue;
            }
            let offset = 8 - offset - 1;
            let mask: u8 = 0b0000_0001 << offset;
            let reverse_mask: u8 = !mask;
            match color {
                Color::Black => {
                    let original = self.black[index];
                    self.black[index] = (original & reverse_mask) | (0 << offset);
                }
                Color::Chromatic => {
                    let original = self.chromatic[index];
                    self.chromatic[index] = original & reverse_mask | (0 << offset);
                }
                Color::White => {
                    let original = self.black[index];
                    self.black[index] = original & reverse_mask | (1 << offset);
                    let original = self.chromatic[index];
                    self.chromatic[index] = (original & reverse_mask) | (1 << offset);
                }
                Color::Transparent => {}
            }
        }

        Ok(())
    }
}

/// Get index and offset
fn get_index_and_offset<const WIDTH: usize>(x: i32, y: i32) -> (usize, usize) {
    let bit_index = get_bit_index::<WIDTH>(x, y);
    get_index_and_offset_from_bit_index(bit_index)
}

/// Get bit index
#[allow(clippy::cast_sign_loss)]
fn get_bit_index<const WIDTH: usize>(x: i32, y: i32) -> usize {
    x as usize + y as usize * WIDTH
}

/// Get index and offset from bit index
fn get_index_and_offset_from_bit_index(bit_index: usize) -> (usize, usize) {
    let index = bit_index >> 3_i32;
    let offset = bit_index & 0b0000_0111;
    (index, offset)
}

impl<const WIDTH: usize, const HEIGHT: usize, const BYTE_SIZE: usize> OriginDimensions
    for Buffer<WIDTH, HEIGHT, BYTE_SIZE>
{
    #[allow(clippy::cast_possible_truncation)]
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

/// A buffer sized for 1.54 in displays
#[allow(clippy::module_name_repetitions)]
pub type Epd1in54Buffer = Buffer<200, 200, 5000>;
