// Copyright Claudio Mattera 2024.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! HTTP client

use core::fmt::Error as FormatError;
use core::num::ParseIntError;

use log::{debug, trace, warn};

use embassy_net::{
    dns::{DnsQueryType, Error as DnsError},
    tcp::{ConnectError as TcpConnectError, Error as TcpError, TcpSocket},
    IpAddress, Stack,
};

use esp_wifi::wifi::{WifiDevice, WifiStaDevice};

use embedded_tls::{Aes128GcmSha256, NoVerify, TlsConfig, TlsConnection, TlsContext, TlsError};

use reqwless::{
    request::{Method, Request, RequestBuilder},
    response::Response,
    Error as ReqlessError,
};

use heapless::Vec;

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

    /// Buffer for received TCP data
    rx_buffer: [u8; 4096],

    /// Buffer for transmitted TCP data
    tx_buffer: [u8; 4096],

    /// Buffer for received TLS data
    read_record_buffer: [u8; 16640],

    /// Buffer for transmitted TLS data
    write_record_buffer: [u8; 16640],
}

impl Client {
    /// Create a new client
    pub fn new(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>, rng: RngWrapper) -> Self {
        Self {
            stack,
            rng,

            rx_buffer: [0_u8; 4096],
            tx_buffer: [0_u8; 4096],

            read_record_buffer: [0_u8; 16640],
            write_record_buffer: [0_u8; 16640],
        }
    }

    /// Send a plain HTTP request
    async fn send_plain_http_request(
        &mut self,
        url: &str,
        host: &str,
        port: u16,
        path: &str,
    ) -> Result<Vec<u8, RESPONSE_SIZE>, Error> {
        debug!("Send plain HTTP request to path {path} at host {host}:{port}");

        let ip_address = self.resolve(host).await?;
        let remote_endpoint = (ip_address, port);

        debug!("Create TCP socket");
        let mut socket = TcpSocket::new(self.stack, &mut self.rx_buffer, &mut self.tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        debug!("Connect to HTTP server");
        socket.connect(remote_endpoint).await?;
        debug!("Connected to HTTP server");

        let request = Request::get(url).build();
        request.write(&mut socket).await?;

        let mut headers_buf = [0_u8; 1024];
        let mut buf = [0_u8; 4096];
        let response = Response::read(&mut socket, Method::GET, &mut headers_buf).await?;

        debug!("Response status: {:?}", response.status);

        let total_length = response.body().reader().read_to_end(&mut buf).await?;

        debug!("Close TCP socket");
        socket.close();

        debug!("Read {} bytes", total_length);

        let output = Vec::<u8, RESPONSE_SIZE>::from_slice(&buf[..total_length])
            .map_err(|()| Error::ResponseTooLarge)?;

        Ok(output)
    }

    /// Send an HTTPS request
    async fn send_https_request(
        &mut self,
        url: &str,
        host: &str,
        port: u16,
        path: &str,
    ) -> Result<Vec<u8, RESPONSE_SIZE>, Error> {
        debug!("Send HTTPs request to path {path} at host {host}:{port}");

        let ip_address = self.resolve(host).await?;
        let remote_endpoint = (ip_address, port);

        let mut socket = TcpSocket::new(self.stack, &mut self.rx_buffer, &mut self.tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

        debug!("Connect to HTTP server");
        socket.connect(remote_endpoint).await?;
        debug!("Connected to HTTP server");

        let config: TlsConfig<Aes128GcmSha256> = TlsConfig::new()
            .with_server_name(host)
            .enable_rsa_signatures();
        let mut tls = TlsConnection::new(
            socket,
            &mut self.read_record_buffer,
            &mut self.write_record_buffer,
        );

        debug!("Perform TLS handshake");
        tls.open::<_, NoVerify>(TlsContext::new(&config, &mut self.rng))
            .await?;
        debug!("TLS handshake succeeded");

        let request = Request::get(url).build();
        request.write(&mut tls).await?;

        let mut headers_buf = [0_u8; 1024];
        let mut buf = [0_u8; 4096];
        let response = Response::read(&mut tls, Method::GET, &mut headers_buf).await?;

        debug!("Response status: {:?}", response.status);

        let total_length = response.body().reader().read_to_end(&mut buf).await?;

        debug!("Close TLS wrapper");
        let mut socket = match tls.close().await {
            Ok(socket) => socket,
            Err((socket, error)) => {
                warn!("Cannot close TLS wrapper: {error:?}");
                socket
            }
        };

        debug!("Close TCP socket");
        socket.close();

        debug!("Read {} bytes", total_length);

        let output = Vec::<u8, RESPONSE_SIZE>::from_slice(&buf[..total_length])
            .map_err(|()| Error::ResponseTooLarge)?;

        Ok(output)
    }

    /// Resolve a hostname to an IP address through DNS
    async fn resolve(&mut self, host: &str) -> Result<IpAddress, Error> {
        let mut ip_addresses = self.stack.dns_query(host, DnsQueryType::A).await?;
        let ip_address = ip_addresses.pop().ok_or(Error::DnsLookup)?;
        debug!("Host {host} resolved to {ip_address}");
        Ok(ip_address)
    }
}

impl ClientTrait for Client {
    async fn send_request(&mut self, url: &str) -> Result<Vec<u8, RESPONSE_SIZE>, Error> {
        if let Some(rest) = url.strip_prefix("https://") {
            trace!("Rest: {rest}");
            let (host_and_port, path) = rest.split_once('/').unwrap_or((rest, ""));
            trace!("Host and port: {host_and_port}, path: {path}");
            let (host, port) = host_and_port
                .split_once(':')
                .unwrap_or((host_and_port, "443"));
            trace!("Host: {host}, port: {port}, path: {path}");
            let port = port.parse::<u16>().map_err(Error::PortParse)?;
            self.send_https_request(url, host, port, path).await
        } else if let Some(rest) = url.strip_prefix("http://") {
            trace!("Rest: {rest}");
            let (host_and_port, path) = rest.split_once('/').unwrap_or((rest, ""));
            trace!("Host and port: {host_and_port}, path: {path}");
            let (host, port) = host_and_port
                .split_once(':')
                .unwrap_or((host_and_port, "80"));
            trace!("Host: {host}, port: {port}, path: {path}");
            let port = port.parse::<u16>().map_err(Error::PortParse)?;
            self.send_plain_http_request(url, host, port, path).await
        } else {
            Err(Error::UnsupportedScheme)
        }
    }
}

/// An error within an HTTP request
#[derive(Debug)]
pub enum Error {
    /// URL scheme is not supported
    UnsupportedScheme,

    /// Response was too large
    ResponseTooLarge,

    /// Hostname could not be resolved through DNS
    DnsLookup,

    /// Error within TCP streams
    PortParse(ParseIntError),

    /// Error within TCP streams
    Tcp(TcpError),

    /// Error within TCP connection
    TcpConnect(TcpConnectError),

    /// Error within DNS system
    Dns(DnsError),

    /// Error while formatting strings
    Format(FormatError),

    /// Error while handling TLS
    Tls(TlsError),

    /// Error in HTTP client
    Reqless(ReqlessError),
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

impl From<FormatError> for Error {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}

impl From<TlsError> for Error {
    fn from(error: TlsError) -> Self {
        Self::Tls(error)
    }
}

impl From<ReqlessError> for Error {
    fn from(error: ReqlessError) -> Self {
        Self::Reqless(error)
    }
}
