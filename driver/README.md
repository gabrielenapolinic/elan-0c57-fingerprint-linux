# elan-0c57-rs

Open source user-space driver, written in Rust, for the **ELAN `04f3:0c57`**
press fingerprint reader (Lenovo IdeaPad 81WB and others).

Today this reader works on Linux only with ELAN's proprietary driver
(see the top of this repository, which builds and installs it). It replaced
ELAN's proprietary driver:

1. **Sensor protocol** — done: calibration, finger detection, capture of one
   touch, LED (`src/protocol.rs`, `src/device.rs`).
2. **Dataset tool** — done: `elan-capture` saves the touches of each finger.
3. **Matcher** for an 80x80 sensor — first version done: masked normalised
   cross-correlation over rotations and shifts (`src/matcher.rs`), identical
   to the Python reference `research/eval.py`, about 36 ms per verification
   (see below). Partial touches (finger on less than 85% of the sensor) are
   rejected and must be repeated. Still to do: a fixed threshold tested on a
   second dataset.
4. **fprintd integration** — done: `tod/` is a small libfprint-TOD driver
   in C that calls this library (12 enrollment touches, verify, identify,
   cancellation, partial touches reported as "center your finger").

## Testing the driver without installing it

To install it for `fprintd`, use `build.sh` and `install.sh` at the top of
the repository. To try a change first, with libfprint's example programs:

```
cd tod
make TOD_SRC=../../build/libfprint-tod TOD_BUILD=../../build/libfprint-tod/_build
TOD_SRC=../../build/libfprint-tod TOD_BUILD=../../build/libfprint-tod/_build ./try.sh enroll
TOD_SRC=../../build/libfprint-tod TOD_BUILD=../../build/libfprint-tod/_build ./try.sh verify 5
```

(`TOD_SRC` defaults to a development checkout in `~/fprint-porting/libfprint-tod`.)

## Build

```
sudo dnf install rust cargo libusb1-devel
cargo build --release
cargo test
```

## Access to the reader

The reader is root-only by default. Give the logged-in user access:

```
sudo cp udev/71-elan-0c57-uaccess.rules /etc/udev/rules.d/
sudo udevadm control --reload
sudo udevadm trigger --attr-match=idVendor=04f3 --attr-match=idProduct=0c57
```

`fprintd` must not be using the reader at the same time (it only opens it
while enrolling or verifying).

## Usage

```
target/release/elan-capture info                       # firmware, size, calibration
target/release/elan-capture led                        # LED on for 2 s
target/release/elan-capture capture -f right-index -n 25
```

Each touch is saved in `dataset/<finger>/`:

| File | Content |
|---|---|
| `NNN-print.png` | fingerprint: raw frame minus background, 16 bit |
| `NNN-raw.png` | raw frame, 16 bit |
| `NNN-bg.png` | background (no finger), 16 bit |
| `NNN.png` | preview: 2–98 percentile stretch, enlarged 4x, 8 bit |

`dataset/index.csv` lists all touches. Running the command again adds new
touches after the existing ones. Touches that cover less than 85% of the
sensor are not saved and must be repeated (`--keep-partial` saves them).

## Matcher evaluation

```
cargo run --release --bin elan-match -- eval --splits 5   # error rates
cargo run --release --bin elan-match -- score             # scores of split 0, as CSV
python3 research/eval.py                                  # Python reference (POC, BLPOC, NCC)
```

First dataset (2026-10-27, one session, 25 touches each of right index,
left index and right thumb; 12 enrolled, the rest verified, 5 random splits).
5 touches are partial (4 of the left index, 1 of the thumb): a full touch
covers 0.90 of the sensor, these 0.69-0.83. `elan-match eval` leaves them
out, as the driver would ask to repeat them (`--keep-partial` keeps them):

| Matcher | Genuine accepted with no impostor accepted* | right index | left index | right thumb |
|---|---|---|---|---|
| Phase-only correlation (Python, split 0 only) | 77% | 100% | 77% | 54% |
| Masked NCC (Rust), all touches | 81% | 98% | 89% | 54% |
| Masked NCC (Rust), partial touches left out | **85%** | 98% | 96% | 63% |

\* threshold chosen on the same data, so the numbers are optimistic. For
reference, ELAN's proprietary driver accepts the right index about 17 times
out of 21 (live test).

### Speed

`elan-match bench`: one probe against a finger enrolled with 12 touches, on
an Intel i5-10210U.

| Version | One core | All cores (8 threads) |
|---|---|---|
| First port (f64, 160x160 FFT, 13 angles) | 469 ms | – |
| Current | 112 ms | **36 ms** |

What changed, with no change in the genuine scores:

- single precision FFTs;
- 135x135 FFTs instead of 160x160: only shifts that keep 35% of the image in
  common are possible (at most 52 pixels per axis), so smaller transforms do
  not wrap around;
- partial transforms: empty rows are skipped going forward, and only the rows
  of allowed shifts are computed going back;
- rotations searched coarse to fine (every 10°, then ±5° around the best angle
  of each enrolled touch) instead of every 5°;
- the rotations are spread over the CPU cores.

## Protocol

See [`../docs/TECHNICAL.md`](../docs/TECHNICAL.md).
The sequence follows libfprint's `elan` driver with the press-mode patch:
read background → calibration mean `40 24` → recalibrate with `40 23` if the
two differ by more than 500 → LED `40 31` → wait for finger `40 3f` → up to
3 images `00 09` (the last one is kept) → stop `00 0b`.
A reply other than `0x55` to `40 3f` (e.g. `0xca` at the first capture after
boot) is a spurious wake: stop, wait 200 ms and start over, up to 5 times.

## License

LGPL-2.1-or-later, like libfprint, from which the protocol logic is derived.
