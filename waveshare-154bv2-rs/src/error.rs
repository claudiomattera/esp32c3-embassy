// Copyright Claudio Mattera 2024.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files License-MIT.txt and License-Apache-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Data structures and functions for error handling

#[cfg(any(feature = "async", feature = "blocking"))]
use embedded_hal::digital::Error as DigitalError;
#[cfg(any(feature = "async", feature = "blocking"))]
use embedded_hal::digital::ErrorKind as DigitalErrorKind;
#[cfg(any(feature = "async", feature = "blocking"))]
use embedded_hal::spi::Error as SpiError;
#[cfg(any(feature = "async", feature = "blocking"))]
use embedded_hal::spi::ErrorKind as SpiErrorKind;

/// An error
#[derive(Debug, PartialEq)]
pub enum Error {
    #[cfg(any(feature = "async", feature = "blocking"))]
    /// An error in the underlying SPI bus
    Spi(SpiErrorKind),

    #[cfg(any(feature = "async", feature = "blocking"))]
    /// An error in the underlying digital system
    Digital(DigitalErrorKind),
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl<E> From<E> for Error
where
    E: SpiError,
{
    fn from(error: E) -> Self {
        Self::Spi(error.kind())
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
impl Error {
    /// Convert a digital error to an error
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_digital<E>(error: E) -> Self
    where
        E: DigitalError,
    {
        Self::Digital(error.kind())
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

#[cfg(feature = "std")]
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}
