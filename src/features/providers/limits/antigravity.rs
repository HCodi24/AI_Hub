use super::claude::iso_seconds;
use super::LimitWindow;
use crate::config::ProviderConfig;
use crate::providers::cli_support;

/// `agy /usage` prints one tab-separated line per window: `group`, `label`, `NN%` remaining, reset time (ISO, UTC).
pub(super) fn antigravity_windows(text: &str) -> Vec<LimitWindow> {
    text.lines()
        .filter_map(|line| {
            let mut cells = line.split('\t').map(str::trim);
            let (group, label, left, reset) = (cells.next()?, cells.next()?, cells.next()?, cells.next()?);
            let left = left.strip_suffix('%')?.parse::<f64>().ok()?;
            let group = group.split_whitespace().next().unwrap_or(group);
            Some(LimitWindow {
                label: format!("{group} · {}", label.trim_end_matches(" Limit Remaining")),
                used_percent: Some((100.0 - left).clamp(0.0, 100.0)),
                status: if left <= 0.0 { "rejected" } else { "allowed" }.to_owned(),
                resets_at: iso_seconds(reset),
            })
        })
        .collect()
}

pub(super) async fn read_antigravity(provider: &ProviderConfig) -> Option<Vec<LimitWindow>> {
    let home = crate::providers::antigravity_cli::clean_home(&provider.id).ok()?;
    let path = cli_support::resolve_executable("agy", Some(&provider.executable))?;
    let mut command = cli_support::command_for(&path, &["--print=/usage", "--output-format", "text"].map(Into::into));
    cli_support::agy_environment(&mut command, &home);
    command.stdin(std::process::Stdio::null()).kill_on_drop(true);
    let output = tokio::time::timeout(std::time::Duration::from_secs(25), command.output()).await.ok()?.ok()?;
    output.status.success().then(|| antigravity_windows(&String::from_utf8_lossy(&output.stdout)))
}
