//! A PNG writer small enough to carry: 8-bit RGB, no compression.

use std::io::{self, Write};
use std::path::Path;

/// Write pixels as produced by [`Renderer::render`](crate::Renderer::render)
/// to a PNG file. The file is not compressed, so it is about three bytes a
/// pixel.
///
/// # Panics
/// If `pixels` is not `width * height` long.
pub fn write_png(
    path: impl AsRef<Path>,
    width: u32,
    height: u32,
    pixels: &[u32],
) -> io::Result<()> {
    assert_eq!(pixels.len(), width as usize * height as usize);

    // Each row is a filter byte (0: none) and then R, G, B per pixel.
    let mut raw = Vec::with_capacity((width as usize * 3 + 1) * height as usize);
    for row in pixels.chunks(width as usize) {
        raw.push(0);
        for p in row {
            raw.extend_from_slice(&p.to_le_bytes()[..3]);
        }
    }

    // A zlib stream of stored blocks.
    let mut data = vec![0x78, 0x01];
    let mut blocks = raw.chunks(0xFFFF).peekable();
    while let Some(block) = blocks.next() {
        let len = block.len() as u16;
        data.push(blocks.peek().is_none() as u8);
        data.extend_from_slice(&len.to_le_bytes());
        data.extend_from_slice(&(!len).to_le_bytes());
        data.extend_from_slice(block);
    }
    data.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut header = Vec::new();
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // 8 bits a channel, colour type 2 (RGB), default compression, filter
    // and interlace.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut out = io::BufWriter::new(std::fs::File::create(path)?);
    out.write_all(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A])?;
    chunk(&mut out, b"IHDR", &header)?;
    chunk(&mut out, b"IDAT", &data)?;
    chunk(&mut out, b"IEND", &[])?;
    out.flush()
}

fn chunk(out: &mut impl Write, kind: &[u8; 4], body: &[u8]) -> io::Result<()> {
    out.write_all(&(body.len() as u32).to_be_bytes())?;
    out.write_all(kind)?;
    out.write_all(body)?;
    let crc = crc32(crc32(0xFFFF_FFFF, kind), body) ^ 0xFFFF_FFFF;
    out.write_all(&crc.to_be_bytes())
}

fn crc32(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
        }
    }
    crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in bytes {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        // The standard check values for the ASCII digits 1 to 9.
        assert_eq!(crc32(0xFFFF_FFFF, b"123456789") ^ 0xFFFF_FFFF, 0xCBF4_3926);
        assert_eq!(adler32(b"123456789"), 0x091E_01DE);
    }
}
