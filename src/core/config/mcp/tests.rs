use super::*;

fn server(name: &str, command: &str) -> McpServerConfig {
    McpServerConfig { name: name.into(), command: command.into(), enabled: true, ..Default::default() }
}

#[test]
fn accepts_a_normal_list_and_rejects_bad_names_duplicates_and_empty_commands() {
    assert!(validate_mcp_servers(&[server("codegraph", "codegraph"), server("my_tool-2", "x")]).is_ok());
    assert!(validate_mcp_servers(&[server("Bad Name", "x")]).is_err());
    assert!(validate_mcp_servers(&[server("a", "x"), server("a", "y")]).is_err());
    assert!(validate_mcp_servers(&[server("a", "  ")]).is_err());
}

#[test]
fn missing_enabled_defaults_to_on() {
    let parsed: McpServerConfig = serde_json::from_str(r#"{"name":"a","command":"b"}"#).unwrap();
    assert!(parsed.enabled);
}
