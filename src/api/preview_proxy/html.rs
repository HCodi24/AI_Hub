use super::PICKER_PATH;

pub(super) fn inject(html: &[u8]) -> Vec<u8> {
    let script = format!(r#"<script src="{PICKER_PATH}"></script>"#);
    let mut output = Vec::with_capacity(html.len() + script.len());
    let start = head_end(html).unwrap_or_else(|| {
        if html.starts_with(b"\xef\xbb\xbf") {
            3
        } else {
            0
        }
    });
    output.extend_from_slice(&html[..start]);
    output.extend_from_slice(script.as_bytes());
    output.extend_from_slice(&html[start..]);
    output
}

fn head_end(html: &[u8]) -> Option<usize> {
    let mut cursor = 0;
    let mut raw_text: &[u8] = b"";

    while let Some(offset) = html[cursor..].iter().position(|byte| *byte == b'<') {
        let start = cursor + offset;
        cursor = start + 1;
        let rest = &html[cursor..];
        let closing = rest.starts_with(b"/");
        let name_start = cursor + usize::from(closing);
        let name_end = advance(html, name_start, |byte| {
            !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'<' | b'>')
        });
        let name = &html[name_start..name_end];
        if !raw_text.is_empty() {
            if !closing || !name.eq_ignore_ascii_case(raw_text) {
                continue;
            }
            raw_text = b"";
        }
        if rest.starts_with(b"!--") {
            cursor = rest[3..]
                .windows(3)
                .position(|window| window == b"-->")
                .map_or(html.len(), |offset| cursor + offset + 6);
            continue;
        }
        if name.first().is_some_and(|byte| matches!(*byte, b'!' | b'?')) {
            cursor = advance(html, name_end, |byte| byte != b'>');
            continue;
        }
        if !name.first().is_some_and(u8::is_ascii_alphabetic) {
            continue;
        }
        if closing && name.eq_ignore_ascii_case(b"head") {
            return Some(start);
        }
        if !closing
            && [
                b"script".as_slice(),
                b"style",
                b"title",
                b"textarea",
                b"iframe",
                b"xmp",
            ]
            .iter()
            .any(|tag| name.eq_ignore_ascii_case(tag))
        {
            raw_text = name;
        }

        cursor = name_end;
        while cursor < html.len() && html[cursor] != b'>' {
            let attr_start = cursor;
            cursor = advance(html, cursor, |byte| {
                !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'=' | b'>')
            });
            if attr_start == cursor {
                cursor += 1;
                continue;
            }
            cursor = skip_space(html, cursor);
            if html.get(cursor) != Some(&b'=') {
                continue;
            }
            cursor = skip_space(html, cursor + 1);
            let quote = html
                .get(cursor)
                .copied()
                .filter(|byte| matches!(*byte, b'\'' | b'"'));
            let value_start = cursor + usize::from(quote.is_some());
            cursor = advance(html, value_start, |byte| match quote {
                Some(quote) => byte != quote,
                None => !byte.is_ascii_whitespace() && byte != b'>',
            });
            if quote.is_some() && cursor < html.len() {
                cursor += 1;
            }
        }
    }

    None
}

fn advance(html: &[u8], start: usize, predicate: impl Fn(u8) -> bool) -> usize {
    html[start..]
        .iter()
        .position(|byte| !predicate(*byte))
        .map_or(html.len(), |offset| start + offset)
}

fn skip_space(html: &[u8], start: usize) -> usize {
    advance(html, start, |byte| byte.is_ascii_whitespace())
}
