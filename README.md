# ELAN 04f3:0c57 fingerprint reader on Linux

[![license: LGPL v2.1 or later](images/license-lgpl.svg)](LICENSE)
![driver: 100% open source](images/driver.svg)
![Rust: 1.85+](images/rust.svg)
![platform: Fedora](images/platform.svg)
![languages: EN · IT · FR · DE · ES · PT](images/languages.svg)

## Overview

Make the **ELAN `04f3:0c57`** fingerprint reader (`ELAN:Fingerprint`, found in
Lenovo IdeaPad laptops such as the IdeaPad 3 15IML05) work with `fprintd`: login, lock screen and `sudo`
with your fingerprint, LED included.

The reader is not supported by upstream libfprint
([libfprint#323](https://gitlab.freedesktop.org/libfprint/libfprint/-/issues/323)).
This project provides a **fully open source driver written in Rust**: sensor
protocol, image processing and a matcher designed for the tiny 80x80 sensor,
plugged into `fprintd` through libfprint-TOD.

> **Status:** works well on the tested machine. The sensor only sees about
> 4x4 mm of the finger, so now and then a touch has to be repeated (see
> [Results](#results)). This is an unofficial project, not affiliated with ELAN or Lenovo.

---

## Key Features

* **Fingerprint login, lock screen and `sudo`** through the standard `fprintd` stack.
* **100% open source:** no proprietary ELAN code, no binary patches, no OpenSSL 1.1 shim.
* **Written in Rust:** USB protocol, calibration, capture and matching, with a small C adapter for libfprint-TOD.
* **Matcher for small sensors:** masked normalised cross-correlation over rotations and shifts, about 40 ms per verification.
* **Partial touches detected:** if the finger covers too little of the sensor, you are asked to touch again.
* **Working LED** while the reader waits for a finger.
* **Automatic language:** the scripts and the `elan-capture` / `elan-match` tools speak English, Italian, French, German, Spanish or Portuguese, following the system.
* **Clean install and uninstall:** everything goes to `/usr/local`, distribution packages are not touched, previous files are backed up.
* **Optional GNOME keyring unlock** via the TPM, so the fingerprint works exactly like the password.

---

## Repository Structure

* `build.sh`: clones libfprint-TOD at a pinned commit and builds it together with the driver into `build/stage/`.
* `install.sh` / `uninstall.sh`: install to / remove from `/usr/local` (as root), with backup and restore.
* `lib/i18n.sh`: translations of the scripts' messages (`driver/src/i18n.rs` for the tools).
* `driver/`: the open source driver ([details](driver/README.md)):
  * `src/`: Rust library (`protocol`, `device`, `image`, `matcher`, `print`, `ffi`);
  * `src/bin/`: `elan-capture` (talks to the reader, collects datasets) and `elan-match` (evaluates the matcher);
  * `tod/`: the C adapter for libfprint-TOD, and `try.sh` to test it without installing;
  * `research/`: Python reference of the matcher, used to validate the Rust port;
  * `udev/`: optional rule to use `elan-capture` without root.
* `docs/TECHNICAL.md`: protocol findings and experiments.
* `images/`: README badges.
* `patches/`: earlier experiment with libfprint's own `elan` driver.
* `extras/keyring-tpm/`: optional tool to unlock the GNOME keyring after a fingerprint login.

---

## Tested on

| | |
|---|---|
| Laptop | Lenovo IdeaPad 3 15IML05 (machine type 81WB) |
| Reader | `04f3:0c57` ELAN:Fingerprint, firmware 0x0216, 80x80 press sensor |
| OS | Fedora 44, kernel 7.2, fprintd 1.94.5 |

Check your reader with `lsusb | grep 04f3:0c57`.

---

## How it works

```
fprintd ──> libfprint-TOD ──loads──> libfprint-2-tod1-elan0c57-rs.so
                                        ├─ C adapter (driver/tod): enroll, verify, identify, cancel
                                        └─ Rust library (driver/src)
                                             ├─ USB protocol: calibration, finger detection, capture
                                             ├─ image: background removal, contrast normalisation
                                             └─ matcher: masked NCC over rotations and shifts
```

1. **[libfprint-TOD](https://gitlab.freedesktop.org/3v1n0/libfprint/-/tree/tod)**
   is libfprint able to load drivers built outside of it. It is built and
   installed in `/usr/local`, where it takes precedence over the distribution
   libfprint, without touching any package.
2. The **driver** talks to the sensor with the protocol of libfprint's `elan`
   driver (commands on endpoint `0x01`, replies on `0x83`, 80x80 16 bit images
   on `0x82`), recalibrating before each touch.
3. **Enrolling** takes 12 touches, stored by `fprintd` as the print. To
   **verify**, a new touch is compared with each of them, rotated up to ±30°
   and shifted over every position that leaves at least 35% of the image in
   common; the best normalised correlation over the touched area is the score.
   It is a match from **0.86** up.

---

## Install

### 1. Dependencies (Fedora)

```sh
sudo dnf install git gcc make meson ninja-build pkgconf-pkg-config \
    glib2-devel libgusb-devel libusb1-devel rust cargo fprintd fprintd-pam
```

On other distributions install the equivalent packages (glib, gusb, libusb,
Rust, meson, ninja). Only Fedora has been tested.

### 2. Build (as a normal user)

```sh
./build.sh
```

This clones libfprint-TOD at a pinned commit into `build/`, builds it and the
driver, and prepares the files to install in `build/stage/`. Nothing is
installed yet.

### 3. Install (as root)

```sh
sudo ./install.sh
```

The script:

- backs up any libfprint already present in `/usr/local/lib64` that was not
  installed by this project, and verifies the backup before overwriting
  anything;
- removes ELAN's proprietary driver if an earlier version of this project
  installed it;
- copies the files to `/usr/local/lib64`;
- adds `/usr/local/lib64` to the dynamic linker path if it is missing
  (`/etc/ld.so.conf.d/elan-0c57-fingerprint-linux.conf`);
- fixes SELinux labels (`restorecon`) and stops `fprintd`, which restarts on
  demand with the new library.

### 4. Enroll and test

```sh
fprintd-enroll -f right-index-finger   # 12 touches, move the finger a bit each time
fprintd-verify
```

Prints enrolled with ELAN's proprietary driver can't be used by this one:
enroll them again.

To use the fingerprint for login and `sudo` on Fedora:

```sh
sudo authselect enable-feature with-fingerprint
```

### 5. Optional: unlock the GNOME keyring

A fingerprint login provides no password, so the GNOME keyring stays locked.
[`extras/keyring-tpm`](extras/keyring-tpm) seals the login password in the
TPM and unlocks the keyring at the start of the session, so the fingerprint
works exactly like the password.

---

## Uninstall

```sh
sudo ./uninstall.sh
fprintd-delete $USER        # prints made with this driver can't be reused by others
```

This removes the installed files and restores the backup made by
`install.sh`, if any; otherwise `fprintd` goes back to the distribution
libfprint.

---

## Results

Offline, on a dataset of 75 touches (right index, left index, right thumb;
12 touches enrolled per finger, the others verified, 20 random splits, partial
touches left out), with the fixed threshold 0.86:

| | |
|---|---|
| Right finger accepted at the first touch | 79% |
| Wrong finger accepted | 0 of 1360 attempts (highest score 0.825) |

Live, through `fprintd` and libfprint's example programs:

| Test | Result |
|---|---|
| Right index, enrolled | 9 accepted out of 9 |
| Other fingers | 3 rejected out of 3 (1 partial touch, asked to repeat) |

For comparison, ELAN's proprietary driver accepted the right index about 17
times out of 21. The dataset was collected in one session: keep a password
as fallback, as for any fingerprint reader.

---

## Notes

- **Updates:** distribution updates don't touch `/usr/local`. If a future
  `fprintd` stops working with this libfprint-TOD version, run `./build.sh`
  and `sudo ./install.sh` again (possibly with a newer `TOD_COMMIT`).
- **Debugging:** `G_MESSAGES_DEBUG=libfprint-elan0c57rs` prints the score of
  every verification; `G_MESSAGES_DEBUG=all` enables all libfprint logs.
- **Developing:** see [`driver/README.md`](driver/README.md) to capture
  datasets, evaluate the matcher and test the driver without installing it.

---

## License

The code in this repository is licensed under the
**GNU Lesser General Public License v2.1 or later** (LGPL), like libfprint.

[![license: LGPL v2.1 or later](images/license-lgpl.svg)](LICENSE)

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU Lesser General Public License as published by the Free Software Foundation, either version 2.1
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Lesser General Public License for more details.

---

## Authors & Credits

* **Gabriele Nicola Napoli**

Third-party work this project builds on:

- [Marco Trevisan (3v1n0) – libfprint-TOD](https://gitlab.freedesktop.org/3v1n0/libfprint/-/tree/tod).
- [libfprint](https://fprint.freedesktop.org/) and the authors of its `elan` driver, whose protocol
  the Rust driver follows.
- Padfield, *Masked object registration in the Fourier domain* (2012), for the masked NCC.
- The Rust crates in [`driver/Cargo.toml`](driver/Cargo.toml) (`rusb`, `rustfft`, `png`, `clap`,
  `anyhow` and their dependencies), all under permissive licenses (MIT, Apache-2.0, Zlib).
  They are downloaded by `cargo` at build time and are not part of this repository.

---

## Contacts

For any request, comment, or suggestion, please open an issue or write to: gabrielenicolanapoli@gmail.com
