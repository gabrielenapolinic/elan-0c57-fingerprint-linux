# Technical notes: ELAN 04f3:0c57

Findings collected while porting the reader, useful for anyone who wants to
write a fully open source driver.

## The device

```
Bus 001 Device 002: ID 04f3:0c57 Elan Microelectronics Corp. ELAN:Fingerprint
bcdDevice 2.16, interface class 255 (vendor specific)
EP 0x81 IN  bulk 64     EP 0x01 OUT bulk 64
EP 0x82 IN  bulk 64     EP 0x03 OUT bulk 64
EP 0x83 IN  bulk 64
```

- It is **not** a match-on-chip sensor: `elanmoc` / `elanmoc2` use endpoint
  `0x84`, which this device does not have, and their protocol does not work.
- It speaks the protocol of the open source **`elan`** image driver
  (`libfprint/drivers/elan.c`): commands on `0x01`, replies on `0x83`, image on
  `0x82`. The neighbouring `0c58` and `0c4b` ids are already in that driver.
- Firmware `0x0216`, sensor dimensions **80x80**, 16 bit raw pixels
  (one image = 12800 bytes).
- It is a **press (touch) sensor**, not a swipe sensor.

## Command sequence of ELAN's proprietary driver

Traced with `ELAN_LED_TRACE=1` (one touch during enrollment):

| Command | Reply | Meaning |
|---|---|---|
| `40 19` | 2 bytes on 0x83 | firmware version |
| `40 21` (3 bytes) | 1 byte | sensor setup / status |
| `00 0c` | 4 bytes | sensor dimensions |
| `40 24` | 2 bytes | calibration mean |
| `40 3f` | 1 byte, blocks until a finger is present | wait for finger |
| `00 09` | 12800 bytes on 0x82 | read image |
| `40 3e` | 1 byte, blocks until the finger is lifted | wait for finger off |
| `00 09` | 12800 bytes on 0x82 | image without finger |

The ELAN driver never sends `40 31` (LED on), so the LED stays off unless the
hook in `src/elan-led.c` adds it. `00 0b` (stop) switches the LED off.

## Binary patches applied to the ELAN driver (earlier versions, no longer used)

Earlier versions of this project installed ELAN's proprietary driver,
patched as below. The open source driver (last section) replaced it.

Original file: `libfprint-2-tod1-elan.so` from
[TonyHoyle/libfprint-2-tod1-elan](https://github.com/TonyHoyle/libfprint-2-tod1-elan)
(commit `f2aabc25`), SHA-256
`34087826dee386a2919e8b0cd2663d1c249be1f8f1a3951bc1f2c88038d30605`.
The driver is not stripped and has debug info; its id table (`id_table`,
`.rodata`) contains `04f3:0c42` and `04f3:0c4b`, entries of 0x90 bytes.

| File offset | Original | Patched | Why |
|---|---|---|---|
| `0x35630` | `4b` | `57` | id table entry `0c4b` → `0c57` |
| `12997` (`.dynstr`) | `libcrypto.so.1.1` | `libelancrypto.so` | OpenSSL 1.1 is gone; the shim provides the 9 EVP symbols (version `OPENSSL_1_1_0`) |
| `14704` (`.dynstr`) | `g_usb_device_bulk_transfer_async` | `e_usb_device_bulk_transfer_async` | route USB transfers through the LED hook (version `LIBGUSB_0.1.0`, provided by the shim) |

Patched SHA-256:
`87ee037cc85fee844b54ab867fed07376a963a2bf2d990f5b8acd0b9015a8677`.

## Experiments with the open source `elan` driver

`patches/elan-0c57-press-mode.patch` (against
[Depau's libfprint fork](https://gitlab.freedesktop.org/Depau/libfprint),
branch `elanmoc2`, which contains the regular `elan` driver) adds the `0c57` to
the `elan` driver with:

- a **press mode** (`ELAN_PRESS`): a few frames are read once the finger is
  detected, the last one (80x80, no cropping to 50 rows) is upscaled 2x
  (pixman) instead of assembling a swipe;
- waiting for the finger to be lifted (or for a non-empty background) before
  recalibrating: without it, recalibration right after a capture fails with
  "Calibration failed!".

With it the sensor works (calibration, capture, clear ridge images) but the
matchers available in libfprint are **not reliable** on an 80x80 area:

| Matcher | Result |
|---|---|
| NBIS (bozorth3) | 1–3 minutiae per image, score always 0: no match |
| SIGFM ([MR !418](https://gitlab.freedesktop.org/libfprint/libfprint/-/merge_requests/418)) | genuine finger accepted 2–3 / 7; offline, some thumb images scored higher than genuine ones |
| SIFT + rigid RANSAC (offline test) | same problem: impostor scores overlapping genuine ones |

Normalising the raw frames with a 2nd–98th percentile linear stretch gave
visibly sharper ridges than the driver's "thirds" normalisation.

A fully open source solution needed a matcher designed for very small
sensors: see below.

## The open source driver (driver/)

Written in Rust; it replaced ELAN's proprietary driver in this project.

- **Protocol** as above, following the `elan` driver with the press-mode
  patch: background → calibration mean `40 24` → recalibration (`40 23`,
  status `0x01` then `0x03`) if they differ by more than 500, re-reading the
  background while the finger is still down → LED `40 31` → wait for finger
  `40 3f` → up to 3 images `00 09`, the last one kept → stop `00 0b`. The wait
  for a finger is read again every 500 ms, so it can be cancelled.
- **Image**: raw frame minus background; local contrast normalisation
  (Gaussian high-pass divided by the local standard deviation, sigma 4); the
  touched area is where the smoothed contrast exceeds 0.35 x its median,
  eroded by 2 pixels. A touch covering less than 0.85 of the sensor is
  partial (a full one covers 0.90) and must be repeated.
- **Matcher**: masked normalised cross-correlation (Padfield 2012) between
  the probe and each enrolled touch, for every shift leaving at least 35% of
  the image in common, rotations searched every 10° and then ±5° around the
  best one (±30° in all). Single precision 135x135 FFTs; about 40 ms per
  verification on 8 threads.
- **Results** on 75 touches of 3 fingers, against phase-only correlation:

| Matcher | Genuine accepted with no impostor accepted (threshold chosen on the data) |
|---|---|
| Phase-only correlation | 77% |
| Band-limited POC | 74% |
| Masked NCC | 81%, 85% without partial touches |

  With the fixed threshold 0.86: 79% of genuine touches accepted, 0 of 1360
  impostor attempts (highest impostor score 0.825).
