use super::claude::claude_home;
use super::github::LinkPlugin;
use super::{
    copy_skill, github_source, inspect_checkout, install_checkout_skill, load_skill, overview,
    remove_shared_skill, remove_skill, safe_component, shared_skill_catalogue, shared_skills_dir,
    valid_name, SkillInstall,
};
use crate::config::ProviderConfig;
use std::{ffi::OsStr, fs, path::PathBuf};

fn skill_copy_fixture(label: &str) -> PathBuf {
    let dir = temp(label);
    for (home, content) in [("personal", "# New skill"), ("hub", "# Old skill")] {
        let skill = dir.join(home).join("skills/example");
        fs::create_dir_all(&skill).unwrap();
        fs::write(skill.join("SKILL.md"), content).unwrap();
    }
    dir
}

#[test]
fn copy_skill_replaces_an_existing_hub_copy() {
    let dir = skill_copy_fixture("copy-skill");
    let personal = dir.join("personal");
    let hub = dir.join("hub");
    let destination = hub.join("skills/example");
    fs::write(destination.join("obsolete.txt"), "old asset").unwrap();
    fs::write(personal.join("skills/example/asset.txt"), "new asset").unwrap();

    super::copy_skill(&personal, &hub, "example").unwrap();

    assert_eq!(
        fs::read_to_string(destination.join("SKILL.md")).unwrap(),
        "# New skill"
    );
    assert_eq!(
        fs::read_to_string(destination.join("asset.txt")).unwrap(),
        "new asset"
    );
    assert!(!destination.join("obsolete.txt").exists());
    assert_eq!(fs::read_dir(hub.join("skills")).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(unix)]
#[test]
fn copy_skill_rejects_unsafe_file_names_without_replacing_the_old_copy() {
    let dir = skill_copy_fixture("copy-unsafe-skill");
    let personal = dir.join("personal");
    let hub = dir.join("hub");
    fs::write(personal.join("skills/example/CON"), "unsafe asset").unwrap();

    assert_failed_skill_copy_preserves_hub(&personal, &hub);
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(any(unix, windows))]
#[test]
fn copy_skill_rejects_symlinks_without_replacing_the_old_copy() {
    let dir = skill_copy_fixture("copy-linked-skill");
    let personal = dir.join("personal");
    let hub = dir.join("hub");
    let skill = personal.join("skills/example");
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    #[cfg(windows)]
    use std::os::windows::fs::symlink_file as symlink;
    if symlink(skill.join("SKILL.md"), skill.join("link.md")).is_ok() {
        assert_failed_skill_copy_preserves_hub(&personal, &hub);
    }
    fs::remove_dir_all(dir).unwrap();
}

#[cfg(any(unix, windows))]
fn assert_failed_skill_copy_preserves_hub(personal: &std::path::Path, hub: &std::path::Path) {
    assert!(super::copy_skill(personal, hub, "example").is_err());
    assert_eq!(
        fs::read_to_string(hub.join("skills/example/SKILL.md")).unwrap(),
        "# Old skill"
    );
    assert_eq!(fs::read_dir(hub.join("skills")).unwrap().count(), 1);
}

#[test]
fn copy_and_remove_skill_reject_uppercase_synced() {
    let dir = temp("reserved-skill");
    for error in [
        super::copy_skill(&dir, &dir, "SYNCED").unwrap_err(),
        super::remove_skill(&dir, "SYNCED").unwrap_err(),
    ] {
        assert_eq!(error.to_string(), "Tên skill không hợp lệ");
    }
    fs::remove_dir_all(dir).unwrap();
}
#[test]
fn links_are_limited_to_plain_github_repositories() {
    for good in [
        "owner/repo",
        "https://github.com/owner/repo",
        "https://github.com/o-w.n/re_po.git/",
    ] {
        assert!(github_source(good)
            .unwrap()
            .starts_with("https://github.com/"));
    }
    for bad in [
        "",
        "owner",
        "a/b/c",
        "-x/repo",
        "../repo",
        "http://github.com/o/r",
        "https://evil.com/o/r",
        "o/r --upload-pack=x",
        "file:///c/x",
    ] {
        assert!(github_source(bad).is_err(), "{bad}");
    }
}

#[test]
fn checkout_shows_the_marketplaces_each_cli_can_install() {
    let dir = temp("link");
    std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
    std::fs::write(
        dir.join(".claude-plugin/marketplace.json"),
        r#"{"name":"mk","plugins":[{"name":"a","description":"A"},{"name":"../bad"}]}"#,
    )
    .unwrap();
    let found = inspect_checkout(&dir, "https://github.com/owner/repo", &[]);
    assert_eq!(
        found.claude.unwrap().plugins,
        vec![LinkPlugin {
            name: "a".into(),
            description: "A".into()
        }]
    );
    assert!(found.codex.is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn checkout_detects_mcp_from_readme_or_manifest() {
    let dir = temp("link-mcp");
    let source = "https://github.com/codegraph-ai/CodeGraph";
    let readme = dir.join("README.md");
    let tree = git_tree(&[("100644", "blob", "README.md")]);
    std::fs::write(&readme, "Use an MCP server with this client.").unwrap();
    assert!(!inspect_checkout(&dir, source, &tree).mcp);
    std::fs::write(
        &readme,
        "### MCP Server\n{\"mcpServers\":{\"codegraph\":{\"command\":\"codegraph-server\"}}}",
    )
    .unwrap();
    assert!(inspect_checkout(&dir, source, &tree).mcp);
    let symlink = git_tree(&[("120000", "blob", "README.md")]);
    assert!(!inspect_checkout(&dir, source, &symlink).mcp);

    let tree = git_tree(&[("100644", "blob", ".mcp.json")]);
    std::fs::write(
        dir.join(".mcp.json"),
        r#"{"mcpServers":{"example":{"url":"https://example.com/mcp"}}}"#,
    )
    .unwrap();
    assert!(inspect_checkout(&dir, source, &tree).mcp);
    std::fs::remove_dir_all(dir).unwrap();
}

fn git_tree(entries: &[(&str, &str, &str)]) -> Vec<u8> {
    entries
        .iter()
        .map(|(mode, oid, path)| format!("{mode} blob {oid}\t{path}\0"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn checkout_lists_root_and_nested_skills_alongside_marketplaces() {
    let dir = temp("mixed");
    let source = "https://github.com/heydamir/unslop";
    let markers = [
        "skills/zeta/SKILL.md",
        "skills/unslop/SKILL.md",
        "SKILL.md",
        "skills/nested/ignored/SKILL.md",
        "skills/synced/SKILL.md",
        "skills/link/SKILL.md",
    ];
    for marker in markers {
        let file = dir.join(marker);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, "# Skill").unwrap();
    }
    let mut records: Vec<_> = markers[..5]
        .iter()
        .map(|&path| ("100644", "a", path))
        .collect();
    records.push(("120000", "b", markers[5]));
    let tree = git_tree(&records);
    let standalone = inspect_checkout(&dir, source, &tree);
    assert!(standalone.claude.is_none() && standalone.codex.is_none());
    assert_eq!(
        standalone
            .skills
            .iter()
            .map(|skill| (skill.name.as_str(), skill.path.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("unslop", "SKILL.md"),
            ("unslop", "skills/unslop/SKILL.md"),
            ("zeta", "skills/zeta/SKILL.md")
        ],
    );
    assert_ne!(standalone.skills[0].id, standalone.skills[1].id);
    let other = inspect_checkout(&dir, "https://github.com/another/unslop", &tree);
    assert_ne!(other.skills[1].id, standalone.skills[1].id);
    for market in [".claude-plugin", ".agents/plugins"] {
        fs::create_dir_all(dir.join(market)).unwrap();
        fs::write(
            dir.join(market).join("marketplace.json"),
            r#"{"name":"heydamir","plugins":[{"name":"unslop","source":"./"}]}"#,
        )
        .unwrap();
    }
    let mixed = inspect_checkout(&dir, source, &tree);
    assert_eq!(mixed.skills, standalone.skills);
    assert_eq!(mixed.claude.unwrap().plugins[0].name, "unslop");
    assert_eq!(mixed.codex.unwrap().plugins[0].name, "unslop");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn standalone_skill_copies_assets_and_requires_confirmation_for_the_current_destination() {
    let dir = temp("install");
    let config = dir.join("config.json");
    let checkout = dir.join("checkout");
    let from = checkout.join("skills/unslop");
    fs::create_dir_all(from.join("references")).unwrap();
    fs::create_dir(from.join(".git")).unwrap();
    fs::write(from.join("SKILL.md"), "# Bản mới").unwrap();
    fs::write(from.join("references/Ảnh.bin"), [0, 128, 255]).unwrap();
    let tree = git_tree(&[
        ("100644", "a", "skills/unslop/SKILL.md"),
        ("100644", "b", "skills/unslop/references/Ảnh.bin"),
    ]);
    let found = inspect_checkout(&checkout, "https://github.com/owner/repo", &tree);
    let skill = &found.skills[0];
    let shared = shared_skills_dir(&config).unwrap();
    let other_config = dir.join("other/config.json");
    let other_shared = shared_skills_dir(&other_config).unwrap();
    let claude = dir.join("claude-config");
    let legacy = claude.join("skills/unslop/SKILL.md");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(&legacy, "# Bản cũ của Claude").unwrap();
    assert!(matches!(
        install_checkout_skill(&checkout, &tree, skill, &config, None).unwrap(),
        SkillInstall::Installed
    ));
    assert_eq!(
        fs::read(shared.join("unslop/references/Ảnh.bin")).unwrap(),
        [0, 128, 255]
    );
    assert!(!shared.join("unslop/.git").exists());
    assert!(shared_skill_catalogue(&config).contains("unslop"));
    assert_eq!(overview(&claude, &shared).shared_skills, ["unslop"]);

    fs::create_dir_all(other_shared.join("unslop")).unwrap();
    let current = other_shared.join("unslop/SKILL.md");
    fs::write(&current, "# Bản cũ").unwrap();
    let SkillInstall::Conflict(token) =
        install_checkout_skill(&checkout, &tree, skill, &config, None).unwrap()
    else {
        panic!("expected conflict");
    };
    let SkillInstall::Conflict(other_token) =
        install_checkout_skill(&checkout, &tree, skill, &other_config, Some(&token)).unwrap()
    else {
        panic!("a confirmation from another destination must not be accepted");
    };
    assert_ne!(other_token, token);
    assert_eq!(fs::read_to_string(&current).unwrap(), "# Bản cũ");
    assert_eq!(fs::read_dir(&other_shared).unwrap().count(), 1);
    assert!(matches!(
        install_checkout_skill(&checkout, &tree, skill, &other_config, Some(&other_token)).unwrap(),
        SkillInstall::Installed
    ));
    assert_eq!(fs::read_to_string(&current).unwrap(), "# Bản mới");
    assert!(other_shared.join("unslop/references/Ảnh.bin").is_file());

    remove_shared_skill(&config, "unslop").unwrap();
    assert!(overview(&claude, &shared).shared_skills.is_empty());
    assert_eq!(fs::read_to_string(legacy).unwrap(), "# Bản cũ của Claude");
    assert!(current.is_file());
    assert!(remove_shared_skill(&config, "unslop").is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejected_git_links_and_failed_staging_leave_the_installed_skill_intact() {
    let dir = temp("unsafe");
    let config = dir.join("config.json");
    let checkout = dir.join("checkout");
    fs::create_dir(&checkout).unwrap();
    fs::write(checkout.join("SKILL.md"), "# Bản mới").unwrap();
    fs::write(checkout.join("reference.txt"), "asset").unwrap();
    let tree = git_tree(&[
        ("100644", "a", "SKILL.md"),
        ("100644", "b", "reference.txt"),
    ]);
    let found = inspect_checkout(&checkout, "https://github.com/owner/repo", &tree);
    let skill = &found.skills[0];
    let skills = dir.join("skills");
    let invalid = git_tree(&[
        ("100644", "a", "SKILL.md"),
        ("120000", "b", "reference.txt"),
    ]);
    assert!(install_checkout_skill(&checkout, &invalid, skill, &config, None).is_err());
    let unsafe_path = git_tree(&[
        ("100644", "a", "SKILL.md"),
        ("100644", "b", "assets/CON.txt"),
    ]);
    assert!(install_checkout_skill(&checkout, &unsafe_path, skill, &config, None).is_err());
    assert!(!skills.exists());
    for path in [
        "..",
        "a:b",
        "a\\b",
        "a/b",
        "NUL.txt",
        "com¹.zip",
        "name.",
        "name ",
    ] {
        assert!(!safe_component(OsStr::new(path)), "{path}");
    }

    fs::create_dir_all(skills.join("repo")).unwrap();
    let current = skills.join("repo/SKILL.md");
    fs::write(&current, "# Bản cũ").unwrap();
    let SkillInstall::Conflict(token) =
        install_checkout_skill(&checkout, &tree, skill, &config, None).unwrap()
    else {
        panic!("expected conflict");
    };
    fs::remove_file(checkout.join("SKILL.md")).unwrap();
    assert!(install_checkout_skill(&checkout, &tree, skill, &config, Some(&token)).is_err());
    assert_eq!(fs::read_to_string(current).unwrap(), "# Bản cũ");
    assert_eq!(fs::read_dir(skills).unwrap().count(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn legacy_codex_skills_use_the_same_home_as_app_server() {
    let dir = temp("codex-home");
    let config = dir.join("config.json");
    let provider = ProviderConfig {
        id: "first-codex".into(),
        ..Default::default()
    };
    let skills = crate::providers::codex_cli::legacy_skills_dir(&config, &provider.id).unwrap();
    assert!(!skills.parent().unwrap().exists());
    let home = crate::providers::codex::home(&config, &provider).unwrap();
    assert_eq!(skills, home.join("skills"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn shared_skill_wins_and_legacy_lookup_only_reads_configured_homes() {
    let dir = temp("lookup");
    let config = dir.join("config.json");
    let providers = ["first-codex", "second-codex"].map(|id| ProviderConfig {
        id: id.into(),
        kind: "codex_cli".into(),
        ..Default::default()
    });
    let shared = dir.join("skills");
    let claude = dir.join("claude-config/skills");
    let codex = dir.join("codex/second-codex/skills");
    let unconfigured = dir.join("codex/other/skills");
    let write = |root: &std::path::Path, content: &str| {
        let directory = root.join("review");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("SKILL.md"), content).unwrap();
    };
    write(&unconfigured, "# Nguồn chưa cấu hình");
    assert!(load_skill(&config, &providers, "review").unwrap().is_none());
    assert!(!shared.exists());
    assert!(!dir.join("codex/first-codex").exists());

    write(&codex, "# Bản cũ của Codex");
    let skill = load_skill(&config, &providers, "review").unwrap().unwrap();
    assert_eq!(skill.content, "# Bản cũ của Codex");
    assert!(skill
        .directory
        .ends_with("codex/second-codex/skills/review"));
    assert!(shared_skill_catalogue(&config).is_empty());

    write(&claude, "# Bản cũ của Claude");
    let skill = load_skill(&config, &[], "review").unwrap().unwrap();
    assert_eq!(skill.content, "# Bản cũ của Claude");
    let Err(error) = load_skill(&config, &providers, "review") else {
        panic!("expected ambiguous legacy skill");
    };
    assert!(error
        .to_string()
        .contains("Hãy cài bản bạn muốn dùng vào kho chung"));

    write(&shared, "# Bản dùng chung");
    let skill = load_skill(&config, &providers, "review").unwrap().unwrap();
    assert_eq!(skill.content, "# Bản dùng chung");
    assert_eq!(
        fs::read_to_string(claude.join("review/SKILL.md")).unwrap(),
        "# Bản cũ của Claude"
    );
    assert_eq!(
        fs::read_to_string(codex.join("review/SKILL.md")).unwrap(),
        "# Bản cũ của Codex"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn skill_lookup_rejects_invalid_names_non_files_and_oversized_markdown() {
    let dir = temp("invalid-lookup");
    let config = dir.join("config.json");
    let file = dir.join("skills/review/SKILL.md");
    assert!(load_skill(&config, &[], "../review").unwrap().is_none());
    assert!(remove_shared_skill(&config, "../review").is_err());
    fs::create_dir_all(&file).unwrap();
    assert!(load_skill(&config, &[], "review").is_err());
    fs::remove_dir(&file).unwrap();
    fs::write(&file, vec![b'x'; 64 * 1024 + 1]).unwrap();
    let Err(error) = load_skill(&config, &[], "review") else {
        panic!("expected size limit");
    };
    assert!(error.to_string().contains("64 KiB"));
    fs::remove_dir_all(dir).unwrap();
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ai-hub-addons-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn names_are_restricted_to_safe_characters() {
    for ok in [
        "ponytail@ponytail",
        "ui-ux-pro-max@ui-ux-pro-max-skill",
        "code_graph",
        "a.b",
    ] {
        assert!(valid_name(ok), "{ok}");
    }
    for bad in [
        "", "../x", "a b", "-flag", ".hidden", "a/b", "a\\b", "x;y", "a\"b",
    ] {
        assert!(!valid_name(bad), "{bad}");
    }
}

#[test]
fn claude_home_lists_plugins_skills_and_mcp_without_secrets() {
    let dir = temp("claude");
    std::fs::create_dir_all(dir.join("plugins")).unwrap();
    std::fs::create_dir_all(dir.join("skills/graphify")).unwrap();
    std::fs::create_dir_all(dir.join("skills/synced/x")).unwrap();
    std::fs::create_dir_all(dir.join("skills/not-a-skill")).unwrap();
    std::fs::write(dir.join("skills/graphify/SKILL.md"), "# Tóm tắt đồ thị").unwrap();
    std::fs::write(
        dir.join("plugins/installed_plugins.json"),
        r#"{"plugins":{"ponytail@ponytail":[{"version":"5.3.3"}],"old@market":[{"version":"1"}]}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("plugins/known_marketplaces.json"),
        r#"{"ponytail":{"source":{"source":"github","repo":"DietrichGebert/ponytail"}}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("settings.json"),
        r#"{"enabledPlugins":{"old@market":false}}"#,
    )
    .unwrap();
    let state = dir.join(".claude.json");
    std::fs::write(
        &state,
        r#"{"mcpServers":{"codegraph":{"command":"codegraph","args":["serve","--mcp"],"env":{"KEY":"secret"}},
           "web":{"type":"http","url":"https://example.test/mcp","headers":{"Authorization":"Bearer secret"}}}}"#,
    )
    .unwrap();
    let home = claude_home(&dir, &state);
    assert_eq!(home.skills, vec!["graphify"]);
    assert_eq!(home.plugins.len(), 2);
    assert!(!home.plugins[0].enabled && home.plugins[0].id == "old@market");
    assert_eq!(
        home.plugins[1].marketplace_source,
        "DietrichGebert/ponytail"
    );
    assert_eq!(home.plugins[1].version, "5.3.3");
    assert_eq!(home.mcp[0].summary, "codegraph serve --mcp");
    assert_eq!(home.mcp[1].summary, "https://example.test/mcp");
    assert!(!serde_json::to_string(&home).unwrap().contains("secret"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn skills_are_copied_replaced_and_removed() {
    let personal = temp("personal");
    let hub = temp("hub");
    std::fs::create_dir_all(personal.join("skills/graphify/scripts")).unwrap();
    std::fs::write(personal.join("skills/graphify/SKILL.md"), "bản một").unwrap();
    std::fs::write(personal.join("skills/graphify/scripts/run.py"), "print(1)").unwrap();
    copy_skill(&personal, &hub, "graphify").unwrap();
    assert!(hub.join("skills/graphify/scripts/run.py").is_file());
    std::fs::write(personal.join("skills/graphify/SKILL.md"), "bản hai").unwrap();
    std::fs::remove_file(personal.join("skills/graphify/scripts/run.py")).unwrap();
    copy_skill(&personal, &hub, "graphify").unwrap();
    assert_eq!(
        std::fs::read_to_string(hub.join("skills/graphify/SKILL.md")).unwrap(),
        "bản hai"
    );
    assert!(!hub.join("skills/graphify/scripts/run.py").exists());
    assert!(copy_skill(&personal, &hub, "missing").is_err());
    assert!(copy_skill(&personal, &hub, "../x").is_err());
    remove_skill(&hub, "graphify").unwrap();
    assert!(!hub.join("skills/graphify").exists());
    assert!(remove_skill(&hub, "graphify").is_err());
    for dir in [personal, hub] {
        let _ = std::fs::remove_dir_all(dir);
    }
}
