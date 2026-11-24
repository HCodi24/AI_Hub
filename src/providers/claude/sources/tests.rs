use super::*;

#[test]
fn project_switches_are_independent_and_bounded() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/claude-sources-test")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join(".claude/skills/demo")).unwrap();
    std::fs::write(root.join("CLAUDE.md"), "INSTRUCTION_MARKER").unwrap();
    std::fs::write(root.join(".claude/skills/demo/SKILL.md"), "SKILL_MARKER").unwrap();
    let mut s = ClaudeConfig::default();
    assert_eq!(project_context(&root, &s).unwrap(), "");
    s.project_skills = true;
    let text = project_context(&root, &s).unwrap();
    assert!(text.contains("SKILL_MARKER") && !text.contains("INSTRUCTION_MARKER"));
    s.project_skills = false;
    s.project_instructions = true;
    let text = project_context(&root, &s).unwrap();
    assert!(!text.contains("SKILL_MARKER") && text.contains("INSTRUCTION_MARKER"));
    std::fs::write(root.join("CLAUDE.md"), "x".repeat(32_001)).unwrap();
    assert!(project_context(&root, &s).is_err());
}
