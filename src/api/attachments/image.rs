use super::MAX_IMAGE_BYTES;

const IMAGE_FORMATS: [(&str, &str); 4] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/webp", "webp"),
    ("image/gif", "gif"),
];

pub(super) fn ext_for_mime(mime: &str) -> Option<&'static str> {
    IMAGE_FORMATS
        .iter()
        .find_map(|&(known_mime, ext)| (mime == known_mime).then_some(ext))
}

pub(super) fn mime_for_ext(ext: &str) -> Option<&'static str> {
    IMAGE_FORMATS
        .iter()
        .find_map(|&(mime, known_ext)| (ext == known_ext).then_some(mime))
}

pub(super) fn image_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else {
        None
    }
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        result.push(TABLE[((n >> 18) & 63) as usize] as char);
        result.push(TABLE[((n >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

pub(super) fn decode_base64(input: &str) -> Option<Vec<u8>> {
    if input.len() > MAX_IMAGE_BYTES * 4 / 3 + 8 || !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.as_bytes().chunks_exact(4) {
        let mut n = 0u32;
        let mut padding = 0;
        for (index, byte) in chunk.iter().enumerate() {
            n <<= 6;
            match byte {
                b'A'..=b'Z' => n |= u32::from(byte - b'A'),
                b'a'..=b'z' => n |= u32::from(byte - b'a' + 26),
                b'0'..=b'9' => n |= u32::from(byte - b'0' + 52),
                b'+' => n |= 62,
                b'/' => n |= 63,
                b'=' if index >= 2 => padding += 1,
                _ => return None,
            }
        }
        if padding > 2 {
            return None;
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding == 0 {
            out.push(n as u8);
        }
        if out.len() > MAX_IMAGE_BYTES {
            return None;
        }
    }
    Some(out)
}
