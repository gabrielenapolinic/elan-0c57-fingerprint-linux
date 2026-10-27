// SPDX-License-Identifier: LGPL-2.1-or-later

//! Wire protocol of the ELAN image sensors (libfprint `elan` driver).
//!
//! Every command is two bytes written to [`EP_CMD_OUT`]. Most replies come
//! back on [`EP_CMD_IN`]; image data comes on [`EP_IMG_IN`] as 16 bit
//! little-endian pixels, column by column (the frame is transposed).

use std::time::Duration;

pub const VENDOR_ID: u16 = 0x04f3;
pub const PRODUCT_ID: u16 = 0x0c57;

pub const EP_CMD_OUT: u8 = 0x01;
pub const EP_CMD_IN: u8 = 0x83;
pub const EP_IMG_IN: u8 = 0x82;

pub const CMD_TIMEOUT: Duration = Duration::from_millis(10_000);
/// Timeout used while reading further frames: a timeout means the finger left.
pub const FINGER_TIMEOUT: Duration = Duration::from_millis(200);

/// How often a cancellable wait checks whether it was cancelled.
pub const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Frames read per touch; the last one is the sharpest.
pub const PRESS_FRAMES: usize = 3;
pub const CALIBRATION_MAX_DELTA: u32 = 500;
pub const CALIBRATION_ATTEMPTS: u32 = 10;
pub const FINGER_OFF_ATTEMPTS: u32 = 50;
pub const FINGER_OFF_DELAY: Duration = Duration::from_millis(100);
pub const CALIBRATION_STATUS_DELAY: Duration = Duration::from_millis(50);
/// Oldest firmware that understands the calibration commands.
pub const MIN_CALIBRATION_FW: u16 = 0x0138;

/// Reply byte of [`Command::WaitFinger`] when a finger is on the sensor.
pub const FINGER_PRESENT: u8 = 0x55;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    FirmwareVersion,
    SensorDimensions,
    ReadImage,
    CalibrationStatus,
    CalibrationMean,
    LedOn,
    /// Blocks until a finger touches the sensor.
    WaitFinger,
    /// Blocks until the finger is lifted (used by the ELAN Windows/TOD driver).
    WaitFingerOff,
    /// Ends a capture and switches the LED off.
    Stop,
}

impl Command {
    pub fn bytes(self) -> [u8; 2] {
        match self {
            Command::FirmwareVersion => [0x40, 0x19],
            Command::SensorDimensions => [0x00, 0x0c],
            Command::ReadImage => [0x00, 0x09],
            Command::CalibrationStatus => [0x40, 0x23],
            Command::CalibrationMean => [0x40, 0x24],
            Command::LedOn => [0x40, 0x31],
            Command::WaitFinger => [0x40, 0x3f],
            Command::WaitFingerOff => [0x40, 0x3e],
            Command::Stop => [0x00, 0x0b],
        }
    }

    /// Length of the reply on [`EP_CMD_IN`]; `0` means no reply.
    /// [`Command::ReadImage`] replies on [`EP_IMG_IN`] with a size that
    /// depends on the sensor, see [`Dimensions::raw_len`].
    pub fn reply_len(self) -> usize {
        match self {
            Command::FirmwareVersion | Command::CalibrationMean => 2,
            Command::SensorDimensions => 4,
            Command::CalibrationStatus | Command::WaitFinger | Command::WaitFingerOff => 1,
            Command::ReadImage | Command::LedOn | Command::Stop => 0,
        }
    }
}

pub fn parse_firmware(reply: &[u8]) -> u16 {
    u16::from(reply[0]) << 8 | u16::from(reply[1])
}

/// The calibration mean, computed exactly like the libfprint driver (`* 0xff`, not `<< 8`).
pub fn parse_calibration_mean(reply: &[u8]) -> u32 {
    u32::from(reply[0]) * 0xff + u32::from(reply[1])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: usize,
    pub height: usize,
}

impl Dimensions {
    /// Parses the reply of [`Command::SensorDimensions`] for a rotated sensor
    /// (all of them except the 0c03).
    pub fn parse(reply: &[u8]) -> Self {
        let (mut width, mut height) = (usize::from(reply[2]), usize::from(reply[0]));
        // Some sensors report the last index instead of the size.
        if width % 2 == 1 && height % 2 == 1 {
            width += 1;
            height += 1;
        }
        Dimensions { width, height }
    }

    pub fn pixels(self) -> usize {
        self.width * self.height
    }

    /// Bytes of one raw image (16 bit pixels).
    pub fn raw_len(self) -> usize {
        self.pixels() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_of_the_0c57() {
        let dims = Dimensions::parse(&[80, 0, 80, 0]);
        assert_eq!(dims, Dimensions { width: 80, height: 80 });
        assert_eq!(dims.raw_len(), 12_800);
    }

    #[test]
    fn zero_based_dimensions_are_fixed() {
        assert_eq!(Dimensions::parse(&[79, 0, 79, 0]), Dimensions { width: 80, height: 80 });
    }

    #[test]
    fn firmware_and_mean() {
        assert_eq!(parse_firmware(&[0x02, 0x16]), 0x0216);
        assert_eq!(parse_calibration_mean(&[0x10, 0x20]), 0x10 * 0xff + 0x20);
    }
}
