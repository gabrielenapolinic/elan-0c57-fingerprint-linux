// SPDX-License-Identifier: LGPL-2.1-or-later

//! C interface, used by the libfprint-TOD driver in `tod/`.
//!
//! Every call is blocking and meant to run in a worker thread. Buffers
//! returned by the library are freed with [`elan0c57_free`], strings with
//! [`elan0c57_free_string`].

use std::ffi::{CString, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::time::Duration;

use crate::device::{Error, Sensor, UsbTransport};
use crate::image::Frame;
use crate::matcher::{Matcher, Prepared, THRESHOLD};
use crate::print;

pub const ELAN0C57_OK: c_int = 0;
pub const ELAN0C57_CANCELLED: c_int = 1;
/// The finger covered only part of the sensor.
pub const ELAN0C57_PARTIAL: c_int = 2;
/// Bad touch (e.g. the finger was already down during calibration): try again.
pub const ELAN0C57_RETRY: c_int = 3;
pub const ELAN0C57_ERROR: c_int = -1;

/// Returns true when the current operation must stop.
pub type CancelFn = Option<unsafe extern "C" fn(user_data: *mut c_void) -> bool>;

pub struct Elan {
    sensor: Sensor<UsbTransport>,
    matcher: Matcher,
}

fn set_error(error: *mut *mut c_char, message: impl ToString) {
    if !error.is_null() {
        let text = CString::new(message.to_string().replace('\0', " ")).unwrap_or_default();
        unsafe { *error = text.into_raw() };
    }
}

fn give(data: Vec<u8>, out: *mut *mut u8, out_len: *mut usize) {
    let boxed = data.into_boxed_slice();
    unsafe {
        *out_len = boxed.len();
        *out = Box::into_raw(boxed).cast();
    }
}

unsafe fn bytes<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if data.is_null() { &[] } else { unsafe { std::slice::from_raw_parts(data, len) } }
}

/// Opens the reader. Returns NULL and sets `error` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn elan0c57_open(error: *mut *mut c_char) -> *mut Elan {
    match catch_unwind(Sensor::open_usb) {
        Ok(Ok(sensor)) => Box::into_raw(Box::new(Elan { sensor, matcher: Matcher::default() })),
        Ok(Err(e)) => {
            set_error(error, e);
            ptr::null_mut()
        }
        Err(_) => {
            set_error(error, "internal error while opening the reader");
            ptr::null_mut()
        }
    }
}

/// # Safety
/// `dev` must come from [`elan0c57_open`] and not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_close(dev: *mut Elan) {
    if !dev.is_null() {
        drop(unsafe { Box::from_raw(dev) });
    }
}

/// Waits for a finger (until `cancel` says stop) and captures one touch.
/// On [`ELAN0C57_OK`], `out` holds the touch in the [`print`] format.
///
/// # Safety
/// `dev` from [`elan0c57_open`]; `out`, `out_len` valid; `error` NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_capture(
    dev: *mut Elan,
    cancel: CancelFn,
    user_data: *mut c_void,
    out: *mut *mut u8,
    out_len: *mut usize,
    error: *mut *mut c_char,
) -> c_int {
    let dev = unsafe { &mut *dev };
    let is_cancelled = || cancel.is_some_and(|f| unsafe { f(user_data) });
    let result = catch_unwind(AssertUnwindSafe(|| dev.sensor.capture_cancellable(None, &is_cancelled)));
    match result {
        Ok(Ok(touch)) => {
            if Prepared::new(&touch.print).is_partial() {
                return ELAN0C57_PARTIAL;
            }
            give(print::serialize(&[touch.print]), out, out_len);
            ELAN0C57_OK
        }
        Ok(Err(Error::Cancelled)) => ELAN0C57_CANCELLED,
        Ok(Err(Error::FingerDuringCalibration | Error::Calibration)) => ELAN0C57_RETRY,
        Ok(Err(e)) => {
            set_error(error, e);
            ELAN0C57_ERROR
        }
        Err(_) => {
            set_error(error, "internal error during capture");
            ELAN0C57_ERROR
        }
    }
}

/// Waits (at most a few seconds) for the finger to leave the sensor.
///
/// # Safety
/// `dev` from [`elan0c57_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_wait_finger_off(dev: *mut Elan) {
    let dev = unsafe { &mut *dev };
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if dev.sensor.wait_finger_off(Duration::from_secs(5)).is_err() {
            std::thread::sleep(Duration::from_millis(500));
        }
        let _ = dev.sensor.stop();
    }));
}

/// Joins `n` captured touches into one enrolled print.
///
/// # Safety
/// `touches` and `lens` point to `n` buffers and their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_merge(
    touches: *const *const u8,
    lens: *const usize,
    n: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> c_int {
    let mut frames: Vec<Frame> = Vec::with_capacity(n);
    for i in 0..n {
        let data = unsafe { bytes(*touches.add(i), *lens.add(i)) };
        match print::deserialize(data) {
            Some(mut f) => frames.append(&mut f),
            None => return ELAN0C57_ERROR,
        }
    }
    give(print::serialize(&frames), out, out_len);
    ELAN0C57_OK
}

/// Similarity of a probe to an enrolled print, in [-1, 1]; < -1 if a print is damaged.
///
/// # Safety
/// `dev` from [`elan0c57_open`]; the buffers valid for their lengths.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_score(
    dev: *mut Elan,
    enrolled: *const u8,
    enrolled_len: usize,
    probe: *const u8,
    probe_len: usize,
) -> f64 {
    let dev = unsafe { &*dev };
    let (enrolled, probe) = unsafe { (bytes(enrolled, enrolled_len), bytes(probe, probe_len)) };
    let (Some(enrolled), Some(probe)) = (print::deserialize(enrolled), print::deserialize(probe)) else {
        return -2.0;
    };
    let Some(probe) = probe.first() else { return -2.0 };
    catch_unwind(AssertUnwindSafe(|| {
        let touches: Vec<Prepared> = enrolled.iter().map(Prepared::new).collect();
        dev.matcher.score(&dev.matcher.template(&touches), &Prepared::new(probe))
    }))
    .unwrap_or(-2.0)
}

/// Scores at or above this value are a match.
#[unsafe(no_mangle)]
pub extern "C" fn elan0c57_threshold() -> f64 {
    THRESHOLD
}

/// # Safety
/// `data`/`len` exactly as returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_free(data: *mut u8, len: usize) {
    if !data.is_null() {
        drop(unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(data, len)) });
    }
}

/// # Safety
/// `text` as returned by this library (error messages).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn elan0c57_free_string(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}
