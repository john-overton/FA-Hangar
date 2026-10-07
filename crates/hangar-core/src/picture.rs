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
        self.brush(source, x, y, radius, |_, _| color)
    }
    /// Same raster geometry and byte map, so pixels can be copied back one by one.
    pub fn same_layout(&self, other: &Pic) -> bool {
        self.width == other.width && self.height == other.height && self.offsets == other.offsets
    }
    /// Eraser: the same circle as `paint`, writing each pixel's index from `original`.
    /// Only raster bytes change; header, palette and span tables stay untouched.
    pub fn paint_from(
        &mut self,
        source: &mut [u8],
        x: usize,
        y: usize,
        radius: usize,
        original: &Pic,
    ) -> Result<usize> {
        if !self.same_layout(original) {
            return Err(invalid(
                "Stored original has a different raster layout; use Restore texture",
            ));
        }
        self.brush(source, x, y, radius, |i, _| original.pixels[i])
    }
    fn brush(
        &mut self,
        source: &mut [u8],
        x: usize,
        y: usize,
        radius: usize,
        color: impl Fn(usize, u8) -> u8,
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
                let color = color(i, self.pixels[i]);
                if off != u32::MAX && self.pixels[i] != color {
                    source[off as usize] = color;
                    self.pixels[i] = color;
                    count += 1;
                }
            }
        }
        Ok(count)
    }
    pub fn patch_indices(&mut self, source: &mut [u8], pixels: &[u8]) -> Result<usize> {
        if !self.paintable || source.len() != self.source_len || pixels.len() != self.pixels.len() {
            return Err(invalid("PIC raster cannot be patched safely"));
        }
        let mut count = 0;
        for (i, color) in pixels.iter().enumerate() {
            let at = self.offsets[i];
            if at != u32::MAX && self.pixels[i] != *color {
                source[at as usize] = *color;
                self.pixels[i] = *color;
                count += 1;
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
    fn eraser_restores_original_raster_bytes_in_the_brush_circle() {
        let original = demo();
        let source = Pic::parse(&original).unwrap();
        let mut bytes = original.clone();
        let mut p = source.clone();
        assert!(p.paint(&mut bytes, 10, 10, 3, 7).unwrap() > 20);
        assert_eq!(p.paint(&mut bytes, 20, 20, 0, 7).unwrap(), 1);
        // Erasing one dab restores only that circle.
        p.paint_from(&mut bytes, 10, 10, 3, &source).unwrap();
        assert_eq!(bytes[64 + 20 * 32 + 20], 7);
        assert_eq!(&bytes[..64 + 20 * 32 + 20], &original[..64 + 20 * 32 + 20]);
        assert_eq!(p.paint_from(&mut bytes, 20, 20, 0, &source).unwrap(), 1);
        assert_eq!(bytes, original);
        assert_eq!(p.paint_from(&mut bytes, 20, 20, 3, &source).unwrap(), 0);
        let mut other = vec![0; 64 + 16 * 16 + 768];
        for (at, n) in [
            (2, 16u32),
            (6, 16),
            (10, 64),
            (14, 256),
            (18, 320),
            (22, 768),
        ] {
            other[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        let other = Pic::parse(&other).unwrap();
        assert!(!p.same_layout(&other));
        assert!(p.same_layout(&source));
        assert!(p.paint_from(&mut bytes, 1, 1, 0, &other).is_err());
        assert_eq!(bytes, original);
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

/// Palette indices a color replacement acts on, by index.
pub type IndexSet = [bool; 256];
/// Largest replacement tolerance, in 6-bit palette steps.
pub const MAX_TOLERANCE: u8 = 64;
/// The indices that match `from` within `tolerance`: the Euclidean distance
/// between their RGB and `from`'s, in 6-bit palette steps (the units a PAL
/// stores), is at most `tolerance`. Tolerance 0 is `from` alone, even when
/// another index holds the same color.
pub fn matching(colors: &[[u8; 3]; 256], from: u8, tolerance: u8) -> IndexSet {
    let mut set = [false; 256];
    set[from as usize] = true;
    if tolerance == 0 {
        return set;
    }
    // Exact inverse of the 6-bit expansion in `palette` and `Pic::parse`.
    let six = |c: [u8; 3]| c.map(|v| (v as i32 * 63 + 127) / 255);
    let a = six(colors[from as usize]);
    let t = tolerance.min(MAX_TOLERANCE) as i32;
    for (i, c) in colors.iter().enumerate() {
        let b = six(*c);
        if (0..3).map(|k| (a[k] - b[k]).pow(2)).sum::<i32>() <= t * t {
            set[i] = true;
        }
    }
    set
}
/// Pixels covered by face UV polygons (stored SH UVs, V counted up from the
/// bottom row as the renderer maps them). A pixel is covered when its centre
/// lies inside or on a polygon whose corners are pixel centres, so thin and
/// degenerate faces still cover the pixels on their edges. Bounded: at most
/// 4,096 polygons of 3 to 64 corners within ±65,536.
pub fn footprint(width: usize, height: usize, polygons: &[&[[i32; 2]]]) -> Result<Vec<bool>> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 || width * height > 4_194_304 {
        return Err(invalid("Footprint outside PIC limits"));
    }
    if polygons.len() > 4096 {
        return Err(invalid("Too many faces for one footprint"));
    }
    let mut mask = vec![false; width * height];
    for uv in polygons {
        if uv.len() > 64 || uv.iter().flatten().any(|v| !(-65536..=65536).contains(v)) {
            return Err(invalid("Face UVs outside footprint limits"));
        }
        if uv.is_empty() {
            continue;
        }
        let p: Vec<[i64; 2]> = uv
            .iter()
            .map(|q| [q[0] as i64, height as i64 - 1 - q[1] as i64])
            .collect();
        let lo = |k: usize| p.iter().map(|q| q[k]).min().unwrap();
        let hi = |k: usize| p.iter().map(|q| q[k]).max().unwrap();
        let (x0, x1) = (lo(0).max(0), hi(0).min(width as i64 - 1));
        let (y0, y1) = (lo(1).max(0), hi(1).min(height as i64 - 1));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let mut inside = false;
                let mut edge = false;
                for j in 0..p.len() {
                    let (a, b) = (p[j], p[(j + 1) % p.len()]);
                    let cross = (b[0] - a[0]) * (y - a[1]) - (x - a[0]) * (b[1] - a[1]);
                    if cross == 0
                        && x >= a[0].min(b[0])
                        && x <= a[0].max(b[0])
                        && y >= a[1].min(b[1])
                        && y <= a[1].max(b[1])
                    {
                        edge = true;
                        break;
                    }
                    if (a[1] > y) != (b[1] > y) && (cross > 0) == (b[1] > a[1]) {
                        inside = !inside;
                    }
                }
                if edge || inside {
                    mask[y as usize * width + x as usize] = true;
                }
            }
        }
    }
    Ok(mask)
}
impl Pic {
    /// Whether pixel `i` may take part in a replacement: opaque, inside
    /// `region`, and not the transparent index 255 of a glyph strip.
    fn replaceable(&self, i: usize, region: Option<&[bool]>) -> bool {
        self.offsets[i] != u32::MAX
            && self.mask[i]
            && region.is_none_or(|r| r[i])
            && (self.glyphs.is_empty() || self.pixels[i] != 255)
    }
    fn replace_guard(&self, source: Option<&[u8]>, to: u8, region: Option<&[bool]>) -> Result<()> {
        if !self.paintable || source.is_some_and(|s| s.len() != self.source_len) {
            return Err(invalid(
                "PIC storage aliases metadata or samples; painting disabled",
            ));
        }
        if region.is_some_and(|r| r.len() != self.pixels.len()) {
            return Err(invalid("Region does not match the PIC size"));
        }
        if !self.glyphs.is_empty() && to == 255 {
            return Err(invalid("Index 255 is transparent in this glyph strip"));
        }
        Ok(())
    }
    /// Replace brush: the same circle as `paint`, writing `to` only over opaque
    /// pixels whose index is in `from` (and inside `region` when given). Only
    /// raster bytes change; transparency and span holes stay as they are.
    #[allow(clippy::too_many_arguments)]
    pub fn replace(
        &mut self,
        source: &mut [u8],
        x: usize,
        y: usize,
        radius: usize,
        from: &IndexSet,
        to: u8,
        region: Option<&[bool]>,
    ) -> Result<usize> {
        self.replace_guard(Some(source), to, region)?;
        let glyph = !self.glyphs.is_empty();
        // `brush` writes opaque pixels only (those with a source offset).
        self.brush(source, x, y, radius, |i, p| {
            if region.is_none_or(|r| r[i]) && from[p as usize] && !(glyph && p == 255) {
                to
            } else {
                p
            }
        })
    }
    /// Pixels `replace_all` would change, without changing anything.
    pub fn replace_count(&self, from: &IndexSet, to: u8, region: Option<&[bool]>) -> Result<usize> {
        self.replace_guard(None, to, region)?;
        Ok((0..self.pixels.len())
            .filter(|i| {
                let p = self.pixels[*i];
                p != to && from[p as usize] && self.replaceable(*i, region)
            })
            .count())
    }
    /// Replace every opaque `from` pixel of the image, or of `region` (for
    /// example a `footprint`), with `to`. Raster bytes only; returns the
    /// number of pixels changed, and with none the bytes are untouched.
    pub fn replace_all(
        &mut self,
        source: &mut [u8],
        from: &IndexSet,
        to: u8,
        region: Option<&[bool]>,
    ) -> Result<usize> {
        self.replace_guard(Some(source), to, region)?;
        let mut count = 0;
        for i in 0..self.pixels.len() {
            let p = self.pixels[i];
            if p != to && from[p as usize] && self.replaceable(i, region) {
                source[self.offsets[i] as usize] = to;
                self.pixels[i] = to;
                count += 1;
            }
        }
        Ok(count)
    }
}
#[cfg(test)]
mod replace_tests {
    use super::*;
    /// A 4 x 3 span PIC: row 0 x 1..=2, row 2 x 0..=3, the rest holes.
    fn spans() -> Vec<u8> {
        let mut b = vec![0; 64 + 6 + 30];
        b[0] = 1;
        for (at, n) in [(2, 4u32), (6, 3), (10, 64), (14, 6), (26, 70), (30, 30)] {
            b[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        b[64..70].copy_from_slice(&[5, 9, 5, 5, 0, 9]);
        let span = |b: &mut Vec<u8>, at: usize, v: [u16; 3], off: u32| {
            for (k, n) in v.iter().enumerate() {
                b[at + k * 2..at + k * 2 + 2].copy_from_slice(&n.to_le_bytes());
            }
            b[at + 6..at + 10].copy_from_slice(&off.to_le_bytes());
        };
        span(&mut b, 70, [0, 1, 2], 0);
        span(&mut b, 80, [2, 0, 3], 2);
        b[90..92].copy_from_slice(&65535u16.to_le_bytes());
        b
    }
    fn changed(a: &[u8], b: &[u8]) -> Vec<usize> {
        a.iter()
            .zip(b)
            .enumerate()
            .filter(|(_, (x, y))| x != y)
            .map(|(i, _)| i)
            .collect()
    }
    #[test]
    fn tolerance_maps_palette_distance_to_indices() {
        for c6 in 0..=63u16 {
            let c8 = ((c6 * 255 + 31) / 63) as i32;
            assert_eq!((c8 * 63 + 127) / 255, c6 as i32);
        }
        let mut raw = [0u8; 768];
        raw[30..33].copy_from_slice(&[20, 20, 20]);
        raw[33..36].copy_from_slice(&[20, 20, 20]); // index 11: the same color
        raw[36..39].copy_from_slice(&[21, 20, 20]); // index 12: 1 step
        raw[39..42].copy_from_slice(&[22, 22, 21]); // index 13: 3 steps
        raw[42..45].copy_from_slice(&[24, 20, 20]); // index 14: 4 steps
        for i in 15..256 {
            raw[i * 3..i * 3 + 3].copy_from_slice(&[63, 63, 63]);
        }
        let colors = palette(&raw).unwrap();
        let on = |s: IndexSet| (0..256).filter(|i| s[*i]).collect::<Vec<_>>();
        assert_eq!(on(matching(&colors, 10, 0)), [10], "exact index only");
        assert_eq!(on(matching(&colors, 10, 1)), [10, 11, 12]);
        assert_eq!(on(matching(&colors, 10, 3)), [10, 11, 12, 13]);
        assert_eq!(on(matching(&colors, 10, 4)), [10, 11, 12, 13, 14]);
        assert_eq!(
            matching(&colors, 10, 255),
            matching(&colors, 10, MAX_TOLERANCE)
        );
        assert!(on(matching(&colors, 0, MAX_TOLERANCE)).len() < 256);
    }
    #[test]
    fn replace_brush_changes_only_matching_pixels_in_the_circle() {
        let original = demo();
        let mut bytes = original.clone();
        let mut p = Pic::parse(&bytes).unwrap();
        let before = p.clone();
        // Pixel (5, 5) is index 32; its 7 px circle also covers 0, 16 and 48.
        let from = matching(&p.colors(&[[0; 3]; 256]), 32, 0);
        let n = p.replace(&mut bytes, 5, 5, 7, &from, 200, None).unwrap();
        let expected: Vec<usize> = (0..1024)
            .filter(|i| {
                let (x, y) = ((i % 32) as i64, (i / 32) as i64);
                before.pixels[*i] == 32 && (x - 5).pow(2) + (y - 5).pow(2) <= 49
            })
            .collect();
        assert_eq!(n, expected.len());
        assert!(n > 10);
        assert_eq!(
            changed(&original, &bytes),
            expected.iter().map(|i| 64 + i).collect::<Vec<_>>()
        );
        assert!(p
            .pixels
            .iter()
            .zip(&before.pixels)
            .all(|(a, b)| a == b || (*b == 32 && *a == 200)));
        // A second pass over the same pixels changes nothing more.
        assert_eq!(p.replace(&mut bytes, 5, 5, 7, &from, 200, None).unwrap(), 0);
        assert_eq!(Pic::parse(&bytes).unwrap().pixels, p.pixels);
    }
    #[test]
    fn replace_all_whole_image_and_footprint_regions() {
        let original = demo();
        let p0 = Pic::parse(&original).unwrap();
        let from = matching(&p0.colors(&[[0; 3]; 256]), 32, 0);
        let total = p0.pixels.iter().filter(|v| **v == 32).count();
        let mut bytes = original.clone();
        let mut p = p0.clone();
        assert_eq!(p.replace_count(&from, 7, None).unwrap(), total);
        assert_eq!(p.replace_all(&mut bytes, &from, 7, None).unwrap(), total);
        assert!(changed(&original, &bytes)
            .iter()
            .all(|at| *at >= 64 && p0.pixels[at - 64] == 32 && bytes[*at] == 7));
        assert_eq!(changed(&original, &bytes).len(), total);
        assert_eq!(
            &bytes[64 + 1024..],
            &original[64 + 1024..],
            "palette untouched"
        );
        // The UV square (0,31)-(7,24) is the top-left 8 x 8 pixels (V flipped).
        let square: &[[i32; 2]] = &[[0, 31], [7, 31], [7, 24], [0, 24]];
        let region = footprint(32, 32, &[square]).unwrap();
        assert_eq!(region.iter().filter(|r| **r).count(), 64);
        assert!(region[0] && region[7 * 32 + 7] && !region[8] && !region[8 * 32]);
        let mut bytes = original.clone();
        let mut p = p0.clone();
        let n = p.replace_all(&mut bytes, &from, 7, Some(&region)).unwrap();
        let inside = (0..1024)
            .filter(|i| region[*i] && p0.pixels[*i] == 32)
            .count();
        assert_eq!(n, inside);
        assert!(n > 0 && n < total);
        assert!(changed(&original, &bytes)
            .iter()
            .all(|at| region[at - 64] && p0.pixels[at - 64] == 32));
        // The brush honours the same region.
        let mut bytes = original.clone();
        let mut p = p0.clone();
        assert!(
            p.replace(&mut bytes, 8, 3, 7, &from, 7, Some(&region))
                .unwrap()
                > 0
        );
        assert!(changed(&original, &bytes).iter().all(|at| region[at - 64]));
    }
    #[test]
    fn footprints_cover_triangles_edges_and_refuse_bad_input() {
        // Right triangle with legs of 4 pixels: 15 centres inside or on it.
        let tri: &[[i32; 2]] = &[[0, 7], [4, 7], [0, 3]];
        let m = footprint(8, 8, &[tri]).unwrap();
        assert_eq!(m.iter().filter(|v| **v).count(), 15);
        assert!(m[0] && m[4] && m[4 * 8] && !m[4 * 8 + 1]);
        // A degenerate face still covers the pixels along it.
        let line: &[[i32; 2]] = &[[1, 6], [5, 6], [3, 6]];
        let m = footprint(8, 8, &[line]).unwrap();
        assert_eq!(
            (0..8).filter(|x| m[8 + x]).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        assert_eq!(m.iter().filter(|v| **v).count(), 5);
        // Clipped to the image; corners far outside are bounded.
        let big: &[[i32; 2]] = &[[-50, -50], [60, -50], [60, 60], [-50, 60]];
        assert!(footprint(8, 8, &[big]).unwrap().iter().all(|v| *v));
        assert!(footprint(8, 8, &[]).unwrap().iter().all(|v| !*v));
        let far: &[[i32; 2]] = &[[0, 0], [70000, 0], [0, 1]];
        assert!(footprint(8, 8, &[far]).is_err());
        assert!(footprint(0, 8, &[]).is_err());
        let many = vec![tri; 4097];
        assert!(footprint(8, 8, &many).is_err());
    }
    #[test]
    fn span_pics_keep_holes_tables_and_transparency() {
        let original = spans();
        let p0 = Pic::parse(&original).unwrap();
        assert_eq!(
            p0.mask,
            [false, true, true, false, false, false, false, false, true, true, true, true]
        );
        // Holes read as index 0; replacing 0 must leave them alone.
        let zero = matching(&[[0; 3]; 256], 0, 0);
        let mut bytes = original.clone();
        let mut p = p0.clone();
        assert_eq!(p.replace_all(&mut bytes, &zero, 3, None).unwrap(), 1);
        assert_eq!(changed(&original, &bytes), [68]);
        assert_eq!(Pic::parse(&bytes).unwrap().mask, p0.mask);
        let five = matching(&[[0; 3]; 256], 5, 0);
        let mut bytes = original.clone();
        let mut p = p0.clone();
        assert_eq!(p.replace(&mut bytes, 1, 1, 2, &five, 3, None).unwrap(), 3);
        assert_eq!(changed(&original, &bytes), [64, 66, 67]);
        assert_eq!(&bytes[70..], &original[70..], "span table untouched");
        assert_eq!(Pic::parse(&bytes).unwrap().mask, p0.mask);
    }
    #[test]
    fn no_op_identity_bounds_and_truncation() {
        let original = demo();
        let p0 = Pic::parse(&original).unwrap();
        let colors = p0.colors(&[[0; 3]; 256]);
        let mut bytes = original.clone();
        let mut p = p0.clone();
        // Same index, and an index the image never uses: identical bytes.
        let same = matching(&colors, 16, 0);
        assert_eq!(p.replace_all(&mut bytes, &same, 16, None).unwrap(), 0);
        assert_eq!(p.replace(&mut bytes, 4, 4, 7, &same, 16, None).unwrap(), 0);
        let unused = matching(&colors, 3, 0);
        assert_eq!(p.replace_count(&unused, 9, None).unwrap(), 0);
        assert_eq!(p.replace_all(&mut bytes, &unused, 9, None).unwrap(), 0);
        assert_eq!(bytes, original);
        // Bounds: brush outside the image, wrong region size, truncated source.
        let from = matching(&colors, 0, 0);
        assert!(p.replace(&mut bytes, 32, 0, 1, &from, 9, None).is_err());
        assert!(p.replace(&mut bytes, 0, 0, 33, &from, 9, None).is_err());
        assert!(p
            .replace_all(&mut bytes, &from, 9, Some(&[true; 3]))
            .is_err());
        assert!(p.replace_count(&from, 9, Some(&[true; 3])).is_err());
        let mut short = original[..original.len() - 1].to_vec();
        assert!(p.replace_all(&mut short, &from, 9, None).is_err());
        assert!(p.replace(&mut short, 1, 1, 1, &from, 9, None).is_err());
        assert_eq!(short, original[..original.len() - 1]);
        assert_eq!(bytes, original);
        assert_eq!(p.pixels, p0.pixels);
        // Truncated payloads never parse, so nothing is replaced in them.
        for end in [0, 63, 64, 1000, original.len() - 1] {
            assert!(Pic::parse(&original[..end]).is_err());
        }
    }
    #[test]
    fn glyph_strips_keep_index_255_transparent() {
        let mut bytes = demo();
        let at = bytes.len() as u32;
        bytes[42..46].copy_from_slice(&at.to_le_bytes());
        bytes.extend(vec![0; 256 * 6]);
        bytes[64] = 255;
        let original = bytes.clone();
        let mut p = Pic::parse(&bytes).unwrap();
        assert!(!p.glyphs.is_empty());
        let expected = p.pixels.iter().filter(|v| **v != 255 && **v != 4).count();
        let all = [true; 256];
        assert!(p.replace_all(&mut bytes, &all, 255, None).is_err());
        assert_eq!(p.replace_all(&mut bytes, &all, 4, None).unwrap(), expected);
        assert_eq!(bytes[64], 255, "transparent glyph pixel kept");
        assert_eq!(&bytes[64 + 1024..], &original[64 + 1024..]);
    }
}
