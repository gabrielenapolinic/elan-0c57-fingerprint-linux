# ELAN 04f3:0c57 fingerprint reader on Linux

## Overview

Make the **ELAN `04f3:0c57`** fingerprint reader (`ELAN:Fingerprint`, found in
Lenovo IdeaPad laptops such as the IdeaPad 3 15IML05) work with `fprintd`: login, lock screen and `sudo`
with your fingerprint, LED included.

The reader is not supported by upstream libfprint
([libfprint#323](https://gitlab.freedesktop.org/libfprint/libfprint/-/issues/323)).
This project reuses ELAN's own Linux driver for the closely related
`04f3:0c4b` reader (shipped by Lenovo for the ThinkPad E14 Gen 4), which
contains ELAN's matching algorithm, and adapts it to the `0c57`.

> **Status:** works well on the tested machine, but it is not perfect: now and
> then a touch is rejected and has to be repeated (see [Results](#results)).
> This is an unofficial project, not affiliated with ELAN or Lenovo.

---

## Key Features

* **Fingerprint login, lock screen and `sudo`** through the standard `fprintd` stack.
* **ELAN's own matching algorithm:** reuses ELAN's Linux TOD driver for the related `0c4b` reader, patched for the `0c57`.
* **No proprietary code in the repository:** the ELAN driver is downloaded, checked against a known SHA-256 and patched on your machine.
* **OpenSSL 3 compatibility shim** replacing the OpenSSL 1.1 dependency of the driver.
* **Working LED:** the LED blinks once per touch.
* **Clean install and uninstall:** everything goes to `/usr/local`, distribution packages are not touched, previous files are backed up.
* **Optional GNOME keyring unlock** via the TPM, so the fingerprint works exactly like the password.

---

## Repository Structure

* `build.sh`: clones libfprint-TOD and the ELAN driver at pinned commits, patches and builds them into `build/stage/`.
* `install.sh` / `uninstall.sh`: install to / remove from `/usr/local` (as root), with backup and restore.
* `src/`: sources of `libelancrypto.so` (OpenSSL shim and LED hook).
* `patches/`: experimental patch for the open source `elan` driver.
* `docs/TECHNICAL.md`: protocol findings and experiments.
* `extras/keyring-tpm/`: optional tool to unlock the GNOME keyring after a fingerprint login.

---

## Tested on

| | |
|---|---|
| Laptop | Lenovo IdeaPad 3 15IML05 (machine type 81WB) |
| Reader | `04f3:0c57` ELAN:Fingerprint, firmware 0x0216, 80x80 press sensor |
| OS | Fedora 44, kernel 7.2, fprintd 1.94.5 |
| Arch | x86_64 only (the ELAN driver is an x86_64 binary) |

Check your reader with `lsusb | grep 04f3:0c57`.

---

## How it works

```
fprintd ──> libfprint-TOD ──loads──> libfprint-2-tod1-elan-0c57.so  (ELAN binary, patched)
                                              │
                                              └─needs─> libelancrypto.so  (this project)
                                                          ├─ OpenSSL 1.1 EVP shim → OpenSSL 3
                                                          └─ LED hook on USB transfers
```

1. **[libfprint-TOD](https://gitlab.freedesktop.org/3v1n0/libfprint/-/tree/tod)**
   is libfprint with support for binary "Touch OEM Drivers". It is built and
   installed in `/usr/local`, where it takes precedence over the distribution
   libfprint, without touching any package.
2. The **ELAN TOD driver** for `0c4b` is downloaded from its
   [original source](https://github.com/TonyHoyle/libfprint-2-tod1-elan)
   (extracted from Lenovo's Ubuntu driver package), checked against a known
   SHA-256 and patched in three places (each one checked before and after):
   - the USB id table entry `04f3:0c4b` becomes `04f3:0c57`;
   - its dependency on `libcrypto.so.1.1` (OpenSSL 1.1, no longer shipped by
     current distributions) is renamed to `libelancrypto.so`;
   - its import of `g_usb_device_bulk_transfer_async` is renamed to
     `e_usb_device_bulk_transfer_async`, so that USB transfers go through the
     LED hook.
3. **`libelancrypto.so`** (sources in [`src/`](src)) provides:
   - the 9 OpenSSL 1.1 EVP functions used by the driver (AES-256-CBC, SHA-256)
     forwarded to the system OpenSSL 3;
   - the LED handling: the ELAN driver never switches the LED on, so the hook
     sends "LED on" (`40 31`) before the driver waits for a finger (`40 3f`)
     and "LED off" (`00 0b`) once the image has been read, before the driver
     waits for the finger to be lifted (`40 3e`). The LED blinks once per
     touch, like with the open source `elan` driver.

The proprietary driver is **not** included in this repository: `build.sh`
downloads it and applies the changes on your machine.

---

## Install

### 1. Dependencies (Fedora)

```sh
sudo dnf install git gcc meson ninja-build pkgconf-pkg-config \
    glib2-devel libgusb-devel openssl-devel fprintd fprintd-pam
```

On other distributions install the equivalent packages (glib, gusb, OpenSSL 3
development files, meson, ninja). Only Fedora has been tested.

### 2. Build (as a normal user)

```sh
./build.sh
```

This clones libfprint-TOD and the ELAN driver at pinned commits into `build/`,
builds them and prepares the files to install in `build/stage/`. Nothing is
installed yet.

### 3. Install (as root)

```sh
sudo ./install.sh
```

The script:

- backs up any libfprint already present in `/usr/local/lib64` that was not
  installed by this project, and verifies the backup before overwriting
  anything;
- copies the files to `/usr/local/lib64`;
- adds `/usr/local/lib64` to the dynamic linker path if it is missing
  (`/etc/ld.so.conf.d/elan-0c57-fingerprint-linux.conf`);
- fixes SELinux labels (`restorecon`) and stops `fprintd`, which restarts on
  demand with the new library.

### 4. Enroll and test

```sh
fprintd-enroll -f right-index-finger   # 18 touches, move the finger a bit each time
fprintd-verify
```

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
fprintd-delete $USER        # prints made with the ELAN driver can't be reused
```

This removes the installed files and restores the backup made by
`install.sh`, if any; otherwise `fprintd` goes back to the distribution
libfprint.

---

## Results

Measured on the tested laptop (right index finger enrolled, 18 touches):

| Test | Right index accepted | Wrong finger (thumb) accepted |
|---|---|---|
| libfprint example `verify` | 5 / 7 | 0 / 2 |
| `fprintd-verify` | 5 / 5 | – |
| `fprintd-verify` with LED handling | 7 / 9 | – |

The sensor only sees about 4x4 mm of the finger, so a touch is sometimes
rejected and has to be repeated. No wrong finger was accepted in the tests,
but the number of impostor attempts is small: keep a password as fallback.

---

## Notes

- **Updates:** distribution updates don't touch `/usr/local`. If a future
  `fprintd` stops working with this libfprint-TOD version, run `./build.sh`
  and `sudo ./install.sh` again (possibly with a newer `TOD_COMMIT`).
- **Debugging:** `ELAN_LED_TRACE=1` makes `libelancrypto.so` print every USB
  transfer of the driver; `G_MESSAGES_DEBUG=all` enables libfprint debug logs.
- **Open source driver:** [`docs/TECHNICAL.md`](docs/TECHNICAL.md) describes
  the protocol findings and the experiments with the open source `elan` driver
  (it talks to the sensor fine, but its matchers are not reliable on such a
  small sensor). The experimental patch is in [`patches/`](patches).

---

## License

The code in this repository (scripts, `src/`, `patches/`, `extras/`) is licensed under the
**GNU Lesser General Public License v2.1 or later** (LGPL), like libfprint.

<img src="images/LGPLv3_Logo.svg.png" alt="LGPL License" width="200"/>

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU Lesser General Public License as published by the Free Software Foundation, either version 2.1
of the License, or (at your option) any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without
even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Lesser General Public License for more details.

The ELAN driver is proprietary software owned by its authors and is subject to their own
license; it is not distributed here.

---

## Authors & Credits

* **Gabriele Nicola Napoli**

Third-party work this project builds on:

- ELAN Microelectronics / Lenovo: the proprietary TOD driver.
- [TonyHoyle/libfprint-2-tod1-elan](https://github.com/TonyHoyle/libfprint-2-tod1-elan):
  the extracted driver.
- [Marco Trevisan (3v1n0) – libfprint-TOD](https://gitlab.freedesktop.org/3v1n0/libfprint/-/tree/tod).
- [libfprint](https://fprint.freedesktop.org/) and the `elan` driver authors.

---

## Contacts

For any request, comment, or suggestion, please open an issue or write to: gabrielenicolanapoli@gmail.com
