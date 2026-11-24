use anyhow::{bail, Result};

/// The model always sends LF; if the file uses CRLF, convert its text so multi-line matches and writes keep the file's endings.
pub(super) fn match_line_endings(file: &str, text: &str) -> String {
    if file.contains("\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_owned()
    }
}

pub(super) fn exact_replace(text: &str, old: &str, new: &str) -> Result<String> {
    let count = text.matches(old).count();
    if count != 1 {
        bail!("old_text khớp {count} lần; cần đúng 1 lần");
    }
    Ok(text.replacen(old, new, 1))
}
