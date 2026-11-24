use super::edit::{exact_replace, match_line_endings};
use super::*;

#[test]
fn crlf_files_accept_lf_edits_and_stay_crlf() {
    let file = "a\nb\nc\n";
    let (old, new) = (match_line_endings(file, "a\nb"), match_line_endings(file, "a\nx"));
    assert_eq!(exact_replace(file, &old, &new).unwrap(), "a\nx\nc\n");
    assert_eq!(match_line_endings("a\nb\n", "x\ny"), "x\ny");
}

#[test]
fn path_is_confined_and_blocks_generated_directories() {
    let root = std::env::current_dir().unwrap();
    for path in [
        "../escape",
        "C:\\temp\\x",
        ".git/config",
        "target/out",
        "node_modules/x",
        ".codegraph/index",
    ] {
        assert!(safe_path(&root, path).is_err(), "{path}");
    }
}

#[test]
fn edit_requires_exactly_one_match_and_diff_counts_changes() {
    assert_eq!(
        exact_replace("a b a", "a", "x").unwrap_err().to_string(),
        "old_text khớp 2 lần; cần đúng 1 lần"
    );
    assert_eq!(exact_replace("a b", "a", "x").unwrap(), "x b");
    assert_eq!(line_counts("a\nb\n", "a\nc\n"), (1, 1));
}

#[test]
fn tool_names_are_valid_for_openai_and_anthropic() {
    for write in [false, true] {
        for name in anthropic_definitions(write)
            .iter()
            .filter_map(|tool| tool["name"].as_str())
        {
            assert!(valid_tool_name(name), "{name}");
        }
        for tool in openai_definitions(write) {
            assert!(valid_tool_name(tool["function"]["name"].as_str().unwrap()));
        }
    }
}

fn valid_tool_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
        })
}
