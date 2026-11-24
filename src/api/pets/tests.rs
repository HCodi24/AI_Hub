use super::*;

#[test]
fn upload_ids_keep_legacy_ascii_names_and_distinguish_unicode_names() {
    assert_eq!(upload_id("My Pet").as_deref(), Some("my-my-pet"));
    assert_eq!(upload_id("A_B-2").as_deref(), Some("my-a-b-2"));
    assert_eq!(
        upload_id(&"a".repeat(60)).unwrap(),
        format!("my-{}", "a".repeat(45))
    );
    assert_eq!(upload_id(""), None);
    assert_eq!(upload_id(" \t\n"), None);
    assert_eq!(upload_id("_-"), None);

    let cat = upload_id("mèo").unwrap();
    let other = upload_id("mío").unwrap();
    assert_ne!(cat, other);
    assert_eq!(upload_id("mèo").as_deref(), Some(cat.as_str()));

    let unicode_only = upload_id("猫").unwrap();
    assert_eq!(unicode_only.len(), 9);
    let long = upload_id(&format!("{}猫", "a".repeat(60))).unwrap();
    for id in [cat, other, unicode_only, long] {
        assert!(safe_name(&id));
        assert!(id.len() <= 48);
    }
}

#[test]
fn pet_names_cannot_leave_the_pets_folder() {
    assert!(!safe_name("../../.ssh"));
    assert!(!safe_name("Clawd"));
    assert!(safe_name("clawd-svg"));
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes
}

#[test]
fn only_pet_sheet_pngs_are_accepted() {
    assert!(is_sheet_png(&png(1536, 1248)));
    assert!(is_sheet_png(&png(1536, 1040)));
    assert!(!is_sheet_png(&png(1536, 1872)));
    assert!(!is_sheet_png(&png(800, 1248)));
    assert!(!is_sheet_png(&png(1536, 1456)));
    assert!(!is_sheet_png(&png(1536, 1041)));
    assert!(!is_sheet_png(b"<html>not an image</html>"));
}

#[test]
fn webp_size_is_read_from_the_header() {
    let (w, h) = (1535u32.to_le_bytes(), 1871u32.to_le_bytes());
    let mut vp8x = b"RIFF\0\0\0\0WEBPVP8X\n\0\0\0\0\0\0\0".to_vec();
    vp8x.extend(&w[..3]);
    vp8x.extend(&h[..3]);
    assert_eq!(webp_size(&vp8x), Some((1536, 1872)));
    let mut vp8l = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
    vp8l.extend((1535u32 | 1871 << 14).to_le_bytes());
    assert_eq!(webp_size(&vp8l), Some((1536, 1872)));
    assert_eq!(webp_size(b"RIFF\0\0\0\0WEBPjunk"), None);
}

#[tokio::test]
async fn list_accepts_short_sheet_headers_and_rejects_wrong_dimensions() {
    let root = std::env::temp_dir().join(format!("mutis-pets-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let state = AppState::new(
        crate::config::Config::seeded(),
        root.join("config.json"),
        root.join("chats"),
        None,
        0,
    );
    for (id, width) in [("valid", 1536), ("invalid", 800)] {
        let path = downloaded_file(&state, id).unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, png(width, 1248)).unwrap();
    }

    let Json(response) = list(State(state)).await;
    assert_eq!(
        response,
        json!({ "pets": [{ "id": "valid", "name": "valid" }] })
    );
    fs::remove_dir_all(root).unwrap();
}
