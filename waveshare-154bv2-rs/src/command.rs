// Copyright Claudio Mattera 2024.
//
// Distributed under the MIT License or the Apache 2.0 License at your option.
// See the accompanying files License-MIT.txt and License-Apache-2.0.txt, or
// online at
// https://opensource.org/licenses/MIT
// https://opensource.org/licenses/Apache-2.0

//! Commands

/// Command for driver output control
pub const DRIVER_OUTPUT_CONTROL: u8 = 0x01;

/// Command for deep sleep mode
pub const DEEP_SLEEP_MODE: u8 = 0x10;

/// Command for data entry mode
pub const DATA_ENTRY_MODE: u8 = 0x11;

/// Command for software reset
pub const SOFTWARE_RESET: u8 = 0x12;

/// Command for master activation
pub const MASTER_ACTIVATION: u8 = 0x20;

/// Command for display update control 2
pub const DISPLAY_UPDATE_CONTROL_2: u8 = 0x22;

/// Command for write RAM black
pub const WRITE_RAM_BLACK: u8 = 0x24;

/// Command for write RAM chromatic
pub const WRITE_RAM_CHROMATIC: u8 = 0x26;

/// Command for border waveform control
pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;

/// Command for setting RAM X address start and end position
pub const SET_RAM_X_ADDRESS_START_END_POSITION: u8 = 0x44;

/// Command for setting RAM Y address start and end position
pub const SET_RAM_Y_ADDRESS_START_END_POSITION: u8 = 0x45;

/// Command for setting RAM X address counter
pub const SET_RAM_X_ADDRESS_COUNTER: u8 = 0x4E;

/// Command for setting RAM Y address counter
pub const SET_RAM_Y_ADDRESS_COUNTER: u8 = 0x4F;
