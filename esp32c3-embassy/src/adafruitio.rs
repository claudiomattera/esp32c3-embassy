// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Client for Adafruit IO API

use core::num::ParseIntError;
use core::str::from_utf8;
use core::str::Utf8Error;

use microjson::JSONParsingError;

use time::error::ComponentRange as TimeComponentRangeError;
use time::OffsetDateTime;

use crate::http::Client as HttpClient;
use crate::http::ClientTrait as HttpClientTrait;
use crate::http::Error as HttpError;

/// Extend an HTTP client for accessing Adafruit IO API
pub trait AdafruitIoClient: HttpClientTrait {
    /// Fetch current time
    async fn fetch_current_time(&mut self) -> Result<OffsetDateTime, Error> {
        let url = "https://io.adafruit.com/api/v2/time/seconds";

        let response = self.send_request(url).await?;

        let text = from_utf8(&response)?;
        let timestamp = text.parse::<i64>()?;
        let utc = OffsetDateTime::from_unix_timestamp(timestamp)?;
        Ok(utc)
    }
}

impl AdafruitIoClient for HttpClient {}

/// An error within a request to Adafruit IO
#[derive(Debug)]
pub enum Error {
    /// Error from HTTP client
    Http(#[expect(unused, reason = "Never read directly")] HttpError),

    /// A time component is out of range
    TimeComponentRange(#[expect(unused, reason = "Never read directly")] TimeComponentRangeError),

    /// An integer valued returned by the server could not be parsed
    ParseInt(#[expect(unused, reason = "Never read directly")] ParseIntError),

    /// Text returned by the server is not valid UTF-8
    Utf8(#[expect(unused, reason = "Never read directly")] Utf8Error),

    /// Text returned by the server is not valid JSON
    Json(#[expect(unused, reason = "Never read directly")] JSONParsingError),
}

impl From<HttpError> for Error {
    fn from(error: HttpError) -> Self {
        Self::Http(error)
    }
}

impl From<ParseIntError> for Error {
    fn from(error: ParseIntError) -> Self {
        Self::ParseInt(error)
    }
}

impl From<TimeComponentRangeError> for Error {
    fn from(error: TimeComponentRangeError) -> Self {
        Self::TimeComponentRange(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

impl From<JSONParsingError> for Error {
    fn from(error: JSONParsingError) -> Self {
        Self::Json(error)
    }
}
