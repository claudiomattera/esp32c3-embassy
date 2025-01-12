// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! An `UnsafeCell` that implements `Sync`
//!
//! This is a placeholder until `core::cell::SyncUnsafeCell` is stabilized.

use core::cell::UnsafeCell;

/// An `UnsafeCell` that implements `Sync`
pub struct SyncUnsafeCell<T> {
    /// The inner cell
    inner: UnsafeCell<T>,
}

impl<T> SyncUnsafeCell<T> {
    /// Create a new cell
    #[must_use]
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
        }
    }

    /// Get a mutable pointer to the wrapped value
    pub fn get(&self) -> *mut T {
        self.inner.get()
    }
}

// SAFETY:
// There is only one thread on a ESP32-C3.
unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}
