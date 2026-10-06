use crate::{invalid, slice, u16_at, u32_at, Result};
use alloc::vec::Vec;

#[derive(Clone)]
pub struct Pic {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub mask: Vec<bool>,
    pub palette: Vec<[u8; 3]>,
    offsets: Vec<u32>,
    source_len: usize,
    pub paintable: bool,
    pub glyphs: Vec<[usize; 3]>,
}
impl Pic {
    pub fn parse(data: &[u8]) -> Result<Self> {
        slice(data, 0, 64)?;
        let kind = u16_at(data, 0)?;
        let (width, height) = (u32_at(data, 2)?, u32_at(data, 6)?);
        if kind > 1
            || width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width * height > 4_194_304
        {
            return Err(invalid("unsupported menu PIC dimensions/kind"));
        }
        if u32_at(data, 10)? != 64 || data[50..64].iter().any(|b| *b != 0) {
            return Err(invalid("invalid PIC header"));
        }
        let source = slice(data, 64, u32_at(data, 14)?)?;
        let pal = slice(data, u32_at(data, 18)?, u32_at(data, 22)?)?;
        if pal.len() > 768 || pal.len() % 3 != 0 || pal.iter().any(|b| *b > 63) {
            return Err(invalid("invalid 6-bit PIC palette"));
        }
        let palette = pal
            .chunks_exact(3)
            .map(|rgb| core::array::from_fn(|i| ((rgb[i] as u16 * 255 + 31) / 63) as u8))
            .collect();
        let mut offsets = vec![u32::MAX; width * height];
        let mut used = vec![false; source.len()];
        let mut paintable = true;
        let mut pixels = vec![0; width * height];
        let mut mask = vec![kind == 0; width * height];
        if kind == 0 {
            if source.len() != pixels.len() {
                return Err(invalid("PIC raster length mismatch"));
            }
            pixels.copy_from_slice(source);
            for (i, off) in offsets.iter_mut().enumerate() {
                *off = (64 + i) as u32;
            }
            let row_offset = u32_at(data, 34)?;
            let row_size = u32_at(data, 38)?;
            if row_offset != 0 || row_size != 0 {
                if row_size != height * 4 {
                    return Err(invalid("invalid PIC row table"));
                }
                slice(data, row_offset, row_size)?;
                for row in 0..height {
                    if u32_at(data, row_offset + row * 4)? != 64 + row * width {
                        return Err(invalid("invalid PIC row offset"));
                    }
                }
            }
        } else {
            let spans = slice(data, u32_at(data, 26)?, u32_at(data, 30)?)?;
            let mut total = 0;
            let mut terminated = false;
            if spans.len() % 10 != 0 {
                return Err(invalid("invalid PIC span table size"));
            }
            for (index, span) in spans.chunks_exact(10).enumerate() {
                let (y, x, end) = (u16_at(span, 0)?, u16_at(span, 2)?, u16_at(span, 4)?);
                if y == 65535 {
                    if (index + 1) * 10 != spans.len() {
                        return Err(invalid("trailing PIC spans"));
                    }
                    terminated = true;
                    break;
                }
                if y >= height || end < x || end >= width {
                    return Err(invalid("PIC span outside image"));
                }
                let count = end - x + 1;
                let row = y * width + x;
                pixels[row..row + count].copy_from_slice(slice(source, u32_at(span, 6)?, count)?);
                mask[row..row + count].fill(true);
                let offset = u32_at(span, 6)?;
                for j in 0..count {
                    if offsets[row + j] != u32::MAX || used[offset + j] {
                        paintable = false;
                    }
                    offsets[row + j] = (64 + offset + j) as u32;
                    used[offset + j] = true;
                }
                total += count;
            }
            if !terminated || total != source.len() {
                return Err(invalid("incomplete PIC spans"));
            }
        }
        let mut glyphs = Vec::new();
        let glyph_offset = u32_at(data, 42)?;
        if glyph_offset != 0 {
            for glyph in slice(data, glyph_offset, 256 * 6)?.chunks_exact(6) {
                let values = [u16_at(glyph, 0)?, u16_at(glyph, 2)?, u16_at(glyph, 4)?];
                if values[0] + values[1] > width || values[2] > height {
                    return Err(invalid("glyph outside PIC strip"));
                }
                glyphs.push(values);
            }
        }
        // Pixel writes must never alias metadata sections.
        for (offset, size) in [(18, 22), (26, 30), (34, 38)] {
            let a = u32_at(data, offset)?;
            let n = u32_at(data, size)?;
            // Raw PICs retain an unused span capacity with a null span pointer.
            if offset == 26 && kind == 0 && a == 0 {
                continue;
            }
            if n > 0 {
                slice(data, a, n)?;
                if a < 64 + source.len() && a + n > 64 {
                    paintable = false;
                }
            }
        }
        let glyph = u32_at(data, 42)?;
        if glyph != 0 && glyph < 64 + source.len() && glyph + 1536 > 64 {
            paintable = false;
        }
        Ok(Self {
            offsets,
            source_len: data.len(),
            paintable,
            width,
            height,
            pixels,
            mask,
            palette,
            glyphs,
        })
    }
    pub fn rgba(&self, base: &[[u8; 3]; 256]) -> Vec<u8> {
        let mut palette = *base;
        palette[..self.palette.len()].copy_from_slice(&self.palette);
        self.pixels
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                let rgb = palette[*p as usize];
                [
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    if self.mask[i] && (self.glyphs.is_empty() || *p != 255) {
                        255
                    } else {
                        0
                    },
                ]
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn span_mask_preserves_opaque_zero_and_rejects_out_of_bounds() {
        let mut data = vec![0; 85];
        data[0] = 1;
        for (at, value) in [(2, 2u32), (6, 1), (10, 64), (14, 1), (26, 65), (30, 20)] {
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        // One opaque index-zero pixel at x=1, followed by the span terminator.
        data[67] = 1;
        data[69] = 1;
        data[75..77].copy_from_slice(&65535u16.to_le_bytes());
        let p = Pic::parse(&data).unwrap();
        assert_eq!(p.mask, [false, true]);
        let rgba = p.rgba(&[[12, 34, 56]; 256]);
        assert_eq!(rgba[3], 0);
        assert_eq!(&rgba[4..], &[12, 34, 56, 255]);
        data[69] = 2;
        assert!(Pic::parse(&data).is_err());
    }
    #[test]
    fn bounds_and_palette_expansion() {
        let mut data = vec![0; 71];
        for (at, value) in [(2, 2u32), (6, 2), (10, 64), (14, 4), (18, 68), (22, 3)] {
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[68..].copy_from_slice(&[63, 0, 31]);
        let pic = Pic::parse(&data).unwrap();
        assert_eq!(&pic.rgba(&[[0; 3]; 256])[..4], &[255, 0, 125, 255]);
        for end in 0..data.len() {
            assert!(Pic::parse(&data[..end]).is_err());
        }
        data[68] = 64;
        assert!(Pic::parse(&data).is_err());
    }
}

impl Pic {
    /// Patch only existing opaque raster bytes. Span structure and transparency stay intact.
    pub fn paint(
        &mut self,
        source: &mut [u8],
        x: usize,
        y: usize,
        radius: usize,
        color: u8,
    ) -> Result<usize> {
        if !self.paintable || source.len() != self.source_len {
            return Err(invalid(
                "PIC storage aliases metadata or samples; painting disabled",
            ));
        }
        if x >= self.width || y >= self.height || radius > 32 {
            return Err(invalid("Brush outside image/bounds"));
        }
        let mut count = 0;
        for yy in y.saturating_sub(radius)..=(y + radius).min(self.height - 1) {
            for xx in x.saturating_sub(radius)..=(x + radius).min(self.width - 1) {
                if (xx as i64 - x as i64).pow(2) + (yy as i64 - y as i64).pow(2)
                    > (radius * radius) as i64
                {
                    continue;
                }
                let i = yy * self.width + xx;
                let off = self.offsets[i];
                if off != u32::MAX && self.pixels[i] != color {
                    source[off as usize] = color;
                    self.pixels[i] = color;
                    count += 1;
                }
            }
        }
        Ok(count)
    }
    pub fn colors(&self, base: &[[u8; 3]; 256]) -> [[u8; 3]; 256] {
        let mut p = *base;
        p[..self.palette.len()].copy_from_slice(&self.palette);
        p
    }
    /// Portable PNG with an uncompressed zlib stream; preserves alpha for span holes.
    pub fn png(&self, base: &[[u8; 3]; 256]) -> Vec<u8> {
        fn crc(b: &[u8]) -> u32 {
            let mut c = !0u32;
            for x in b {
                c ^= *x as u32;
                for _ in 0..8 {
                    c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
                }
            }
            !c
        }
        fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], bytes: &[u8]) {
            out.extend((bytes.len() as u32).to_be_bytes());
            let at = out.len();
            out.extend(tag);
            out.extend(bytes);
            out.extend(crc(&out[at..]).to_be_bytes());
        }
        let rgba = self.rgba(base);
        let mut raw = Vec::with_capacity(rgba.len() + self.height);
        for row in rgba.chunks_exact(self.width * 4) {
            raw.push(0);
            raw.extend(row);
        }
        let mut z = vec![0x78, 0x01];
        let mut a = 1u32;
        let mut b = 0u32;
        for (i, block) in raw.chunks(65535).enumerate() {
            z.push(u8::from((i + 1) * 65535 >= raw.len()));
            let len = block.len() as u16;
            z.extend(len.to_le_bytes());
            z.extend((!len).to_le_bytes());
            z.extend(block);
            for x in block {
                a = (a + *x as u32) % 65521;
                b = (b + a) % 65521;
            }
        }
        z.extend(((b << 16) | a).to_be_bytes());
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut h = Vec::new();
        h.extend((self.width as u32).to_be_bytes());
        h.extend((self.height as u32).to_be_bytes());
        h.extend([8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &h);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}
pub fn palette(bytes: &[u8]) -> Result<[[u8; 3]; 256]> {
    if bytes.len() != 768 || bytes.iter().any(|b| *b > 63) {
        return Err(invalid("Expected 768-byte 6-bit RGB palette"));
    }
    Ok(core::array::from_fn(|i| {
        core::array::from_fn(|j| ((bytes[i * 3 + j] as u16 * 255 + 31) / 63) as u8)
    }))
}
pub fn demo() -> Vec<u8> {
    let mut b = vec![0; 64 + 32 * 32 + 768];
    for (at, n) in [
        (2, 32u32),
        (6, 32),
        (10, 64),
        (14, 1024),
        (18, 1088),
        (22, 768),
    ] {
        b[at..at + 4].copy_from_slice(&n.to_le_bytes());
    }
    for y in 0..32 {
        for x in 0..32 {
            b[64 + y * 32 + x] = ((x / 4 + y / 4) % 16 * 16) as u8;
        }
    }
    for i in 0..256 {
        b[1088 + i * 3] = (i % 8 * 9) as u8;
        b[1088 + i * 3 + 1] = (i / 8 % 8 * 9) as u8;
        b[1088 + i * 3 + 2] = (i / 64 * 21) as u8;
    }
    b
}
#[cfg(test)]
mod edit_tests {
    use super::*;
    #[test]
    fn paint_preserves_header_palette_and_untouched_pixels() {
        let mut bytes = demo();
        bytes[30..34].copy_from_slice(&330u32.to_le_bytes());
        let old = bytes.clone();
        let mut p = Pic::parse(&bytes).unwrap();
        assert_eq!(p.paint(&mut bytes, 3, 4, 0, 201).unwrap(), 1);
        let changed: Vec<_> = old
            .iter()
            .zip(&bytes)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(changed, vec![64 + 4 * 32 + 3]);
        assert_eq!(Pic::parse(&bytes).unwrap().pixels[4 * 32 + 3], 201);
        assert_eq!(&p.png(&[[0; 3]; 256])[..8], b"\x89PNG\r\n\x1a\n");
    }
    #[test]
    fn span_paint_leaves_holes_and_tables() {
        let mut b = vec![0; 85];
        b[0] = 1;
        for (at, n) in [(2, 2u32), (6, 1), (10, 64), (14, 1), (26, 65), (30, 20)] {
            b[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        b[67] = 1;
        b[69] = 1;
        b[75..77].copy_from_slice(&65535u16.to_le_bytes());
        let mut p = Pic::parse(&b).unwrap();
        let old = b.clone();
        assert_eq!(p.paint(&mut b, 0, 0, 0, 9).unwrap(), 0);
        assert_eq!(p.paint(&mut b, 1, 0, 0, 9).unwrap(), 1);
        assert_eq!(&b[..64], &old[..64]);
        assert_eq!(&b[65..], &old[65..]);
        assert!(!Pic::parse(&b).unwrap().mask[0]);
    }
}
