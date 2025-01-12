// Copyright Claudio Mattera 2024-2025.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files LICENSE-MIT.txt and LICENSE-APACHE-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Blocking display

use log::debug;
use log::log_enabled;
use log::trace;
use log::Level::Trace;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::InputPin;
use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiDevice;

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
pub struct Display<SPI: SpiDevice, BUSY: InputPin, RST: OutputPin, DC: OutputPin, DELAY: DelayNs> {
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
}

impl<SPI, BUSY, RST, DC, DELAY> Display<SPI, BUSY, RST, DC, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin,
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
        }
    }

    /// Initialize display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn initialize(&mut self) -> Result<(), Error> {
        debug!("Initialize display");

        self.hardware_reset()?;
        self.software_reset()?;
        self.set_driver_output_control()?;
        self.set_ram_size(WIDTH, HEIGHT)?;
        self.set_border_waveform_control()?;
        self.set_ram_address_counters()?;

        self.wait_until_idle()?;
        debug!("Initialize display / Done");

        Ok(())
    }

    /// Set RAM address counters
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    fn set_ram_address_counters(&mut self) -> Result<(), Error> {
        self.send_command(command::SET_RAM_X_ADDRESS_COUNTER)?;
        self.send_data(&[0x00])?;
        self.send_command(command::SET_RAM_Y_ADDRESS_COUNTER)?;
        self.send_data(&[0xc7])?;
        self.send_data(&[0x00])?;

        Ok(())
    }

    /// Set border waveform control
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    fn set_border_waveform_control(&mut self) -> Result<(), Error> {
        self.send_command(command::BORDER_WAVEFORM_CONTROL)?;
        self.send_data(&[0x05])?;

        Ok(())
    }

    /// Set driver output control
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    fn set_driver_output_control(&mut self) -> Result<(), Error> {
        self.wait_until_idle()?;
        self.send_command(command::DRIVER_OUTPUT_CONTROL)?;
        self.send_data(&[0xc7, 0x00, 0x01])?;

        Ok(())
    }

    /// Set RAM size
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    #[allow(clippy::cast_possible_truncation, clippy::panic_in_result_fn)]
    fn set_ram_size(&mut self, width: usize, height: usize) -> Result<(), Error> {
        self.send_command(command::DATA_ENTRY_MODE)?;
        self.send_data(&[0x01])?;

        let x_start = 0;
        let x_end = width as u8 / 8 - 1;

        assert_eq!(x_start, 0x00);
        assert_eq!(x_end, 0x18); // 0x18 = 24 = 200 / 8 - 1

        self.send_command(command::SET_RAM_X_ADDRESS_START_END_POSITION)?;
        self.send_data(&[x_start, x_end])?;

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

        self.send_command(command::SET_RAM_Y_ADDRESS_START_END_POSITION)?;
        self.send_data(&[y_start_0, y_start_1, y_end_0, y_end_1])?;

        Ok(())
    }

    /// Clear display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn clear(&mut self) -> Result<(), Error> {
        debug!("Clear display");
        let linewidth = WIDTH / 8;

        self.send_command(command::WRITE_RAM_BLACK)?;
        for _ in 0..linewidth {
            for _ in 0..HEIGHT {
                self.send_data(&[0xff])?;
            }
        }

        self.send_command(command::WRITE_RAM_CHROMATIC)?;
        for _ in 0..linewidth {
            for _ in 0..HEIGHT {
                self.send_data(&[0x00])?;
            }
        }

        self.refresh()?;
        debug!("Clear display / Done");

        Ok(())
    }

    #[cfg(feature = "draw-target")]
    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn draw_buffer(&mut self, buffer: &Buffer<WIDTH, HEIGHT, BYTE_SIZE>) -> Result<(), Error> {
        debug!("Update display");

        self.transfer_black(buffer.black_buffer())?;
        self.transfer_chromatic(buffer.chromatic_buffer())?;

        self.refresh()?;
        debug!("Update display / Done");
        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn transfer_channels(
        &mut self,
        black: Option<&[u8]>,
        chromatic: Option<&[u8]>,
    ) -> Result<(), Error> {
        debug!("Update display");

        if let Some(black) = black {
            self.transfer_black(black)?;
        }

        if let Some(chromatic) = chromatic {
            self.transfer_chromatic(chromatic)?;
        }

        self.refresh()?;
        debug!("Update display / Done");
        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn transfer_chromatic(&mut self, chromatic: &[u8]) -> Result<(), Error> {
        /// Line width
        const LINEWIDTH: usize = WIDTH / 8;

        debug!("Transfer chromatic data");
        self.send_command(command::WRITE_RAM_CHROMATIC)?;

        trace!("Compute inverse of chromatic data");
        let mut buffer = [0x00; (HEIGHT * LINEWIDTH)];
        for (byte, chromatic) in &mut buffer.iter_mut().zip(chromatic.iter()) {
            *byte = !chromatic;
        }
        self.send_data(&buffer)?;

        Ok(())
    }

    ///
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn transfer_black(&mut self, black: &[u8]) -> Result<(), Error> {
        debug!("Transfer black data");
        self.send_command(command::WRITE_RAM_BLACK)?;
        self.send_data(black)?;

        Ok(())
    }

    /// Release display and return inner hardware
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    pub fn release(mut self) -> Result<(SPI, BUSY, RST, DC), Error> {
        debug!("Release display");
        self.send_command(command::DEEP_SLEEP_MODE)?;
        self.send_data(&[0x01])?;

        self.delay.delay_ms(200);
        debug!("Release display / Done");

        Ok((self.spi, self.busy, self.rst, self.dc))
    }

    /// Refresh the display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    fn refresh(&mut self) -> Result<(), Error> {
        debug!("Refresh display");
        self.send_command(command::DISPLAY_UPDATE_CONTROL_2)?;
        self.send_data(&[0xf7])?;

        self.send_command(command::MASTER_ACTIVATION)?;

        self.wait_until_idle()?;

        debug!("Refresh display / Done");

        Ok(())
    }

    /// Send a reset command to the display
    ///
    /// # Errors
    ///
    /// Returns an error if any commands to the display fails
    fn software_reset(&mut self) -> Result<(), Error> {
        debug!("Software reset");
        self.wait_until_idle()?;
        self.send_command(command::SOFTWARE_RESET)?;
        debug!("Software reset / done");

        Ok(())
    }

    /// Send command over SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    fn send_command(&mut self, command: u8) -> Result<(), Error> {
        trace!("Set DC to low for transferring commands");
        self.dc.set_low().map_err(Error::from_digital)?;

        self.write(&[command])
    }

    /// Send data over SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    fn send_data(&mut self, data: &[u8]) -> Result<(), Error> {
        trace!("Set DC to high for transferring data");
        self.dc.set_high().map_err(Error::from_digital)?;

        self.write(data)
    }

    /// Write data to SPI bus
    ///
    /// # Errors
    ///
    /// Returns an error if writing to SPI bus fails.
    fn write(&mut self, data: &[u8]) -> Result<(), Error> {
        if log_enabled!(Trace) {
            trace!("Write {} bytes to SPI", data.len());
        }

        // Linux has a default limit of 4096 bytes per SPI transfer
        // see https://raspberrypi.stackexchange.com/questions/65595/spi-transfer-fails-with-buffer-size-greater-than-4096
        if cfg!(target_os = "linux") {
            for data_chunk in data.chunks(4096) {
                self.spi.write(data_chunk)?;
            }
        } else {
            self.spi.write(data)?;
        }

        Ok(())
    }

    /// Wait while the display is busy
    ///
    /// # Errors
    ///
    /// Returns an error if reading the busy pin fails.
    fn wait_until_idle(&mut self) -> Result<(), Error> {
        while self.is_busy(IS_BUSY_LOW)? {
            self.delay.delay_ms(10);
        }
        Ok(())
    }

    /// Check if the display is busy
    ///
    /// # Errors
    ///
    /// Returns an error if reading the busy pin fails.
    fn is_busy(&mut self, is_busy_low: bool) -> Result<bool, Error> {
        let is_busy = (is_busy_low && self.busy.is_low().map_err(Error::from_digital)?)
            || (!is_busy_low && self.busy.is_high().map_err(Error::from_digital)?);
        Ok(is_busy)
    }

    /// Reset the display
    ///
    /// # Errors
    ///
    /// Returns an error if setting any pin fails.
    fn hardware_reset(&mut self) -> Result<(), Error> {
        debug!("Hardware reset");
        self.rst.set_high().map_err(Error::from_digital)?;

        trace!("Set RST high");
        self.delay.delay_ms(10);

        trace!("Set RST low");
        self.rst.set_low().map_err(Error::from_digital)?;
        self.delay.delay_ms(10);

        trace!("Set RST high");
        self.rst.set_high().map_err(Error::from_digital)?;

        self.delay.delay_ms(200);
        debug!("Hardware reset / done");

        Ok(())
    }
}
