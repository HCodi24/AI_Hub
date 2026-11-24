/// Largest sprite sheet accepted.
pub(super) const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Every sprite sheet is 8 columns of 192 px and 5 or 6 rows of 208 px.
const SHEET_WIDTH: u32 = 1536;
/// 6 rows (thinking, replying, idle, waiting, failed, running) or 5 without the running row.
const SHEET_HEIGHTS: [u32; 2] = [208 * 5, 208 * 6];

pub(super) fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG")
}

pub(super) fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// PNG with the pet sheet layout: 1536 px wide, 1248 px (6 rows) or 1040 px (5 rows) high.
pub(super) fn is_sheet_png(bytes: &[u8]) -> bool {
    let dimension = |at: usize| {
        bytes
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    is_png(bytes)
        && dimension(16) == Some(SHEET_WIDTH)
        && dimension(20).is_some_and(|height| SHEET_HEIGHTS.contains(&height))
}

/// PNG or WebP with the pet sheet layout.
pub(super) fn is_sheet(bytes: &[u8]) -> bool {
    is_sheet_png(bytes)
        || (is_webp(bytes)
            && webp_size(bytes).is_some_and(|(width, height)| {
                width == SHEET_WIDTH && SHEET_HEIGHTS.contains(&height)
            }))
}

/// Pixel size from a WebP header (VP8, VP8L or VP8X); `None` when it cannot be read.
pub(super) fn webp_size(b: &[u8]) -> Option<(u32, u32)> {
    let le = |at: usize, n: usize| {
        b.get(at..at + n)
            .map(|s| s.iter().rev().fold(0u32, |a, &x| a << 8 | u32::from(x)))
    };
    match b.get(12..16)? {
        b"VP8X" => Some((le(24, 3)? + 1, le(27, 3)? + 1)),
        b"VP8L" => {
            let bits = le(21, 4)?;
            (b[20] == 0x2f).then(|| ((bits & 0x3fff) + 1, (bits >> 14 & 0x3fff) + 1))
        }
        b"VP8 " => Some((le(26, 2)? & 0x3fff, le(28, 2)? & 0x3fff)),
        _ => None,
    }
}
