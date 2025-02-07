// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Random numbers generator

use rand_core::CryptoRng;
use rand_core::RngCore;

use esp_hal::rng::Rng;

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
            let [random_value, _, _, _] = self.next_u32().to_ne_bytes();
            *value = random_value;
        }
    }
}

impl CryptoRng for RngWrapper {}

/// Join a pair of `u32` into a `u64`
#[allow(
    clippy::many_single_char_names,
    clippy::min_ident_chars,
    reason = "This is still readable"
)]
fn u32_pair_to_u64(first: u32, second: u32) -> u64 {
    let [a, b, c, d] = first.to_ne_bytes();
    let [e, f, g, h] = second.to_ne_bytes();
    u64::from_ne_bytes([a, b, c, d, e, f, g, h])
}
