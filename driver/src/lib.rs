// SPDX-License-Identifier: LGPL-2.1-or-later

//! Open source user-space driver for the ELAN `04f3:0c57` press fingerprint reader.
//!
//! - [`protocol`]: commands and replies, as used by libfprint's `elan` driver.
//! - [`device`]: the sensor state machine (calibration, capture) over any [`device::Transport`].
//! - [`image`]: raw frames, background subtraction, normalisation and PNG output.
//! - [`matcher`]: comparison of fingerprints (masked normalised cross-correlation).
//! - [`print`]: storage format of enrolled fingerprints.
//! - [`ffi`]: C interface for the libfprint-TOD driver (`tod/`).
//! - [`i18n`]: messages in the language of the system.

pub mod device;
pub mod ffi;
pub mod i18n;
pub mod image;
pub mod matcher;
pub mod print;
pub mod protocol;
