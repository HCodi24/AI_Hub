use super::image::{decode_base64, ext_for_mime, image_type, mime_for_ext};
use super::*;

#[test]
fn validates_image_magic_bytes() {
    for (bytes, mime) in [
        (b"\x89PNG\r\n\x1a\nxx".as_slice(), "image/png"),
        (&[0xff, 0xd8, 0xff, 1], "image/jpeg"),
        (b"RIFFxxxxWEBP".as_slice(), "image/webp"),
        (b"GIF89a".as_slice(), "image/gif"),
    ] {
        assert_eq!(image_type(bytes).map(|v| v.0), Some(mime));
    }
    assert!(image_type(b"not a png").is_none());
}
#[test]
fn base64_roundtrip() {
    let bytes = b"image bytes";
    assert_eq!(
        decode_base64(&encode_base64(bytes)).as_deref(),
        Some(bytes.as_slice())
    );
}
#[test]
fn serves_only_files_inside_chat_dir() {
    let temp = std::env::temp_dir().join(format!("ai-hub-attachments-{}", Uuid::new_v4()));
    let dir = temp.join("chat");
    fs::create_dir_all(&dir).unwrap();
    let outside = temp.join("outside.png");
    fs::write(&outside, b"x").unwrap();
    let link = dir.join("link.png");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&outside, &link).unwrap();
    assert!(safe_file(&link, &dir).is_none());
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn image_mime_and_extension_mappings_are_inverses() {
    for (mime, ext) in [
        ("image/png", "png"),
        ("image/jpeg", "jpg"),
        ("image/webp", "webp"),
        ("image/gif", "gif"),
    ] {
        assert_eq!(ext_for_mime(mime), Some(ext));
        assert_eq!(mime_for_ext(ext), Some(mime));
    }
    assert_eq!(ext_for_mime("image/svg+xml"), None);
    assert_eq!(mime_for_ext("jpeg"), None);
}

#[test]
fn rejects_invalid_base64() {
    for input in ["AAA", "AA?=", "A==="] {
        assert!(decode_base64(input).is_none(), "{input}");
    }
}

#[test]
fn accepts_a_file_inside_the_chat_dir() {
    let dir = std::env::temp_dir().join(format!("ai-hub-attachments-{}", Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    let file = dir.join("inside.png");
    fs::write(&file, b"x").unwrap();

    let expected = fs::canonicalize(&file);
    let found = safe_file(&file, &dir);
    fs::remove_file(&file).unwrap();
    fs::remove_dir(&dir).unwrap();

    assert_eq!(found, Some(expected.unwrap()));
}
