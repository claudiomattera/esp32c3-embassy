// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Data types and function for keeping time and synchronizing clock

use embassy_time::Duration;
use embassy_time::Instant;

use esp_hal::macros::ram;

use time::error::ComponentRange as TimeComponentRange;
use time::OffsetDateTime;
use time::UtcOffset;

use crate::adafruitio::AdafruitIoClient as _;
use crate::adafruitio::Error as AdafruitIoError;
use crate::http::Client as HttpClient;

/// Stored boot time between deep sleep cycles
///
/// This is a statically allocated variable and it is placed in the RTC Fast
/// memory, which survives deep sleep.
#[ram(rtc_fast)]
static mut BOOT_TIME: (u64, i32) = (0, 0);

/// A clock
#[derive(Clone, Debug)]
pub struct Clock {
    /// The boot time in Unix epoch
    boot_time: u64,

    /// The time offset
    offset: UtcOffset,
}

impl Clock {
    /// Create a new clock
    pub fn new(current_time: u64, offset: UtcOffset) -> Self {
        let from_boot = Instant::now().as_secs();
        let boot_time = current_time - from_boot;

        Self { boot_time, offset }
    }

    /// Return the current time
    pub fn now(&self) -> Result<OffsetDateTime, Error> {
        let epoch = self.now_as_epoch();
        #[expect(clippy::cast_possible_wrap, reason = "Timestamp will fit an i64")]
        let utc = OffsetDateTime::from_unix_timestamp(epoch as i64)?;
        let local = utc
            .checked_to_offset(self.offset)
            .ok_or(Error::InvalidInOffset)?;
        Ok(local)
    }

    /// Create a new clock by synchronizing with a server
    pub async fn from_server(
        http_client: &mut HttpClient,
        // stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    ) -> Result<Self, Error> {
        let now = http_client.fetch_current_time().await?;

        let current_time = now.unix_timestamp();

        #[expect(
            clippy::cast_sign_loss,
            reason = "Current timestamp will never be negative"
        )]
        let current_time = current_time as u64;

        let offset = now.offset();

        Ok(Self::new(current_time, offset))
    }

    /// Initialize clock from RTC Fast memory
    pub fn from_rtc_memory() -> Option<Self> {
        // SAFETY:
        // There is only one thread
        let (now, offset_in_seconds) = unsafe { BOOT_TIME };
        let offset = UtcOffset::from_whole_seconds(offset_in_seconds).ok();

        if now == 0 {
            None
        } else {
            offset.map(|offset| Self::new(now, offset))
        }
    }

    /// Store clock into RTC Fast memory
    pub fn save_to_rtc_memory(&self, expected_sleep_duration: Duration) {
        let now = self.now_as_epoch();
        let then = now + expected_sleep_duration.as_secs();
        let offset_in_seconds = self.offset.whole_seconds();
        // SAFETY:
        // There is only one thread
        unsafe {
            BOOT_TIME = (then, offset_in_seconds);
        }
    }

    /// Compute the next wakeup rounded down to a period
    ///
    /// * At 09:46:12 with period 1 minute, next rounded wakeup is 09:47:00.
    /// * At 09:46:12 with period 5 minutes, next rounded wakeup is 09:50:00.
    /// * At 09:46:12 with period 1 hour, next rounded wakeup is 10:00:00.
    pub fn duration_to_next_rounded_wakeup(&self, period: Duration) -> Duration {
        let epoch = Duration::from_secs(self.now_as_epoch());
        duration_to_next_rounded_wakeup(epoch, period)
    }

    /// Return current time as a Unix epoch
    pub fn now_as_epoch(&self) -> u64 {
        let from_boot = Instant::now().as_secs();
        self.boot_time + from_boot
    }
}

/// Compute the next wakeup rounded down to a period
///
/// * At 09:46:12 with period 1 minute, next rounded wakeup is 09:47:00.
/// * At 09:46:12 with period 5 minutes, next rounded wakeup is 09:50:00.
/// * At 09:46:12 with period 1 hour, next rounded wakeup is 10:00:00.
fn next_rounded_wakeup(now: Duration, period: Duration) -> Duration {
    let then = now + period;
    Duration::from_secs((then.as_secs() / period.as_secs()) * period.as_secs())
}

/// Compute the duration to next wakeup rounded down to a period
fn duration_to_next_rounded_wakeup(now: Duration, period: Duration) -> Duration {
    let then = next_rounded_wakeup(now, period);
    then - now
}

/// A clock error
#[derive(Debug)]
pub enum Error {
    /// A time component is out of range
    TimeComponentRange(#[expect(unused, reason = "Never read directly")] TimeComponentRange),

    /// The time is invalid in the current time offset
    InvalidInOffset,

    /// Error synchronizing time from World Time API
    Synchronization(#[expect(unused, reason = "Never read directly")] AdafruitIoError),
}

impl From<TimeComponentRange> for Error {
    fn from(error: TimeComponentRange) -> Self {
        Self::TimeComponentRange(error)
    }
}

impl From<AdafruitIoError> for Error {
    fn from(error: AdafruitIoError) -> Self {
        Self::Synchronization(error)
    }
}
