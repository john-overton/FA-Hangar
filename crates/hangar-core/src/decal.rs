//! Bounded PNG/PIC decal artwork, flattened into existing opaque indexed pixels.
use crate::{invalid, picture::Pic, slice, Result};
use alloc::{collections::BTreeMap, vec::Vec};
#[derive(Clone, Debug)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<[u8; 4]>,
}
fn be(b: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_be_bytes(slice(b, at, 4)?.try_into().unwrap()) as usize)
}
fn crc(b: &[u8]) -> u32 {
    let mut c = !0u32;
    for v in b {
        c ^= *v as u32;
        for _ in 0..8 {
            c = (c >> 1) ^ if c & 1 != 0 { 0xedb88320 } else { 0 };
        }
    }
    !c
}
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
fn sample(row: &[u8], pixel: usize, channel: usize, channels: usize, depth: usize) -> u16 {
    let bit = (pixel * channels + channel) * depth;
    if depth < 8 {
        ((row[bit / 8] >> (8 - depth - bit % 8)) & ((1 << depth) - 1)) as u16
    } else if depth == 8 {
        row[bit / 8] as u16
    } else {
        u16::from_be_bytes([row[bit / 8], row[bit / 8 + 1]])
    }
}
impl Image {
    pub fn from_pic(bytes: &[u8], base: &[[u8; 3]; 256]) -> Result<Self> {
        let pic = Pic::parse(bytes)?;
        let rgba = pic
            .rgba(base)
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        Ok(Self {
            width: pic.width,
            height: pic.height,
            rgba,
        })
    }
    pub fn png(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 16 * 1024 * 1024 || slice(bytes, 0, 8)? != b"\x89PNG\r\n\x1a\n" {
            return Err(invalid("Choose a PNG under 16 MiB"));
        }
        let (mut width, mut height, mut depth, mut kind) = (0, 0, 0, 0);
        let (mut header, mut ended, mut idat_seen, mut idat_closed) = (false, false, false, false);
        let (mut palette, mut transparency, mut compressed) = (Vec::new(), Vec::new(), Vec::new());
        let mut at = 8;
        while at < bytes.len() {
            let n = be(bytes, at)?;
            let tag = slice(bytes, at + 4, 4)?;
            let data = slice(bytes, at + 8, n)?;
            let end = at.checked_add(12 + n).ok_or("PNG chunk size overflow")?;
            if crc(slice(bytes, at + 4, n + 4)?) != be(bytes, at + 8 + n)? as u32 {
                return Err(invalid("PNG chunk checksum failed"));
            }
            if !header && tag != b"IHDR" {
                return Err(invalid("PNG header must be first"));
            }
            if idat_seen && tag != b"IDAT" {
                idat_closed = true;
            }
            match tag {
                b"IHDR" => {
                    if header || n != 13 {
                        return Err(invalid("Invalid PNG header"));
                    }
                    header = true;
                    width = be(data, 0)?;
                    height = be(data, 4)?;
                    depth = data[8] as usize;
                    kind = data[9];
                    if width == 0 || height == 0 || width > 2048 || height > 2048 {
                        return Err(invalid("Decal PNG dimensions must be 1..2048"));
                    }
                    if data[10] != 0 || data[11] != 0 || data[12] != 0 {
                        return Err(invalid(
                            "Use a non-interlaced PNG with standard compression",
                        ));
                    }
                    let valid = match kind {
                        0 => matches!(depth, 1 | 2 | 4 | 8 | 16),
                        2 | 4 | 6 => matches!(depth, 8 | 16),
                        3 => matches!(depth, 1 | 2 | 4 | 8),
                        _ => false,
                    };
                    if !valid {
                        return Err(invalid("Unsupported PNG color type or sample depth"));
                    }
                }
                b"acTL" | b"fcTL" | b"fdAT" => {
                    return Err(invalid("Use a static PNG for decal artwork"))
                }
                b"PLTE" => {
                    if idat_seen || !palette.is_empty() || n == 0 || n > 768 || !n.is_multiple_of(3)
                    {
                        return Err(invalid("Invalid PNG palette"));
                    }
                    palette = data.to_vec();
                }
                b"tRNS" => {
                    if idat_seen || !transparency.is_empty() {
                        return Err(invalid("Invalid PNG transparency"));
                    }
                    transparency = data.to_vec();
                }
                b"IDAT" => {
                    if idat_closed {
                        return Err(invalid("PNG image chunks must be consecutive"));
                    }
                    idat_seen = true;
                    compressed.extend_from_slice(data);
                }
                b"IEND" => {
                    if n != 0 || !idat_seen || end != bytes.len() {
                        return Err(invalid("Invalid PNG end"));
                    }
                    ended = true;
                    break;
                }
                _ if tag[0] & 32 == 0 => return Err(invalid("Unsupported critical PNG chunk")),
                _ => {}
            }
            at = end;
        }
        if !ended {
            return Err(invalid("Incomplete PNG"));
        }
        if kind == 3
            && (palette.is_empty()
                || palette.len() / 3 > 1 << depth
                || transparency.len() > palette.len() / 3)
        {
            return Err(invalid("Indexed PNG palette/transparency mismatch"));
        }
        if !transparency.is_empty()
            && !match kind {
                0 => transparency.len() == 2,
                2 => transparency.len() == 6,
                3 => true,
                _ => false,
            }
        {
            return Err(invalid("PNG transparency does not match color type"));
        }
        let channels = match kind {
            0 | 3 => 1,
            2 => 3,
            4 => 2,
            6 => 4,
            _ => unreachable!(),
        };
        let row_len = (width * channels * depth).div_ceil(8);
        let expected = (row_len + 1) * height;
        let bpp = (channels * depth).div_ceil(8).max(1);
        let mut raw =
            miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&compressed, expected)
                .map_err(|_| invalid("Invalid PNG compressed data or decoded size"))?;
        if raw.len() != expected {
            return Err(invalid("PNG decoded length mismatch"));
        }
        for y in 0..height {
            let base = y * (row_len + 1);
            let filter = raw[base];
            if filter > 4 {
                return Err(invalid("Invalid PNG row filter"));
            }
            for x in 0..row_len {
                let i = base + 1 + x;
                let a = if x >= bpp { raw[i - bpp] } else { 0 };
                let b = if y > 0 { raw[i - row_len - 1] } else { 0 };
                let c = if y > 0 && x >= bpp {
                    raw[i - row_len - 1 - bpp]
                } else {
                    0
                };
                let predictor = match filter {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((a as u16 + b as u16) / 2) as u8,
                    4 => paeth(a, b, c),
                    _ => unreachable!(),
                };
                raw[i] = raw[i].wrapping_add(predictor);
            }
        }
        let max = (1u32 << depth) - 1;
        let convert = |n: u16| ((n as u32 * 255 + max / 2) / max) as u8;
        let mut rgba = Vec::with_capacity(width * height);
        for y in 0..height {
            let row = &raw[y * (row_len + 1) + 1..(y + 1) * (row_len + 1)];
            for x in 0..width {
                let s = |c| sample(row, x, c, channels, depth);
                let color = match kind {
                    0 => {
                        let g = convert(s(0));
                        let transparent = transparency.len() == 2
                            && s(0) == u16::from_be_bytes([transparency[0], transparency[1]]);
                        [g, g, g, if transparent { 0 } else { 255 }]
                    }
                    2 => {
                        let transparent = transparency.len() == 6
                            && (0..3).all(|c| {
                                s(c) == u16::from_be_bytes([
                                    transparency[c * 2],
                                    transparency[c * 2 + 1],
                                ])
                            });
                        [
                            convert(s(0)),
                            convert(s(1)),
                            convert(s(2)),
                            if transparent { 0 } else { 255 },
                        ]
                    }
                    3 => {
                        let i = s(0) as usize;
                        if i * 3 + 2 >= palette.len() {
                            return Err(invalid("PNG palette index outside table"));
                        }
                        [
                            palette[i * 3],
                            palette[i * 3 + 1],
                            palette[i * 3 + 2],
                            transparency.get(i).copied().unwrap_or(255),
                        ]
                    }
                    4 => {
                        let g = convert(s(0));
                        [g, g, g, convert(s(1))]
                    }
                    6 => [convert(s(0)), convert(s(1)), convert(s(2)), convert(s(3))],
                    _ => unreachable!(),
                };
                rgba.push(color);
            }
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
    pub fn demo() -> Self {
        let mut rgba = vec![[0; 4]; 16 * 16];
        for y in 2..14 {
            for x in 2..14 {
                if x == y || x + y == 15 || x == 7 || x == 8 {
                    rgba[y * 16 + x] = [255, 255, 255, 255];
                }
            }
        }
        Self {
            width: 16,
            height: 16,
            rgba,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub center: [i32; 2],
    pub width: usize,
    pub degrees: i32,
    pub opacity: u8,
    pub mirror: bool,
}
/// Always rebuild from the unmodified destination snapshot; adjustment previews\n/// do not repeatedly paint over themselves. Destination span holes stay holes.
pub fn composite(
    source: &[u8],
    base: &[[u8; 3]; 256],
    image: &Image,
    p: Placement,
) -> Result<(Vec<u8>, Pic, usize)> {
    if image.width == 0
        || image.height == 0
        || image.rgba.len() != image.width * image.height
        || p.width == 0
        || p.width > 2048
        || p.opacity > 100
        || p.center.iter().any(|v| !(-8192..=8192).contains(v))
    {
        return Err(invalid("Decal placement exceeds bounds"));
    }
    let height = (p.width * image.height / image.width).max(1);
    if height > 2048 {
        return Err(invalid("Decal height exceeds 2048 pixels"));
    }
    let mut pic = Pic::parse(source)?;
    if !pic.paintable {
        return Err(invalid("PIC uses aliased raster storage"));
    }
    let colors = pic.colors(base);
    let mut pixels = pic.pixels.clone();
    let (s, c) = crate::model::sin_cos(p.degrees);
    let radius = (p.width + height) as i32;
    let (x0, x1) = (
        (p.center[0] - radius).max(0),
        (p.center[0] + radius + 1).min(pic.width as i32),
    );
    let (y0, y1) = (
        (p.center[1] - radius).max(0),
        (p.center[1] + radius + 1).min(pic.height as i32),
    );
    let mut cache = BTreeMap::<u32, u8>::new();
    for y in y0..y1 {
        for x in x0..x1 {
            let i = y as usize * pic.width + x as usize;
            if !pic.mask[i] {
                continue;
            }
            let dx = (x * 2 + 1 - p.center[0] * 2) as i64;
            let dy = (y * 2 + 1 - p.center[1] * 2) as i64;
            let mut u = (dx * c as i64 + dy * s as i64) / 1024;
            let v = (-dx * s as i64 + dy * c as i64) / 1024;
            if p.mirror {
                u = -u;
            }
            let a = u + p.width as i64;
            let b = v + height as i64;
            if a < 0 || b < 0 || a >= p.width as i64 * 2 || b >= height as i64 * 2 {
                continue;
            }
            let sx = (a * image.width as i64 / (p.width as i64 * 2)) as usize;
            let sy = (b * image.height as i64 / (height as i64 * 2)) as usize;
            let src = image.rgba[sy * image.width + sx];
            let alpha = src[3] as u32 * p.opacity as u32 / 100;
            if alpha == 0 {
                continue;
            }
            let dst = colors[pic.pixels[i] as usize];
            let rgb: [u8; 3] = core::array::from_fn(|k| {
                ((src[k] as u32 * alpha + dst[k] as u32 * (255 - alpha) + 127) / 255) as u8
            });
            let key = (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32;
            let index = if let Some(index) = cache.get(&key) {
                *index
            } else {
                let n = colors
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, color)| {
                        (0..3)
                            .map(|k| (color[k] as i32 - rgb[k] as i32).pow(2))
                            .sum::<i32>()
                    })
                    .unwrap()
                    .0 as u8;
                if cache.len() < 4096 {
                    cache.insert(key, n);
                }
                n
            };
            pixels[i] = index;
        }
    }
    let mut bytes = source.to_vec();
    let changed = pic.patch_indices(&mut bytes, &pixels)?;
    Ok((bytes, pic, changed))
}

pub const NATIONAL_NAMES: [&str; 4] = [
    "US stars and bars",
    "UK roundel",
    "French roundel",
    "Japan roundel",
];
impl Image {
    /// Compact built-in artwork; exact unit/squadron artwork can be imported as PNG.
    pub fn national(index: usize) -> Result<Self> {
        if index >= NATIONAL_NAMES.len() {
            return Err(invalid("Unknown national marking"));
        }
        let mut rgba = vec![[0; 4]; 64 * 32];
        let blue = [24, 48, 100, 255];
        let red = [190, 24, 40, 255];
        let white = [255; 4];
        for y in 0..32 {
            for x in 0..64 {
                let dx = x as i32 - 32;
                let dy = y as i32 - 16;
                let d = dx * dx + dy * dy;
                let mut color = [0; 4];
                if index == 0 {
                    if (7..=57).contains(&x) && (9..=23).contains(&y) {
                        color = blue;
                    }
                    if (9..=55).contains(&x) && (11..=21).contains(&y) {
                        color = white;
                    }
                    if (9..=55).contains(&x) && (15..=17).contains(&y) {
                        color = red;
                    }
                    if d <= 15 * 15 {
                        color = blue;
                    }
                } else if index == 1 || index == 2 {
                    if d <= 15 * 15 {
                        color = if index == 1 { blue } else { red };
                    }
                    if d <= 10 * 10 {
                        color = white;
                    }
                    if d <= 5 * 5 {
                        color = if index == 1 { red } else { blue };
                    }
                } else if d <= 14 * 14 {
                    color = red;
                }
                rgba[y * 64 + x] = color;
            }
        }
        if index == 0 {
            let points: Vec<_> = (0..10)
                .map(|i| {
                    let (s, c) = crate::model::sin_cos(-90 + i * 36);
                    let r = if i % 2 == 0 { 14 } else { 5 };
                    [32 + c * r / 1024, 16 + s * r / 1024]
                })
                .collect();
            for y in 1..31 {
                for x in 16..48 {
                    let mut inside = false;
                    for i in 0..points.len() {
                        let a = points[i];
                        let b = points[(i + 1) % points.len()];
                        if (a[1] > y) != (b[1] > y)
                            && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]
                        {
                            inside = !inside;
                        }
                    }
                    if inside {
                        rgba[y as usize * 64 + x as usize] = white;
                    }
                }
            }
        }
        Ok(Self {
            width: 64,
            height: 32,
            rgba,
        })
    }
    pub fn text(text: &str, color: [u8; 3]) -> Result<Self> {
        let text = text.trim().to_ascii_uppercase();
        if text.is_empty() || text.len() > 24 {
            return Err(invalid(
                "Tail text must be 1..24 letters/digits, space, dash, slash or period",
            ));
        }
        let width = text.len() * 6 - 1;
        let mut rgba = vec![[0; 4]; width * 7];
        for (i, ch) in text.chars().enumerate() {
            let rows = match ch {
                'A' => [14, 17, 17, 31, 17, 17, 17],
                'B' => [30, 17, 17, 30, 17, 17, 30],
                'C' => [14, 17, 16, 16, 16, 17, 14],
                'D' => [30, 17, 17, 17, 17, 17, 30],
                'E' => [31, 16, 16, 30, 16, 16, 31],
                'F' => [31, 16, 16, 30, 16, 16, 16],
                'G' => [14, 17, 16, 23, 17, 17, 15],
                'H' => [17, 17, 17, 31, 17, 17, 17],
                'I' => [31, 4, 4, 4, 4, 4, 31],
                'J' => [7, 2, 2, 2, 2, 18, 12],
                'K' => [17, 18, 20, 24, 20, 18, 17],
                'L' => [16, 16, 16, 16, 16, 16, 31],
                'M' => [17, 27, 21, 21, 17, 17, 17],
                'N' => [17, 25, 25, 21, 19, 19, 17],
                'O' => [14, 17, 17, 17, 17, 17, 14],
                'P' => [30, 17, 17, 30, 16, 16, 16],
                'Q' => [14, 17, 17, 17, 21, 18, 13],
                'R' => [30, 17, 17, 30, 20, 18, 17],
                'S' => [15, 16, 16, 14, 1, 1, 30],
                'T' => [31, 4, 4, 4, 4, 4, 4],
                'U' => [17, 17, 17, 17, 17, 17, 14],
                'V' => [17, 17, 17, 17, 17, 10, 4],
                'W' => [17, 17, 17, 21, 21, 21, 10],
                'X' => [17, 17, 10, 4, 10, 17, 17],
                'Y' => [17, 17, 10, 4, 4, 4, 4],
                'Z' => [31, 1, 2, 4, 8, 16, 31],
                '0' => [14, 17, 19, 21, 25, 17, 14],
                '1' => [4, 12, 4, 4, 4, 4, 14],
                '2' => [14, 17, 1, 2, 4, 8, 31],
                '3' => [30, 1, 1, 14, 1, 1, 30],
                '4' => [2, 6, 10, 18, 31, 2, 2],
                '5' => [31, 16, 16, 30, 1, 1, 30],
                '6' => [14, 16, 16, 30, 17, 17, 14],
                '7' => [31, 1, 2, 4, 8, 8, 8],
                '8' => [14, 17, 17, 14, 17, 17, 14],
                '9' => [14, 17, 17, 15, 1, 1, 14],
                '-' => [0, 0, 0, 31, 0, 0, 0],
                '/' => [1, 2, 2, 4, 8, 8, 16],
                '.' => [0, 0, 0, 0, 0, 6, 6],
                ' ' => [0; 7],
                _ => return Err(invalid("Unsupported tail-text character")),
            };
            for (y, row) in rows.iter().enumerate() {
                for x in 0..5 {
                    if row & (1 << (4 - x)) != 0 {
                        rgba[y * width + i * 6 + x] = [color[0], color[1], color[2], 255];
                    }
                }
            }
        }
        Ok(Self {
            width,
            height: 7,
            rgba,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let at = out.len();
        out.extend(tag);
        out.extend(data);
        let c = crc(&out[at..]);
        out.extend(c.to_be_bytes());
    }
    fn png(
        width: u32,
        height: u32,
        depth: u8,
        kind: u8,
        raw: &[u8],
        extra: &[(&[u8; 4], Vec<u8>)],
    ) -> Vec<u8> {
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = width.to_be_bytes().to_vec();
        ihdr.extend(height.to_be_bytes());
        ihdr.extend([depth, kind, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        for (tag, data) in extra {
            chunk(&mut out, tag, data);
        }
        chunk(
            &mut out,
            b"IDAT",
            &miniz_oxide::deflate::compress_to_vec_zlib(raw, 6),
        );
        chunk(&mut out, b"IEND", &[]);
        out
    }
    #[test]
    fn png_filters_alpha_and_palette_depths() {
        let mut raw = Vec::new();
        let mut previous = vec![0; 24];
        let mut expected = Vec::new();
        for y in 0..5 {
            let row: Vec<u8> = (0..24).map(|x| x * 7 + y * 23).collect();
            raw.push(y);
            for x in 0..24 {
                let a = if x >= 4 { row[x - 4] } else { 0 };
                let b = previous[x];
                let c = if x >= 4 { previous[x - 4] } else { 0 };
                let p = match y {
                    0 => 0,
                    1 => a,
                    2 => b,
                    3 => ((a as u16 + b as u16) / 2) as u8,
                    _ => paeth(a, b, c),
                };
                raw.push(row[x].wrapping_sub(p));
            }
            expected.extend(row.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]));
            previous = row;
        }
        let bytes = png(6, 5, 8, 6, &raw, &[]);
        let decoded = Image::png(&bytes).unwrap();
        assert_eq!(decoded.rgba, expected);
        let bytes = png(
            4,
            1,
            2,
            3,
            &[0, 0b00011011],
            &[
                (
                    b"PLTE",
                    vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255],
                ),
                (b"tRNS", vec![255, 128, 0, 255]),
            ],
        );
        let decoded = Image::png(&bytes).unwrap();
        assert_eq!(decoded.rgba[1], [0, 255, 0, 128]);
        assert_eq!(decoded.rgba[2], [0, 0, 255, 0]);
        let bytes = png(1, 1, 16, 6, &[0, 255, 255, 0, 0, 128, 128, 128, 128], &[]);
        assert_eq!(Image::png(&bytes).unwrap().rgba[0], [255, 0, 128, 128]);
        let mut corrupt = bytes.clone();
        corrupt[18] ^= 1;
        assert!(Image::png(&corrupt).is_err());
        assert!(Image::png(&bytes[..bytes.len() - 1]).is_err());
    }
    #[test]
    fn decal_preview_is_repeatable_preserves_metadata_and_alpha() {
        let source = crate::picture::demo();
        let base = [[0; 3]; 256];
        let original = Pic::parse(&source).unwrap();
        let image = Image::text("AF 01", [255; 3]).unwrap();
        let p = Placement {
            center: [16, 16],
            width: 24,
            degrees: 0,
            opacity: 100,
            mirror: false,
        };
        let (a, _, count) = composite(&source, &base, &image, p).unwrap();
        assert!(count > 0);
        let (b, _, _) = composite(&source, &base, &image, p).unwrap();
        assert_eq!(a, b);
        assert_eq!(&a[..64], &source[..64]);
        let after = Pic::parse(&a).unwrap();
        assert_eq!(original.mask, after.mask);
        assert_eq!(original.palette, after.palette);
        let transparent = Image {
            width: 1,
            height: 1,
            rgba: vec![[255, 0, 0, 0]],
        };
        let (out, _, n) = composite(&source, &base, &transparent, p).unwrap();
        assert_eq!(out, source);
        assert_eq!(n, 0);
        assert!(Image::text("BAD!", [0; 3]).is_err());
        for i in 0..NATIONAL_NAMES.len() {
            let marking = Image::national(i).unwrap();
            assert!(marking.rgba.iter().any(|p| p[3] == 0));
            assert!(marking.rgba.iter().any(|p| p[3] == 255));
        }
    }
}

#[cfg(test)]
mod compositing_tests {
    use super::*;
    #[test]
    fn transparent_spans_rotation_mirroring_and_opacity_are_preserved() {
        let mut span = vec![0; 85];
        span[0] = 1;
        for (at, n) in [(2, 2u32), (6, 1), (10, 64), (14, 1), (26, 65), (30, 20)] {
            span[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        span[67] = 1;
        span[69] = 1;
        span[75..77].copy_from_slice(&65535u16.to_le_bytes());
        let mut colors = [[0; 3]; 256];
        colors[1] = [255; 3];
        let image = Image {
            width: 1,
            height: 1,
            rgba: vec![[255; 4]],
        };
        let p = Placement {
            center: [1, 0],
            width: 2,
            degrees: 0,
            opacity: 100,
            mirror: false,
        };
        let (output, pic, n) = composite(&span, &colors, &image, p).unwrap();
        assert_eq!(n, 1);
        assert!(!pic.mask[0]);
        assert_eq!(&output[..64], &span[..64]);
        assert_eq!(&output[65..], &span[65..]);
        let mut pic = vec![0; 64 + 4 + 768];
        for (at, n) in [(2, 2u32), (6, 2), (10, 64), (14, 4), (18, 68), (22, 768)] {
            pic[at..at + 4].copy_from_slice(&n.to_le_bytes());
        }
        for (i, c) in [
            [0, 0, 0],
            [63, 0, 0],
            [0, 0, 63],
            [0, 0, 32],
            [63, 63, 63],
            [32, 0, 0],
            [0, 0, 16],
            [32, 32, 32],
        ]
        .iter()
        .enumerate()
        {
            pic[68 + i * 3..71 + i * 3].copy_from_slice(c);
        }
        let image = Image {
            width: 2,
            height: 2,
            rgba: vec![[255, 0, 0, 255], [0, 0, 255, 128], [0, 255, 0, 0], [255; 4]],
        };
        let mut p = Placement {
            center: [1, 1],
            width: 2,
            degrees: 0,
            opacity: 100,
            mirror: false,
        };
        let (_, decoded, _) = composite(&pic, &colors, &image, p).unwrap();
        assert_eq!(decoded.pixels, vec![1, 3, 0, 4]);
        p.mirror = true;
        let (_, decoded, _) = composite(&pic, &colors, &image, p).unwrap();
        assert_eq!(decoded.pixels, vec![3, 1, 4, 0]);
        p.mirror = false;
        p.degrees = 90;
        let (_, decoded, _) = composite(&pic, &colors, &image, p).unwrap();
        assert_eq!(decoded.pixels, vec![0, 1, 4, 3]);
        p.degrees = 0;
        p.opacity = 50;
        let (_, decoded, _) = composite(&pic, &colors, &image, p).unwrap();
        assert_eq!(decoded.pixels, vec![5, 6, 0, 7]);
    }
}
