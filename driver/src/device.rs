// SPDX-License-Identifier: LGPL-2.1-or-later

//! The sensor: opening, calibration and capture of one touch.
//!
//! The logic follows libfprint's `elan` driver with the press-mode patch
//! (see `docs/TECHNICAL.md` of the elan-0c57-fingerprint-linux package).

use std::fmt;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::i18n::{tr, trf};
use crate::image::Frame;
use crate::protocol::{self, Command, Dimensions};

#[derive(Debug)]
pub enum Error {
    /// A transfer did not complete in time.
    Timeout,
    /// The sensor answered something unexpected.
    Protocol(String),
    /// The finger was already on the sensor when the background was taken.
    FingerDuringCalibration,
    Calibration,
    /// The caller asked to stop waiting.
    Cancelled,
    Usb(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Timeout => f.write_str(tr("the sensor did not answer in time")),
            Error::Protocol(msg) => f.write_str(&trf("unexpected reply from the sensor: {msg}", &[("msg", msg)])),
            Error::FingerDuringCalibration => f.write_str(tr(
                "the image is darker than the background: was the finger on the sensor during calibration?",
            )),
            Error::Calibration => f.write_str(tr("calibration failed")),
            Error::Cancelled => f.write_str(tr("cancelled")),
            Error::Usb(msg) => f.write_str(&trf("USB error: {msg}", &[("msg", msg)])),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Bulk transfers to and from the sensor. Implemented by [`UsbTransport`],
/// and by a fake device in the tests.
pub trait Transport {
    fn write(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()>;
    fn read(&mut self, endpoint: u8, len: usize, timeout: Duration) -> Result<Vec<u8>>;
}

pub struct UsbTransport {
    handle: rusb::DeviceHandle<rusb::GlobalContext>,
}

impl UsbTransport {
    /// Opens the first ELAN 0c57 and claims its interface.
    pub fn open() -> Result<Self> {
        let handle = rusb::open_device_with_vid_pid(protocol::VENDOR_ID, protocol::PRODUCT_ID)
            .ok_or_else(|| {
                Error::Usb(
                    tr("no 04f3:0c57 reader found, or no permission to open it (install udev/71-elan-0c57-uaccess.rules)")
                        .into(),
                )
            })?;
        handle.claim_interface(0).map_err(|e| match e {
            rusb::Error::Busy => Error::Usb(tr("the reader is in use (by fprintd?)").into()),
            e => usb_error(e),
        })?;
        Ok(UsbTransport { handle })
    }
}

impl Drop for UsbTransport {
    fn drop(&mut self) {
        let _ = self.handle.release_interface(0);
    }
}

fn usb_error(e: rusb::Error) -> Error {
    match e {
        rusb::Error::Timeout => Error::Timeout,
        e => Error::Usb(e.to_string()),
    }
}

impl Transport for UsbTransport {
    fn write(&mut self, endpoint: u8, data: &[u8], timeout: Duration) -> Result<()> {
        let n = self.handle.write_bulk(endpoint, data, timeout).map_err(usb_error)?;
        if n != data.len() {
            return Err(Error::Usb(format!("short write: {n} of {} bytes", data.len())));
        }
        Ok(())
    }

    fn read(&mut self, endpoint: u8, len: usize, timeout: Duration) -> Result<Vec<u8>> {
        let mut buf = vec![0; len];
        let n = self.handle.read_bulk(endpoint, &mut buf, timeout).map_err(usb_error)?;
        if n != len {
            return Err(Error::Protocol(format!("short read on 0x{endpoint:02x}: {n} of {len} bytes")));
        }
        Ok(buf)
    }
}

/// One touch, as read from the sensor.
#[derive(Debug, Clone)]
pub struct Touch {
    /// Image without finger, taken during calibration.
    pub background: Frame,
    /// Last frame read while the finger was on the sensor (raw).
    pub frame: Frame,
    /// `frame` minus `background`: the fingerprint.
    pub print: Frame,
}

pub struct Sensor<T: Transport> {
    transport: T,
    pub firmware: u16,
    pub dims: Dimensions,
}

impl Sensor<UsbTransport> {
    pub fn open_usb() -> Result<Self> {
        Sensor::new(UsbTransport::open()?)
    }
}

impl<T: Transport> Sensor<T> {
    /// Reads the firmware version and the sensor size.
    pub fn new(mut transport: T) -> Result<Self> {
        let firmware = protocol::parse_firmware(&command(&mut transport, Command::FirmwareVersion, protocol::CMD_TIMEOUT)?);
        let dims = Dimensions::parse(&command(&mut transport, Command::SensorDimensions, protocol::CMD_TIMEOUT)?);
        if dims.pixels() == 0 {
            return Err(Error::Protocol(format!("sensor size {}x{}", dims.width, dims.height)));
        }
        Ok(Sensor { transport, firmware, dims })
    }

    fn run(&mut self, cmd: Command, timeout: Duration) -> Result<Vec<u8>> {
        command(&mut self.transport, cmd, timeout)
    }

    pub fn read_image(&mut self) -> Result<Frame> {
        self.transport.write(protocol::EP_CMD_OUT, &Command::ReadImage.bytes(), protocol::CMD_TIMEOUT)?;
        let raw = self.transport.read(protocol::EP_IMG_IN, self.dims.raw_len(), protocol::CMD_TIMEOUT)?;
        Ok(Frame::from_raw(&raw, self.dims))
    }

    pub fn calibration_mean(&mut self) -> Result<u32> {
        Ok(protocol::parse_calibration_mean(&self.run(Command::CalibrationMean, protocol::CMD_TIMEOUT)?))
    }

    pub fn led_on(&mut self) -> Result<()> {
        self.run(Command::LedOn, protocol::CMD_TIMEOUT).map(drop)
    }

    /// Ends the capture; also switches the LED off.
    pub fn stop(&mut self) -> Result<()> {
        self.run(Command::Stop, protocol::CMD_TIMEOUT).map(drop)
    }

    /// Takes the background and recalibrates the sensor if it drifted.
    /// Returns the background (without finger).
    pub fn calibrate(&mut self) -> Result<Frame> {
        let mut finger_off_left = protocol::FINGER_OFF_ATTEMPTS;
        let mut calibration_left = protocol::CALIBRATION_ATTEMPTS;
        loop {
            let background = self.read_image()?;
            if self.firmware < protocol::MIN_CALIBRATION_FW {
                return Ok(background);
            }
            let mean = self.calibration_mean()?;
            let bg_mean = background.mean();
            if bg_mean.abs_diff(mean) <= protocol::CALIBRATION_MAX_DELTA {
                return Ok(background);
            }
            // A background much brighter than the mean: the finger is still on
            // the sensor. An empty one is a bad read. Either way, read again.
            let finger_on = bg_mean == 0 || bg_mean > mean + protocol::CALIBRATION_MAX_DELTA;
            if finger_on && finger_off_left > 0 {
                finger_off_left -= 1;
                sleep(protocol::FINGER_OFF_DELAY);
                continue;
            }
            // 0x01 = calibrating, 0x03 = done. Right after the request the sensor
            // may still say 0x03, so a 0x01 must be seen first.
            let mut seen_running = false;
            loop {
                calibration_left -= 1;
                if calibration_left == 0 {
                    return Err(Error::Calibration);
                }
                let status = self.run(Command::CalibrationStatus, protocol::CMD_TIMEOUT)?[0];
                if seen_running && status == 0x03 {
                    break;
                }
                seen_running |= status == 0x01;
                sleep(protocol::CALIBRATION_STATUS_DELAY);
            }
        }
    }

    /// Blocks until a finger is on the sensor, or `timeout` expires.
    pub fn wait_finger(&mut self, timeout: Duration) -> Result<()> {
        finger_status(self.run(Command::WaitFinger, timeout)?[0])
    }

    /// Like [`Sensor::wait_finger`], but without a time limit (`timeout` None)
    /// and checking `cancel` twice a second.
    pub fn wait_finger_cancellable(&mut self, timeout: Option<Duration>, cancel: &dyn Fn() -> bool) -> Result<()> {
        let start = Instant::now();
        self.transport.write(protocol::EP_CMD_OUT, &Command::WaitFinger.bytes(), protocol::CMD_TIMEOUT)?;
        loop {
            // The command stays pending on the sensor: just read its reply again.
            match self.transport.read(protocol::EP_CMD_IN, 1, protocol::POLL_INTERVAL) {
                Ok(reply) => return finger_status(reply[0]),
                Err(Error::Timeout) if cancel() => return Err(Error::Cancelled),
                Err(Error::Timeout) if timeout.is_some_and(|t| start.elapsed() >= t) => return Err(Error::Timeout),
                Err(Error::Timeout) => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Blocks until the finger is lifted, or `timeout` expires.
    pub fn wait_finger_off(&mut self, timeout: Duration) -> Result<()> {
        self.run(Command::WaitFingerOff, timeout).map(drop)
    }

    /// Calibrates, waits for a finger (up to `timeout`) and reads one touch.
    /// The LED is on while waiting. Always ends with [`Command::Stop`].
    pub fn capture(&mut self, timeout: Duration) -> Result<Touch> {
        self.capture_cancellable(Some(timeout), &|| false)
    }

    /// Like [`Sensor::capture`], with no time limit if `timeout` is None, and
    /// returning [`Error::Cancelled`] as soon as `cancel` returns true.
    pub fn capture_cancellable(&mut self, timeout: Option<Duration>, cancel: &dyn Fn() -> bool) -> Result<Touch> {
        let result = self.capture_inner(timeout, cancel);
        let stopped = self.stop();
        let touch = result?;
        stopped?;
        Ok(touch)
    }

    fn capture_inner(&mut self, timeout: Option<Duration>, cancel: &dyn Fn() -> bool) -> Result<Touch> {
        let mut spurious_left = protocol::SPURIOUS_WAKE_ATTEMPTS;
        let background = loop {
            let background = self.calibrate()?;
            self.led_on()?;
            match self.wait_finger_cancellable(timeout, cancel) {
                Ok(()) => break background,
                // A failed verify ends the login attempt (GDM and pam_fprintd
                // do not retry), so start over instead of giving up.
                Err(Error::Protocol(_)) if spurious_left > 0 && !cancel() => {
                    spurious_left -= 1;
                    self.stop()?;
                    sleep(protocol::SPURIOUS_WAKE_DELAY);
                }
                Err(e) => return Err(e),
            }
        };
        let mut frame = self.read_image()?;
        for _ in 1..protocol::PRESS_FRAMES {
            // The finger may already be gone: keep the last good frame.
            match self.wait_finger(protocol::FINGER_TIMEOUT) {
                Ok(()) => frame = self.read_image()?,
                Err(Error::Timeout) => break,
                Err(e) => return Err(e),
            }
        }
        let print = frame.minus_background(&background).ok_or(Error::FingerDuringCalibration)?;
        Ok(Touch { background, frame, print })
    }
}

fn finger_status(reply: u8) -> Result<()> {
    match reply {
        protocol::FINGER_PRESENT => Ok(()),
        0xff => Err(Error::Protocol("sensor not calibrated (0xff)".into())),
        other => Err(Error::Protocol(format!("finger status 0x{other:02x}"))),
    }
}

/// Sends `cmd` and reads its reply (if it has one).
fn command<T: Transport>(transport: &mut T, cmd: Command, timeout: Duration) -> Result<Vec<u8>> {
    transport.write(protocol::EP_CMD_OUT, &cmd.bytes(), timeout)?;
    match cmd.reply_len() {
        0 => Ok(Vec::new()),
        len => transport.read(protocol::EP_CMD_IN, len, timeout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// A scripted sensor: answers each command from a queue.
    #[derive(Default)]
    struct Fake {
        sent: Vec<[u8; 2]>,
        last: Option<[u8; 2]>,
        means: VecDeque<u32>,
        statuses: VecDeque<u8>,
        images: VecDeque<u16>,
        fingers: VecDeque<Option<u8>>,
    }

    impl Transport for Fake {
        fn write(&mut self, _: u8, data: &[u8], _: Duration) -> Result<()> {
            let cmd = [data[0], data[1]];
            self.sent.push(cmd);
            self.last = Some(cmd);
            Ok(())
        }

        fn read(&mut self, _: u8, len: usize, _: Duration) -> Result<Vec<u8>> {
            Ok(match self.last.unwrap() {
                [0x40, 0x19] => vec![0x02, 0x16],
                [0x00, 0x0c] => vec![4, 0, 4, 0],
                [0x40, 0x24] => {
                    let m = self.means.pop_front().unwrap();
                    vec![(m / 0xff) as u8, (m % 0xff) as u8]
                }
                [0x40, 0x23] => vec![self.statuses.pop_front().unwrap()],
                [0x40, 0x3f] => vec![self.fingers.pop_front().flatten().ok_or(Error::Timeout)?],
                [0x00, 0x09] => {
                    let v = self.images.pop_front().unwrap();
                    std::iter::repeat_n(v.to_le_bytes(), len / 2).flatten().collect()
                }
                other => panic!("unexpected read after {other:02x?}"),
            })
        }
    }

    fn sensor(fake: Fake) -> Sensor<Fake> {
        Sensor::new(fake).unwrap()
    }

    #[test]
    fn opens_and_reads_size() {
        let s = sensor(Fake::default());
        assert_eq!((s.firmware, s.dims), (0x0216, Dimensions { width: 4, height: 4 }));
    }

    #[test]
    fn no_calibration_when_background_matches() {
        let mut s = sensor(Fake { images: [3000].into(), means: [3100].into(), ..Default::default() });
        assert_eq!(s.calibrate().unwrap().mean(), 3000);
        assert!(!s.transport.sent.contains(&[0x40, 0x23]));
    }

    #[test]
    fn calibration_waits_for_running_then_done() {
        let mut s = sensor(Fake {
            images: [1000, 3000].into(),
            means: [3000, 3000].into(),
            statuses: [0x03, 0x01, 0x01, 0x03].into(),
            ..Default::default()
        });
        assert_eq!(s.calibrate().unwrap().mean(), 3000);
        assert_eq!(s.transport.sent.iter().filter(|c| **c == [0x40, 0x23]).count(), 4);
    }

    #[test]
    fn calibration_gives_up() {
        let mut s = sensor(Fake {
            images: [1000].into(),
            means: [3000].into(),
            statuses: [0x01; 20].into(),
            ..Default::default()
        });
        assert!(matches!(s.calibrate(), Err(Error::Calibration)));
    }

    #[test]
    fn capture_keeps_last_frame_and_stops() {
        let mut s = sensor(Fake {
            images: [3000, 5000, 6000].into(),
            means: [3000].into(),
            fingers: [Some(0x55), Some(0x55), None].into(),
            ..Default::default()
        });
        let touch = s.capture(Duration::from_secs(1)).unwrap();
        assert_eq!(touch.print.pixels, vec![3000; 16]);
        assert_eq!(s.transport.sent.last(), Some(&[0x00, 0x0b]));
    }

    #[test]
    fn capture_retries_after_a_spurious_wake() {
        let mut s = sensor(Fake {
            images: [3000, 3000, 6000].into(),
            means: [3000, 3000].into(),
            fingers: [Some(0xca), Some(0x55), None].into(),
            ..Default::default()
        });
        let touch = s.capture(Duration::from_secs(1)).unwrap();
        assert_eq!(touch.print.pixels, vec![3000; 16]);
        assert_eq!(s.transport.sent.iter().filter(|c| **c == [0x00, 0x0b]).count(), 2);
    }

    #[test]
    fn capture_gives_up_after_repeated_spurious_wakes() {
        let n = protocol::SPURIOUS_WAKE_ATTEMPTS as usize + 1;
        let mut s = sensor(Fake {
            images: vec![3000; n].into(),
            means: vec![3000; n].into(),
            fingers: vec![Some(0xca); n].into(),
            ..Default::default()
        });
        assert!(matches!(s.capture(Duration::from_secs(1)), Err(Error::Protocol(_))));
    }

    #[test]
    fn capture_stops_even_on_error() {
        let mut s = sensor(Fake {
            images: [3000].into(),
            means: [3000].into(),
            fingers: [None].into(),
            ..Default::default()
        });
        assert!(matches!(s.capture(Duration::ZERO), Err(Error::Timeout)));
        assert_eq!(s.transport.sent.last(), Some(&[0x00, 0x0b]));
    }

    #[test]
    fn waiting_for_the_finger_can_be_cancelled() {
        let mut s = sensor(Fake { images: [3000].into(), means: [3000].into(), ..Default::default() });
        assert!(matches!(s.capture_cancellable(None, &|| true), Err(Error::Cancelled)));
        assert_eq!(s.transport.sent.last(), Some(&[0x00, 0x0b]));
    }
}
