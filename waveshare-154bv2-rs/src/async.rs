// Copyright Claudio Mattera 2024.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files License-MIT.txt and License-Apache-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Async display

use log::debug;
use log::log_enabled;
use log::trace;
use log::Level::Trace;

use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::OutputPin;

use crate::command;
use crate::Error;

#[cfg(feature = "draw-target")]
use crate::Buffer;

/// Flag for busy low
const IS_BUSY_LOW: bool = false;

/// Display width
const WIDTH: usize = 200;

/// Display height
const HEIGHT: usize = 200;

/// Display byte size
const BYTE_SIZE: usize = 5000;

/// A Waveshare E-ink screen
pub struct Display<SPI: SpiDevice, BUSY: Wait, RST: OutputPin, DC: OutputPin, DELAY: DelayNs> {
    /// SPI interface
    spi: SPI,

    /// Busy pin
    busy: BUSY,

    /// Reset pin
    rst: RST,

    /// DC pin
    dc: DC,

    ///Delay
    delay: DELAY,

    /// Flag to force writing one byte at the time
    individual_writes: bool,
}

impl<SPI, BUSY, RST, DC, DELAY> Display<SPI, BUSY, RST, DC, DELAY>
where
    SPI: SpiDevice,
    BUSY: Wait,
    RST: OutputPin,
    DC: OutputPin,
    DELAY: DelayNs,
{
    /// Create a new display
    #[must_use]
    pub fn new(spi: SPI, busy: BUSY, rst: RST, dc: DC, delay: DELAY) -> Self {
        Self {
            spi,
            busy,
            rst,
            dc,
            delay,
            individual_writes: false,
        }
    }

    /// Create a new display, writing individual bytes to SPI
    #[must_use]
    pub fn new_with_individual_writes(
        spi: SPI,
        busy: BUSY,
        rst: RST,
        dc: DC,
        delay: DELAY,
    ) -> Self {
        Self {
            spi,
            busy,
            rst,
            dc,
            delay,
            individual_writes: true,
        }
    }

    /// Initialize display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn initialize(&mut self) -> Result<(), Error> {
        debug!("Initialize display");

        self.hardware_reset().await?;
        self.software_reset().await?;
        self.set_driver_output_control().await?;
        self.set_ram_size(WIDTH, HEIGHT).await?;
        self.set_border_waveform_control().await?;
        self.set_ram_address_counters().await?;

        self.wait_until_idle().await?;
        debug!("Initialize display / Done");

        Ok(())
    }

    /// Set RAM address counters
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    async fn set_ram_address_counters(&mut self) -> Result<(), Error> {
        debug!("Set RAM address counters");
        self.send_command(command::SET_RAM_X_ADDRESS_COUNTER)
            .await?;
        self.send_data(&[0x00]).await?;
        self.send_command(command::SET_RAM_Y_ADDRESS_COUNTER)
            .await?;
        self.send_data(&[0xc7]).await?;
        self.send_data(&[0x00]).await?;
        debug!("Set RAM address counters / done");

        Ok(())
    }

    /// Set border waveform control
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    async fn set_border_waveform_control(&mut self) -> Result<(), Error> {
        debug!("Set border waveform control");
        self.send_command(command::BORDER_WAVEFORM_CONTROL).await?;
        self.send_data(&[0x05]).await?;
        debug!("Set border waveform control / done");

        Ok(())
    }

    /// Set driver output control
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    async fn set_driver_output_control(&mut self) -> Result<(), Error> {
        debug!("Set driver output control");
        self.wait_until_idle().await?;
        self.send_command(command::DRIVER_OUTPUT_CONTROL).await?;
        self.send_data(&[0xc7, 0x00, 0x01]).await?;
        debug!("Set driver output control / done");

        Ok(())
    }

    /// Set RAM size
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    #[allow(clippy::cast_possible_truncation, clippy::panic_in_result_fn)]
    async fn set_ram_size(&mut self, width: usize, height: usize) -> Result<(), Error> {
        debug!("Set RAM size");
        self.send_command(command::DATA_ENTRY_MODE).await?;
        self.send_data(&[0x01]).await?;

        let x_start = 0;
        let x_end = width as u8 / 8 - 1;

        assert_eq!(x_start, 0x00);
        assert_eq!(x_end, 0x18); // 0x18 = 24 = 200 / 8 - 1

        self.send_command(command::SET_RAM_X_ADDRESS_START_END_POSITION)
            .await?;
        self.send_data(&[x_start, x_end]).await?;

        let y_start = height as u16 - 1;
        let y_end = 0_u16;

        assert_eq!(y_start, 0xc7);
        assert_eq!(y_end, 0x00);

        let [y_start_0, y_start_1] = y_start.to_le_bytes();
        let [y_end_0, y_end_1] = y_end.to_le_bytes();

        assert_eq!(y_start_0, 0xc7); // 0xC7 = 199 = 200 - 1
        assert_eq!(y_start_1, 0x00);

        assert_eq!(y_end_0, 0x00);
        assert_eq!(y_end_1, 0x00);

        self.send_command(command::SET_RAM_Y_ADDRESS_START_END_POSITION)
            .await?;
        self.send_data(&[y_start_0, y_start_1, y_end_0, y_end_1])
            .await?;
        debug!("Set RAM size / done");

        Ok(())
    }

    /// Clear display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn clear(&mut self) -> Result<(), Error> {
        debug!("Clear display");
        let linewidth = WIDTH / 8;

        self.send_command(command::WRITE_RAM_BLACK).await?;
        for _ in 0..linewidth {
            for _ in 0..HEIGHT {
                self.send_data(&[0xff]).await?;
            }
        }

        self.send_command(command::WRITE_RAM_CHROMATIC).await?;
        for _ in 0..linewidth {
            for _ in 0..HEIGHT {
                self.send_data(&[0x00]).await?;
            }
        }

        self.refresh().await?;
        debug!("Clear display / Done");

        Ok(())
    }

    #[cfg(feature = "draw-target")]
    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn draw_buffer(
        &mut self,
        buffer: &Buffer<WIDTH, HEIGHT, BYTE_SIZE>,
    ) -> Result<(), Error> {
        debug!("Update display");

        self.transfer_black(buffer.black_buffer()).await?;
        self.transfer_chromatic(buffer.chromatic_buffer()).await?;

        self.refresh().await?;
        debug!("Update display / Done");
        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn transfer_channels(
        &mut self,
        black: Option<&[u8]>,
        chromatic: Option<&[u8]>,
    ) -> Result<(), Error> {
        debug!("Update display");

        if let Some(black) = black {
            self.transfer_black(black).await?;
        }

        if let Some(chromatic) = chromatic {
            self.transfer_chromatic(chromatic).await?;
        }

        self.refresh().await?;
        debug!("Update display / Done");
        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn transfer_chromatic(&mut self, chromatic: &[u8]) -> Result<(), Error> {
        /// Line width
        const LINEWIDTH: usize = WIDTH / 8;

        debug!("Transfer chromatic data");
        self.send_command(command::WRITE_RAM_CHROMATIC).await?;

        trace!("Compute inverse of chromatic data");
        let mut buffer = [0x00; (HEIGHT * LINEWIDTH)];
        for (byte, chromatic) in &mut buffer.iter_mut().zip(chromatic.iter()) {
            *byte = !chromatic;
        }
        self.send_data(&buffer).await?;

        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn transfer_black(&mut self, black: &[u8]) -> Result<(), Error> {
        debug!("Transfer black data");
        self.send_command(command::WRITE_RAM_BLACK).await?;
        self.send_data(black).await?;

        Ok(())
    }

    /// Release display and return inner hardware
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub async fn release(mut self) -> Result<(SPI, BUSY, RST, DC), Error> {
        debug!("Release display");
        self.send_command(command::DEEP_SLEEP_MODE).await?;
        self.send_data(&[0x01]).await?;

        self.delay.delay_ms(200).await;
        debug!("Release display / Done");

        Ok((self.spi, self.busy, self.rst, self.dc))
    }

    /// Refresh the display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    async fn refresh(&mut self) -> Result<(), Error> {
        debug!("Refresh display");
        self.send_command(command::DISPLAY_UPDATE_CONTROL_2).await?;
        self.send_data(&[0xf7]).await?;

        self.send_command(command::MASTER_ACTIVATION).await?;

        self.wait_until_idle().await?;

        debug!("Refresh display / Done");

        Ok(())
    }

    /// Send a reset command to the display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    async fn software_reset(&mut self) -> Result<(), Error> {
        debug!("Software reset");
        self.wait_until_idle().await?;
        self.send_command(command::SOFTWARE_RESET).await?;
        debug!("Software reset / done");

        Ok(())
    }

    /// Send command over SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    async fn send_command(&mut self, command: u8) -> Result<(), Error> {
        trace!("Set DC to low for transferring commands");
        self.dc.set_low().map_err(Error::from_digital)?;

        self.write(&[command]).await
    }

    /// Send data over SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    async fn send_data(&mut self, data: &[u8]) -> Result<(), Error> {
        // trace!("Set DC to high for transferring data");
        self.dc.set_high().map_err(Error::from_digital)?;

        self.write(data).await
    }

    /// Write data to SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    async fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if log_enabled!(Trace) {
            trace!("Write {} bytes to SPI", data.len());
        }

        // Linux has a default limit of 4096 bytes per SPI transfer
        // see https://raspberrypi.stackexchange.com/questions/65595/spi-transfer-fails-with-buffer-size-greater-than-4096
        if cfg!(target_os = "linux") {
            trace!("Write bytes in chunks of 4096 bytes");
            for data_chunk in data.chunks(4096) {
                self.spi.write(data_chunk).await?;
            }
        } else if self.individual_writes {
            for datum in data {
                self.spi.write(&[*datum]).await?;
            }
        } else {
            self.spi.write(data).await?;
        }

        Ok(())
    }

    /// Wait while the display is busy
    ///
    /// # Errors
    ///
    /// Returns an error if reading the busy pin fails.
    async fn wait_until_idle(&mut self) -> Result<(), Error> {
        if IS_BUSY_LOW {
            self.busy.wait_for_high().await.map_err(Error::from_digital)
        } else {
            self.busy.wait_for_low().await.map_err(Error::from_digital)
        }
    }

    /// Reset the display
    ///
    /// # Errors
    ///
    /// Returns an error if setting any pin fails.
    async fn hardware_reset(&mut self) -> Result<(), Error> {
        debug!("Hardware reset");
        trace!("Set RST high");
        self.rst.set_high().map_err(Error::from_digital)?;
        self.delay.delay_ms(10).await;

        trace!("Set RST low");
        self.rst.set_low().map_err(Error::from_digital)?;
        self.delay.delay_ms(10).await;

        trace!("Set RST high");
        self.rst.set_high().map_err(Error::from_digital)?;

        self.delay.delay_ms(200).await;
        debug!("Hardware reset / done");

        Ok(())
    }
}
