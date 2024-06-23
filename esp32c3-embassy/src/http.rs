// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! HTTP client

use embassy_net::dns::DnsSocket;
use embassy_net::dns::Error as DnsError;
use embassy_net::tcp::client::TcpClient;
use embassy_net::tcp::client::TcpClientState;
use embassy_net::tcp::ConnectError as TcpConnectError;
use embassy_net::tcp::Error as TcpError;
use embassy_net::Stack;
use log::debug;

use esp_wifi::wifi::WifiDevice;
use esp_wifi::wifi::WifiStaDevice;

use reqwless::client::HttpClient;
use reqwless::client::TlsConfig;
use reqwless::client::TlsVerify;
use reqwless::request::Method;
use reqwless::Error as ReqlessError;

use heapless::Vec;

use rand_core::RngCore as _;

use crate::RngWrapper;

/// Response size
const RESPONSE_SIZE: usize = 4096;

/// HTTP client
///
/// This trait exists to be extended with requests to specific sites, like in
/// [`WorldTimeApiClient`][crate::worldtimeapi::WorldTimeApiClient].
pub trait ClientTrait {
    /// Send an HTTP request
    async fn send_request(&mut self, url: &str) -> Result<Vec<u8, RESPONSE_SIZE>, Error>;
}

/// HTTP client
pub struct Client {
    /// Wifi stack
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,

    /// Random numbers generator
    rng: RngWrapper,

    /// TCP client state
    tcp_client_state: TcpClientState<1, 4096, 4096>,

    /// Buffer for received TLS data
    read_record_buffer: [u8; 16640],

    /// Buffer for transmitted TLS data
    write_record_buffer: [u8; 16640],
}

impl Client {
    /// Create a new client
    pub fn new(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>, rng: RngWrapper) -> Self {
        debug!("Create TCP client state");
        let tcp_client_state = TcpClientState::<1, 4096, 4096>::new();

        Self {
            stack,
            rng,

            tcp_client_state,

            read_record_buffer: [0_u8; 16640],
            write_record_buffer: [0_u8; 16640],
        }
    }
}

impl ClientTrait for Client {
    async fn send_request(&mut self, url: &str) -> Result<Vec<u8, RESPONSE_SIZE>, Error> {
        debug!("Send HTTPs request to {url}");

        debug!("Create DNS socket");
        let dns_socket = DnsSocket::new(&self.stack);

        let seed = self.rng.next_u64();
        let tls_config = TlsConfig::new(
            seed,
            &mut self.read_record_buffer,
            &mut self.write_record_buffer,
            TlsVerify::None,
        );

        debug!("Create TCP client");
        let tcp_client = TcpClient::new(&self.stack, &self.tcp_client_state);

        debug!("Create HTTP client");
        let mut client = HttpClient::new_with_tls(&tcp_client, &dns_socket, tls_config);

        debug!("Create HTTP request");
        let mut buffer = [0_u8; 4096];
        let mut request = client.request(Method::GET, url).await?;

        debug!("Send HTTP request");
        let response = request.send(&mut buffer).await?;

        debug!("Response status: {:?}", response.status);

        let buffer = response.body().read_to_end().await?;

        debug!("Read {} bytes", buffer.len());

        let output =
            Vec::<u8, RESPONSE_SIZE>::from_slice(buffer).map_err(|()| Error::ResponseTooLarge)?;

        Ok(output)
    }
}

/// An error within an HTTP request
#[derive(Debug)]
pub enum Error {
    /// Response was too large
    ResponseTooLarge,

    /// Error within TCP streams
    Tcp(#[allow(unused)] TcpError),

    /// Error within TCP connection
    TcpConnect(#[allow(unused)] TcpConnectError),

    /// Error within DNS system
    Dns(#[allow(unused)] DnsError),

    /// Error in HTTP client
    Reqless(#[allow(unused)] ReqlessError),
}

impl From<TcpError> for Error {
    fn from(error: TcpError) -> Self {
        Self::Tcp(error)
    }
}

impl From<TcpConnectError> for Error {
    fn from(error: TcpConnectError) -> Self {
        Self::TcpConnect(error)
    }
}

impl From<DnsError> for Error {
    fn from(error: DnsError) -> Self {
        Self::Dns(error)
    }
}

impl From<ReqlessError> for Error {
    fn from(error: ReqlessError) -> Self {
        Self::Reqless(error)
    }
}
