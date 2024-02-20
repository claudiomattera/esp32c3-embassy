// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Random numbers generator

use rand_core::{CryptoRng, Error, RngCore};

use esp_hal::Rng;

/// A wrapper for ESP random number generator that implement traits form
/// `rand_core`
#[derive(Clone)]
pub struct RngWrapper(Rng);

impl From<Rng> for RngWrapper {
    fn from(rng: Rng) -> Self {
        Self(rng)
    }
}

impl RngCore for RngWrapper {
    fn next_u32(&mut self) -> u32 {
        self.0.random()
    }

    fn next_u64(&mut self) -> u64 {
        u32_pair_to_u64(self.next_u32(), self.next_u32())
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        for value in dest.iter_mut() {
            let [random_value, _, _, _] = self.next_u32().to_be_bytes();
            *value = random_value;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

impl CryptoRng for RngWrapper {}

/// Join a pair of `u32` into a `u64`
#[allow(clippy::many_single_char_names, clippy::min_ident_chars)]
fn u32_pair_to_u64(first: u32, second: u32) -> u64 {
    let [a, b, c, d] = first.to_be_bytes();
    let [e, f, g, h] = second.to_be_bytes();
    u64::from_be_bytes([a, b, c, d, e, f, g, h])
}
