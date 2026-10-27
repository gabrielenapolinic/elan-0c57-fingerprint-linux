// SPDX-License-Identifier: LGPL-2.1-or-later

//! Storage format of a fingerprint: one or more touches (background removed),
//! kept by fprintd as the print's data.
//!
//! ```text
//! "E57P"  magic
//! u8      version (1)
//! u16     number of touches  (little-endian, like everything below)
//! u16     width
//! u16     height
//! u16[]   pixels, touch after touch, row by row
//! ```

use crate::image::Frame;

const MAGIC: &[u8; 4] = b"E57P";
const VERSION: u8 = 1;
const HEADER: usize = 4 + 1 + 2 * 3;

pub fn serialize(touches: &[Frame]) -> Vec<u8> {
    let (w, h) = touches.first().map_or((0, 0), |t| (t.width, t.height));
    let mut out = Vec::with_capacity(HEADER + touches.len() * w * h * 2);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    for v in [touches.len(), w, h] {
        out.extend_from_slice(&(v as u16).to_le_bytes());
    }
    for t in touches {
        assert_eq!((t.width, t.height), (w, h), "touches of different sizes");
        out.extend(t.pixels.iter().flat_map(|p| p.to_le_bytes()));
    }
    out
}

pub fn deserialize(data: &[u8]) -> Option<Vec<Frame>> {
    if data.len() < HEADER || &data[..4] != MAGIC || data[4] != VERSION {
        return None;
    }
    let field = |i: usize| usize::from(u16::from_le_bytes([data[5 + 2 * i], data[6 + 2 * i]]));
    let (count, width, height) = (field(0), field(1), field(2));
    let size = width * height * 2;
    if data.len() != HEADER + count * size {
        return None;
    }
    Some(
        data[HEADER..]
            .chunks_exact(size.max(1))
            .take(count)
            .map(|chunk| Frame {
                width,
                height,
                pixels: chunk.chunks_exact(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect(),
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let touches = vec![
            Frame { width: 2, height: 2, pixels: vec![1, 2, 3, 65535] },
            Frame { width: 2, height: 2, pixels: vec![5, 6, 7, 8] },
        ];
        let data = serialize(&touches);
        assert_eq!(data.len(), 11 + 16);
        assert_eq!(deserialize(&data).unwrap(), touches);
    }

    #[test]
    fn rejects_damaged_data() {
        let data = serialize(&[Frame { width: 2, height: 2, pixels: vec![1, 2, 3, 4] }]);
        assert!(deserialize(&data[..data.len() - 1]).is_none());
        assert!(deserialize(b"XXXX").is_none());
    }
}
