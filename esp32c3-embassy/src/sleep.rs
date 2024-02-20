// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Functions for module sleep

use core::time::Duration;

use log::info;

use esp_hal::{peripherals::LPWR, rtc_cntl::sleep::TimerWakeupSource, Delay, Rtc};

/// Enter deep sleep for the specified interval
///
/// **NOTE**: WiFi must be turned off before entering deep sleep, otherwise
/// it will block indefinitely.
pub fn enter_deep(rtc_cntl: LPWR, mut delay: Delay, interval: Duration) -> ! {
    let wakeup_source = TimerWakeupSource::new(interval);

    let mut rtc = Rtc::new(rtc_cntl);

    info!("Entering deep sleep for {interval:?}");
    rtc.sleep_deep(&[&wakeup_source], &mut delay);
}
