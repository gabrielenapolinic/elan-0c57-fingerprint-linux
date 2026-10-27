// SPDX-License-Identifier: LGPL-2.1-or-later

//! Raw sensor frames and their conversion to 8 bit images.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use anyhow::{Context, Result};

use crate::protocol::Dimensions;

/// A frame of 16 bit pixels (14 bit ADC values), row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u16>,
}

impl Frame {
    /// Builds a frame from the bytes sent by the sensor, which are
    /// little-endian and transposed (column by column).
    pub fn from_raw(raw: &[u8], dims: Dimensions) -> Self {
        let (w, h) = (dims.width, dims.height);
        let column_major: Vec<u16> = raw
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let mut pixels = vec![0; w * h];
        for y in 0..h {
            for x in 0..w {
                pixels[x + y * w] = column_major[y + x * h];
            }
        }
        Frame { width: w, height: h, pixels }
    }

    pub fn mean(&self) -> u32 {
        if self.pixels.is_empty() {
            return 0;
        }
        (self.pixels.iter().map(|&p| u64::from(p)).sum::<u64>() / self.pixels.len() as u64) as u32
    }

    /// Subtracts the background (clamping at zero), as the libfprint driver does.
    /// Returns `None` when nothing is left, i.e. the frame is darker than the
    /// background: the finger was probably on the sensor during calibration.
    pub fn minus_background(&self, background: &Frame) -> Option<Frame> {
        let pixels: Vec<u16> = self
            .pixels
            .iter()
            .zip(&background.pixels)
            .map(|(&p, &b)| p.saturating_sub(b))
            .collect();
        pixels.iter().any(|&p| p > 0).then(|| Frame { pixels, ..*self })
    }

    /// Linear stretch between two percentiles (e.g. 2 and 98): sharper ridges
    /// than [`Frame::normalize_thirds`] on this sensor.
    pub fn normalize_percentile(&self, low: f64, high: f64) -> Gray {
        let mut sorted = self.pixels.clone();
        sorted.sort_unstable();
        let at = |pct: f64| {
            let i = ((sorted.len() - 1) as f64 * pct / 100.0).round() as usize;
            f64::from(sorted[i])
        };
        let (lo, hi) = (at(low), at(high));
        let span = (hi - lo).max(1.0);
        let pixels = self
            .pixels
            .iter()
            .map(|&p| ((f64::from(p) - lo) / span * 255.0).clamp(0.0, 255.0).round() as u8)
            .collect();
        Gray { width: self.width, height: self.height, pixels }
    }

    /// The 3 level non-linear mapping of the libfprint driver (30% / 35% / 35% of the pixels).
    pub fn normalize_thirds(&self) -> Gray {
        let mut sorted = self.pixels.clone();
        sorted.sort_unstable();
        let n = sorted.len();
        let lvl = [sorted[0], sorted[n * 3 / 10], sorted[n * 65 / 100], sorted[n - 1]].map(u32::from);
        let scale = |p: u32, a: u32, b: u32, out: u32| if b > a { (p - a) * out / (b - a) } else { 0 };
        let pixels = self
            .pixels
            .iter()
            .map(|&p| {
                let p = u32::from(p);
                let v = if p < lvl[1] {
                    scale(p, lvl[0], lvl[1], 99)
                } else if p < lvl[2] {
                    99 + scale(p, lvl[1], lvl[2], 56)
                } else {
                    155 + scale(p, lvl[2], lvl[3], 100)
                };
                v.min(255) as u8
            })
            .collect();
        Gray { width: self.width, height: self.height, pixels }
    }

    /// Saves the frame losslessly as a 16 bit grayscale PNG.
    pub fn save_png(&self, path: &Path) -> Result<()> {
        let data: Vec<u8> = self.pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        write_png(path, self.width, self.height, png::BitDepth::Sixteen, &data)
    }

    pub fn load_png(path: &Path) -> Result<Frame> {
        let decoder = png::Decoder::new(File::open(path).with_context(|| format!("opening {}", path.display()))?);
        let mut reader = decoder.read_info()?;
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf)?;
        anyhow::ensure!(
            info.bit_depth == png::BitDepth::Sixteen && info.color_type == png::ColorType::Grayscale,
            "{} is not a 16 bit grayscale PNG",
            path.display()
        );
        let pixels = buf[..info.buffer_size()]
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect();
        Ok(Frame { width: info.width as usize, height: info.height as usize, pixels })
    }
}

/// An 8 bit grayscale image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gray {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Gray {
    /// Nearest-neighbour enlargement, to look at the tiny 80x80 images.
    pub fn upscale(&self, factor: usize) -> Gray {
        let (w, h) = (self.width * factor, self.height * factor);
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                pixels.push(self.pixels[x / factor + y / factor * self.width]);
            }
        }
        Gray { width: w, height: h, pixels }
    }

    pub fn save_png(&self, path: &Path) -> Result<()> {
        write_png(path, self.width, self.height, png::BitDepth::Eight, &self.pixels)
    }
}

fn write_png(path: &Path, width: usize, height: usize, depth: png::BitDepth, data: &[u8]) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(depth);
    encoder.write_header()?.write_image_data(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: usize, height: usize, pixels: Vec<u16>) -> Frame {
        Frame { width, height, pixels }
    }

    #[test]
    fn raw_frames_are_transposed() {
        // 2 columns of 3 pixels, sent column by column.
        let raw: Vec<u8> = [1u16, 2, 3, 4, 5, 6].iter().flat_map(|p| p.to_le_bytes()).collect();
        let f = Frame::from_raw(&raw, Dimensions { width: 2, height: 3 });
        assert_eq!(f.pixels, vec![1, 4, 2, 5, 3, 6]);
    }

    #[test]
    fn background_is_subtracted_and_clamped() {
        let bg = frame(2, 1, vec![10, 10]);
        assert_eq!(frame(2, 1, vec![15, 5]).minus_background(&bg).unwrap().pixels, vec![5, 0]);
        assert!(frame(2, 1, vec![9, 10]).minus_background(&bg).is_none());
    }

    #[test]
    fn percentile_stretch_uses_the_full_range() {
        let f = frame(5, 1, vec![100, 200, 300, 400, 500]);
        assert_eq!(f.normalize_percentile(0.0, 100.0).pixels, vec![0, 64, 128, 191, 255]);
    }

    #[test]
    fn thirds_keeps_the_order() {
        let f = frame(10, 1, (0..10).map(|i| i * 100).collect());
        let g = f.normalize_thirds();
        assert!(g.pixels.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!((g.pixels[0], g.pixels[9]), (0, 255));
    }

    #[test]
    fn png_round_trip() {
        let dir = std::env::temp_dir().join(format!("elan-0c57-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("frame.png");
        let f = frame(3, 2, vec![0, 1, 16383, 300, 65535, 7]);
        f.save_png(&path).unwrap();
        assert_eq!(Frame::load_png(&path).unwrap(), f);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
