// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Random numbers generator

use core::convert::Infallible;

use rand_core::Rng as _;
use rand_core::TryCryptoRng;
use rand_core::TryRng;

use esp_hal::rng::Rng as EspRng;

/// A wrapper for ESP random number generator that implements traits form
/// `rand_core`
#[derive(Clone)]
pub struct RngWrapper(EspRng);

impl From<EspRng> for RngWrapper {
    fn from(rng: EspRng) -> Self {
        Self(rng)
    }
}

impl TryRng for RngWrapper {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.0.random())
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(u32_pair_to_u64(self.next_u32(), self.next_u32()))
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for value in dest.iter_mut() {
            let [random_value, _, _, _] = self.next_u32().to_be_bytes();
            *value = random_value;
        }

        Ok(())
    }
}

impl TryCryptoRng for RngWrapper {}

/// Join a pair of `u32` into a `u64`
fn u32_pair_to_u64(first: u32, second: u32) -> u64 {
    #![expect(
        clippy::many_single_char_names,
        clippy::min_ident_chars,
        reason = "This is still readable"
    )]
    let [a, b, c, d] = first.to_be_bytes();
    let [e, f, g, h] = second.to_be_bytes();
    u64::from_be_bytes([a, b, c, d, e, f, g, h])
}
