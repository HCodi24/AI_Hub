use crate::chat::Permission;
use serde_json::{json, Value};

/// The app's three levels on agy's modes. agy's `plan` mode only asks in words, so Read is `accept-edits` with every file write denied
/// by rule (see `permission_rules`); commands still ask. Write allows edits; Full is `auto`.
pub(crate) fn mode(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "accept-edits",
        Permission::Write => "accept-edits",
        Permission::Full => "auto",
    }
}

/// agy's `--settings` permission rules for one turn: the app's CodeGraph is fine, `extra` are what the user just allowed, and Read denies
/// every file write (agy checks file edits, too, as `write_file`). No command is pre-allowed: a wildcard rule such as `command(dir *)` also
/// lets `dir . > a.txt` through, and agy already lets its own read-only tools and commands (list_dir, view_file, git status) pass.
pub(crate) fn permission_rules(permission: Permission, extra: &[String]) -> Value {
    let mut allow = vec!["mcp(codegraph/codegraph_explore)".to_owned()];
    allow.extend(extra.iter().cloned());
    let deny: &[&str] = if permission == Permission::Read { &["write_file(*)"] } else { &[] };
    json!({"permissions": {"allow": allow, "deny": deny}})
}
