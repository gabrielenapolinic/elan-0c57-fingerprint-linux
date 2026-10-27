// SPDX-License-Identifier: LGPL-2.1-or-later

//! Fingerprint matcher for small press sensors: masked normalised cross-correlation.
//!
//! An 80x80 image holds too few minutiae for minutiae matchers, so the
//! ridge images are compared directly:
//!
//! 1. each image is normalised for local contrast, and a mask keeps the
//!    areas actually touched by the finger;
//! 2. the probe is rotated (±30°) and, for every shift, correlated with each
//!    enrolled touch over their common area (masked NCC, Padfield 2012),
//!    computed for all shifts at once with single precision FFTs; rotations
//!    are searched coarse to fine (10° steps, then ±5° for the best touches);
//! 3. the score is the best correlation over enrolled touches, rotations and
//!    shifts with at least [`MIN_OVERLAP`] of the image in common.
//!
//! Validated against `research/eval.py`, which is the reference implementation.

use std::sync::Arc;

use rustfft::num_complex::Complex32 as C32;
use rustfft::{Fft, FftPlanner};

use crate::image::Frame;

/// Minimum common area, as a fraction of the image.
pub const MIN_OVERLAP: f64 = 0.35;
/// Scores at or above this are a match (see the README for how it was chosen).
pub const THRESHOLD: f64 = 0.86;
/// Touches covering less of the sensor are partial and must be repeated.
/// A full touch covers 0.90 (the mask loses a 2 pixel border); the partial
/// touches of the first dataset covered 0.69-0.83.
pub const MIN_COVERAGE: f64 = 0.85;
/// Largest shift (pixels, per axis) that can still leave MIN_OVERLAP of an
/// 80x80 image in common: (80 - 52) * 80 >= 0.35 * 80 * 80.
const MAX_SHIFT: usize = 52;
/// FFT size: shifts up to MAX_SHIFT of an 80 pixel image do not wrap around
/// (80 + 52 <= 135, and 135 = 3^3 * 5 is a fast FFT size).
const PAD: usize = 135;
const SHIFTS: usize = 2 * MAX_SHIFT + 1;
const COARSE_ANGLES: [f64; 7] = [-30., -20., -10., 0., 10., 20., 30.];
const FINE_STEP: f64 = 5.0;
const MAX_ANGLE: f64 = 30.0;
const CONTRAST_SIGMA: f64 = 4.0;
const MASK_SIGMA: f64 = 6.0;
const MASK_LEVEL: f64 = 0.35;
const MASK_EROSION: usize = 2;

/// An image ready for matching: contrast-normalised ridges and the touched area.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub width: usize,
    pub height: usize,
    pub ridges: Vec<f64>,
    pub mask: Vec<f64>,
}

impl Prepared {
    pub fn new(frame: &Frame) -> Self {
        let (w, h) = (frame.width, frame.height);
        let img: Vec<f64> = frame.pixels.iter().map(|&p| f64::from(p)).collect();
        let low = gaussian(&img, w, h, CONTRAST_SIGMA);
        let hp: Vec<f64> = img.iter().zip(&low).map(|(a, b)| a - b).collect();
        let energy = gaussian(&hp.iter().map(|v| v * v).collect::<Vec<_>>(), w, h, CONTRAST_SIGMA);
        let std: Vec<f64> = energy.iter().map(|e| e.sqrt() + 1e-6).collect();
        let contrast = gaussian(&std, w, h, MASK_SIGMA);
        let level = MASK_LEVEL * median(&contrast);
        let touched: Vec<bool> = contrast.iter().map(|&c| c > level).collect();
        let mask: Vec<f64> = erode(&touched, w, h, MASK_EROSION).into_iter().map(f64::from).map(|m| m.min(1.0)).collect();
        let ridges = hp.iter().zip(&std).zip(&mask).map(|((v, s), m)| v / s * m).collect();
        Prepared { width: w, height: h, ridges, mask }
    }

    /// Fraction of the image touched by the finger.
    pub fn coverage(&self) -> f64 {
        self.mask.iter().sum::<f64>() / self.mask.len() as f64
    }

    /// The finger touched only part of the sensor: ask for another touch.
    pub fn is_partial(&self) -> bool {
        self.coverage() < MIN_COVERAGE
    }

    fn rotated(&self, degrees: f64) -> Prepared {
        if degrees == 0.0 {
            return self.clone();
        }
        let mask = rotate(&self.mask, self.width, self.height, degrees, false);
        let ridges = rotate(&self.ridges, self.width, self.height, degrees, true)
            .iter()
            .zip(&mask)
            .map(|(r, m)| r * m)
            .collect();
        Prepared { ridges, mask, ..*self }
    }
}

/// FFTs of one enrolled touch (transposed layout, see [`Matcher::fft`]).
struct Enrolled {
    img: Vec<C32>,
    mask: Vec<C32>,
    img2: Vec<C32>,
}

/// The enrolled touches of one finger.
pub struct Template {
    touches: Vec<Enrolled>,
}

pub struct Matcher {
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    /// Threads used by [`Matcher::score`].
    pub threads: usize,
}

impl Default for Matcher {
    /// A matcher using every CPU core.
    fn default() -> Self {
        Matcher::with_threads(std::thread::available_parallelism().map_or(1, |n| n.get()))
    }
}

/// FFTs of one rotation of the probe, conjugated.
struct Rotated {
    img: Vec<C32>,
    mask: Vec<C32>,
    img2: Vec<C32>,
}

impl Matcher {
    pub fn with_threads(threads: usize) -> Self {
        let mut planner = FftPlanner::new();
        Matcher { forward: planner.plan_fft_forward(PAD), inverse: planner.plan_fft_inverse(PAD), threads }
    }

    pub fn template(&self, touches: &[Prepared]) -> Template {
        Template {
            touches: touches
                .iter()
                .map(|t| Enrolled {
                    img: self.fft(&t.ridges, t.width, t.height),
                    mask: self.fft(&t.mask, t.width, t.height),
                    img2: self.fft(&squares(&t.ridges), t.width, t.height),
                })
                .collect(),
        }
    }

    /// Best masked NCC between the probe and the template, in [-1, 1].
    ///
    /// The rotations are searched coarse to fine: every enrolled touch is tried at
    /// [`COARSE_ANGLES`], then again at ±[`FINE_STEP`] around its best angle.
    pub fn score(&self, template: &Template, probe: &Prepared) -> f64 {
        let all: Vec<usize> = (0..template.touches.len()).collect();
        let jobs: Vec<(f64, Vec<usize>)> = COARSE_ANGLES.iter().map(|&a| (a, all.clone())).collect();
        let coarse = self.run(template, probe, &jobs);

        // Best coarse angle of each touch.
        let mut best: Vec<(f32, f64)> = vec![(-1.0, 0.0); all.len()];
        for ((angle, _), scores) in jobs.iter().zip(&coarse) {
            for (t, &s) in scores.iter().enumerate() {
                if s > best[t].0 {
                    best[t] = (s, *angle);
                }
            }
        }
        let mut fine: Vec<(f64, Vec<usize>)> = Vec::new();
        for t in all {
            for angle in [best[t].1 - FINE_STEP, best[t].1 + FINE_STEP] {
                if angle.abs() > MAX_ANGLE {
                    continue;
                }
                match fine.iter_mut().find(|(a, _)| *a == angle) {
                    Some((_, touches)) => touches.push(t),
                    None => fine.push((angle, vec![t])),
                }
            }
        }
        let refined = self.run(template, probe, &fine);
        coarse.iter().chain(&refined).flatten().fold(-1.0f32, |m, &s| m.max(s)).into()
    }

    /// Scores for each (angle, touches) job; jobs are spread over the threads.
    fn run(&self, template: &Template, probe: &Prepared, jobs: &[(f64, Vec<usize>)]) -> Vec<Vec<f32>> {
        let threads = self.threads.clamp(1, jobs.len().max(1));
        if threads == 1 {
            return jobs.iter().map(|(a, ts)| self.angle(template, probe, *a, ts)).collect();
        }
        let mut out = vec![Vec::new(); jobs.len()];
        std::thread::scope(|s| {
            let per = jobs.len().div_ceil(threads);
            for (chunk, results) in jobs.chunks(per).zip(out.chunks_mut(per)) {
                s.spawn(move || {
                    for ((a, ts), r) in chunk.iter().zip(results) {
                        *r = self.angle(template, probe, *a, ts);
                    }
                });
            }
        });
        out
    }

    /// Scores of the probe rotated by `angle` against some enrolled touches.
    fn angle(&self, template: &Template, probe: &Prepared, angle: f64, touches: &[usize]) -> Vec<f32> {
        let p = probe.rotated(angle);
        let r = Rotated {
            img: conj(self.fft(&p.ridges, p.width, p.height)),
            mask: conj(self.fft(&p.mask, p.width, p.height)),
            img2: conj(self.fft(&squares(&p.ridges), p.width, p.height)),
        };
        let min_n = (MIN_OVERLAP * (probe.width * probe.height) as f64) as f32;
        touches.iter().map(|&t| self.ncc(&template.touches[t], &r, min_n)).collect()
    }

    /// Masked NCC over every allowed shift (Padfield 2012).
    fn ncc(&self, t: &Enrolled, g: &Rotated, min_n: f32) -> f32 {
        // Real correlations, two per inverse FFT (real and imaginary parts).
        let (n, sf) = self.correlate_pair(&t.mask, &g.mask, &t.img, &g.mask);
        let (sg, sff) = self.correlate_pair(&t.mask, &g.img, &t.img2, &g.mask);
        let (sgg, sfg) = self.correlate_pair(&t.mask, &g.img2, &t.img, &g.img);
        let mut best = -1.0f32;
        for i in 0..n.len() {
            let n = n[i];
            if n <= min_n {
                continue;
            }
            let var_f = (sff[i] - sf[i] * sf[i] / n).max(1e-6);
            let var_g = (sgg[i] - sg[i] * sg[i] / n).max(1e-6);
            best = best.max((sfg[i] - sf[i] * sg[i] / n) / (var_f * var_g).sqrt());
        }
        best
    }

    /// 2D FFT of a w x h real image, zero-padded to PAD x PAD. Rows first
    /// (only the h non-empty ones), then columns; the result is left
    /// transposed, which is fine for element-wise products.
    fn fft(&self, img: &[f64], w: usize, h: usize) -> Vec<C32> {
        let mut data = vec![C32::new(0.0, 0.0); PAD * PAD];
        for y in 0..h {
            for x in 0..w {
                data[y * PAD + x].re = img[y * w + x] as f32;
            }
        }
        let mut scratch = vec![C32::new(0.0, 0.0); self.forward.get_inplace_scratch_len()];
        self.forward.process_with_scratch(&mut data[..h * PAD], &mut scratch);
        let mut data = transpose(&data);
        self.forward.process_with_scratch(&mut data, &mut scratch);
        data
    }

    /// Inverse FFTs of `a1 * b1` and `a2 * b2`, which are both real, in one
    /// transform, keeping only the allowed shifts (|dx|, |dy| <= MAX_SHIFT).
    fn correlate_pair(&self, a1: &[C32], b1: &[C32], a2: &[C32], b2: &[C32]) -> (Vec<f32>, Vec<f32>) {
        let mut data: Vec<C32> = (0..a1.len())
            .map(|k| {
                let (p, q) = (a1[k] * b1[k], a2[k] * b2[k]);
                C32::new(p.re - q.im, p.im + q.re) // p + i q
            })
            .collect();
        let mut scratch = vec![C32::new(0.0, 0.0); self.inverse.get_inplace_scratch_len()];
        self.inverse.process_with_scratch(&mut data, &mut scratch);
        let mut data = transpose(&data);
        // Only the rows of allowed vertical shifts: 0..=MAX_SHIFT and the negative ones at the end.
        self.inverse.process_with_scratch(&mut data[..(MAX_SHIFT + 1) * PAD], &mut scratch);
        self.inverse.process_with_scratch(&mut data[(PAD - MAX_SHIFT) * PAD..], &mut scratch);
        let scale = 1.0 / (PAD * PAD) as f32;
        let mut re = Vec::with_capacity(SHIFTS * SHIFTS);
        let mut im = Vec::with_capacity(SHIFTS * SHIFTS);
        for y in shift_indices() {
            for x in shift_indices() {
                let c = data[y * PAD + x];
                re.push(c.re * scale);
                im.push(c.im * scale);
            }
        }
        (re, im)
    }
}

/// Indices of the shifts -MAX_SHIFT..=MAX_SHIFT in a circular correlation.
fn shift_indices() -> impl Iterator<Item = usize> {
    (0..=MAX_SHIFT).chain(PAD - MAX_SHIFT..PAD)
}

fn transpose(data: &[C32]) -> Vec<C32> {
    let mut out = vec![C32::new(0.0, 0.0); PAD * PAD];
    for y in 0..PAD {
        for x in 0..PAD {
            out[x * PAD + y] = data[y * PAD + x];
        }
    }
    out
}

fn squares(v: &[f64]) -> Vec<f64> {
    v.iter().map(|x| x * x).collect()
}

fn conj(mut v: Vec<C32>) -> Vec<C32> {
    v.iter_mut().for_each(|c| *c = c.conj());
    v
}

/// Gaussian blur with mirrored borders, like `scipy.ndimage.gaussian_filter`
/// (mode "reflect", truncate 4).
fn gaussian(img: &[f64], w: usize, h: usize, sigma: f64) -> Vec<f64> {
    let radius = (4.0 * sigma + 0.5) as isize;
    let mut kernel: Vec<f64> = (-radius..=radius).map(|i| (-0.5 * (i as f64 / sigma).powi(2)).exp()).collect();
    let sum: f64 = kernel.iter().sum();
    kernel.iter_mut().for_each(|k| *k /= sum);
    let reflect = |i: isize, n: usize| -> usize {
        let n = n as isize;
        let period = 2 * n;
        let i = i.rem_euclid(period);
        (if i < n { i } else { period - 1 - i }) as usize
    };
    let mut tmp = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            tmp[y * w + x] = kernel
                .iter()
                .enumerate()
                .map(|(k, kv)| kv * img[y * w + reflect(x as isize + k as isize - radius, w)])
                .sum();
        }
    }
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            out[y * w + x] = kernel
                .iter()
                .enumerate()
                .map(|(k, kv)| kv * tmp[reflect(y as isize + k as isize - radius, h) * w + x])
                .sum();
        }
    }
    out
}

fn median(values: &[f64]) -> f64 {
    let mut v = values.to_vec();
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// Binary erosion with a 3x3 cross; outside the image counts as empty.
fn erode(mask: &[bool], w: usize, h: usize, iterations: usize) -> Vec<bool> {
    let mut m = mask.to_vec();
    for _ in 0..iterations {
        let at = |x: isize, y: isize, m: &[bool]| {
            x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && m[y as usize * w + x as usize]
        };
        m = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as isize, (i / w) as isize);
                at(x, y, &m) && at(x - 1, y, &m) && at(x + 1, y, &m) && at(x, y - 1, &m) && at(x, y + 1, &m)
            })
            .collect();
    }
    m
}

/// Rotation about the centre, keeping the size; outside the image is 0.
/// Bilinear (`linear`) or nearest neighbour, like `scipy.ndimage.rotate`
/// with order 1 / 0 and mode "constant".
fn rotate(img: &[f64], w: usize, h: usize, degrees: f64, linear: bool) -> Vec<f64> {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (cx, cy) = ((w as f64 - 1.0) / 2.0, (h as f64 - 1.0) / 2.0);
    let (maxx, maxy) = (w as f64 - 1.0, h as f64 - 1.0);
    let mut out = vec![0.0; w * h];
    for y in 0..h {
        for x in 0..w {
            let (dx, dy) = (x as f64 - cx, y as f64 - cy);
            let sx = cos * dx + sin * dy + cx;
            let sy = -sin * dx + cos * dy + cy;
            if !(-1e-9..=maxx + 1e-9).contains(&sx) || !(-1e-9..=maxy + 1e-9).contains(&sy) {
                continue;
            }
            out[y * w + x] = if linear {
                let (x0, y0) = (sx.floor().min(maxx - 1.0).max(0.0), sy.floor().min(maxy - 1.0).max(0.0));
                let (fx, fy) = (sx - x0, sy - y0);
                let (x0, y0) = (x0 as usize, y0 as usize);
                let p = |xx: usize, yy: usize| img[yy * w + xx];
                (1.0 - fy) * ((1.0 - fx) * p(x0, y0) + fx * p(x0 + 1, y0))
                    + fy * ((1.0 - fx) * p(x0, y0 + 1) + fx * p(x0 + 1, y0 + 1))
            } else {
                img[sy.round() as usize * w + sx.round() as usize]
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ridges(width: usize, height: usize, period: f64, angle: f64, shift: f64) -> Frame {
        let (s, c) = angle.to_radians().sin_cos();
        let pixels = (0..width * height)
            .map(|i| {
                let (x, y) = ((i % width) as f64, (i / width) as f64);
                let t = (x * c + y * s + shift) * std::f64::consts::TAU / period;
                (3000.0 + 1000.0 * t.sin()) as u16
            })
            .collect();
        Frame { width, height, pixels }
    }

    #[test]
    fn gaussian_keeps_constant_images() {
        let out = gaussian(&[5.0; 100], 10, 10, 4.0);
        assert!(out.iter().all(|v| (v - 5.0).abs() < 1e-9));
    }

    #[test]
    fn erosion_shrinks_from_the_border() {
        let m = erode(&[true; 25], 5, 5, 1);
        assert_eq!(m.iter().filter(|&&b| b).count(), 9);
    }

    #[test]
    fn rotation_by_zero_and_quarter_turn() {
        let img: Vec<f64> = (0..9).map(f64::from).collect();
        assert_eq!(rotate(&img, 3, 3, 0.0, true), img);
        let r = rotate(&img, 3, 3, 90.0, true);
        assert!((r[4] - 4.0).abs() < 1e-9);
        assert!((r[0] - img[2]).abs() < 1e-9 || (r[0] - img[6]).abs() < 1e-9);
    }

    #[test]
    fn half_touched_sensor_is_partial() {
        let full = ridges(80, 80, 7.0, 30.0, 0.0);
        assert!(!Prepared::new(&full).is_partial());
        // Only the left half touched: the right half is flat.
        let mut half = full.clone();
        for (i, p) in half.pixels.iter_mut().enumerate() {
            if i % 80 >= 40 {
                *p = 3000;
            }
        }
        assert!(Prepared::new(&half).is_partial());
    }

    #[test]
    fn shifted_copy_matches_better_than_other_ridges() {
        let m = Matcher::default();
        let template = m.template(&[Prepared::new(&ridges(80, 80, 7.0, 30.0, 0.0))]);
        let same = m.score(&template, &Prepared::new(&ridges(80, 80, 7.0, 30.0, 2.5)));
        let other = m.score(&template, &Prepared::new(&ridges(80, 80, 11.0, 100.0, 0.0)));
        assert!(same > 0.9, "same {same}");
        assert!(same > other + 0.2, "same {same} other {other}");
    }
}
