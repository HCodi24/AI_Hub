use super::valid_path_entry;
use std::{env, path::PathBuf, process::Command as StdCommand};

pub(super) fn registry_path_directories() -> Vec<PathBuf> {
    [
        r"HKCU\Environment",
        r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    ]
    .into_iter()
    .filter_map(read_registry_path)
    .flat_map(|path| env::split_paths(std::ffi::OsStr::new(&path)).collect::<Vec<_>>())
    .filter(|path| valid_path_entry(path))
    .collect()
}

fn read_registry_path(key: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = StdCommand::new("reg")
        .args(["query", key, "/v", "Path"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let name = fields.next()?;
        let value_type = fields.next()?;
        if !name.eq_ignore_ascii_case("Path") || !matches!(value_type, "REG_SZ" | "REG_EXPAND_SZ") {
            return None;
        }
        let value_start = line.find(value_type)? + value_type.len();
        let value = line[value_start..].trim();
        Some(expand_windows_variables(value))
    })
}

fn expand_windows_variables(value: &str) -> String {
    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };
        let variable = &after_start[..end];
        if let Some(value) = env::var_os(variable) {
            expanded.push_str(&value.to_string_lossy());
        } else {
            expanded.push('%');
            expanded.push_str(variable);
            expanded.push('%');
        }
        rest = &after_start[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}
