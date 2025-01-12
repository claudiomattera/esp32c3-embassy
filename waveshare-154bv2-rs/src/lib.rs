// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Interface to WaveShare 1.54inches B v2 E-INK display

#![no_std]

#[cfg(feature = "async")]
mod r#async;
#[cfg(feature = "async")]
pub use self::r#async::Display as AsyncDisplay;

#[cfg(feature = "blocking")]
mod blocking;
#[cfg(feature = "blocking")]
pub use self::blocking::Display;

#[cfg(any(feature = "async", feature = "blocking"))]
mod command;

#[cfg(feature = "draw-target")]
mod buffer;
#[cfg(feature = "draw-target")]
pub use self::buffer::Buffer;
#[cfg(feature = "draw-target")]
pub use self::buffer::Epd1in54Buffer;
#[cfg(feature = "draw-target")]
pub use self::buffer::Rotation;

#[cfg(feature = "draw-target")]
mod color;
#[cfg(feature = "draw-target")]
pub use self::color::Color;

#[cfg(any(feature = "async", feature = "blocking", feature = "draw-target"))]
mod error;
#[cfg(any(feature = "async", feature = "blocking", feature = "draw-target"))]
pub use self::error::Error;
