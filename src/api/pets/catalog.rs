//! Pets the app may show, with where their sprite sheet comes from. Add new pets here.
/// Codex's public pet CDN (the same one the Codex TUI downloads from).
pub const CODEX_CDN: &str = "https://persistent.oaistatic.com/codex/pets/v1";

/// Built-in Codex pets; the id ends up in a URL and a file path, so only these are allowed.
pub const CODEX_PETS: [&str; 8] = ["codex", "dewey", "fireball", "rocky", "seedy", "stacky", "bsod", "null-signal"];

/// Sprite file name and download URL for a known pet id.
pub fn sprite(id: &str) -> Option<(String, String)> {
    CODEX_PETS.contains(&id).then(|| {
        let file = format!("{id}-spritesheet-v4.webp");
        let url = format!("{CODEX_CDN}/{file}");
        (file, url)
    })
}

/// A third-party pet the user can choose to download (nothing is fetched or shipped until they do).
pub struct External {
    pub id: &'static str,
    pub name: &'static str,
    /// Which AI the pet is used for, shown in the UI.
    pub provider: &'static str,
    pub url: &'static str,
    /// Where it comes from, shown in the UI.
    pub source: &'static str,
    /// Rights note shown before downloading.
    pub notice: &'static str,
}

pub const EXTERNAL: [External; 1] = [External {
    id: "clawd-svg",
    name: "Clawd",
    provider: "Claude",
    url: "https://raw.githubusercontent.com/godxxy1229/clawd-codex-pet/main/spritesheet.png",
    source: "github.com/godxxy1229/clawd-codex-pet",
    notice: "Nhân vật Clawd thuộc bản quyền Anthropic; đây là bản fan không chính thức, chỉ dùng cá nhân, không thương mại.",
}];

pub fn external(id: &str) -> Option<&'static External> {
    EXTERNAL.iter().find(|pet| pet.id == id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_known_pets_have_a_sprite() {
        assert_eq!(super::sprite("codex").unwrap().0, "codex-spritesheet-v4.webp");
        assert!(super::sprite("../config").is_none());
    }

    #[test]
    fn external_pets_are_looked_up_by_exact_id() {
        assert!(super::external("clawd-svg").is_some());
        assert!(super::external("../clawd-svg").is_none());
    }
}
