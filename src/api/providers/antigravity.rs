//! Antigravity CLI source: it signs in through `agy` itself, so the connection test only asks `agy models`.
use super::connection::{connection, ConnectionTest};
use crate::{config::ProviderConfig, providers::cli_support};
use std::{process::Stdio, time::Duration};
use tokio::time::timeout;

/// Counts the models in `agy models` stdout (one `slug<TAB>name` line each).
fn model_count(stdout: &str) -> usize {
    stdout.lines().filter(|line| !line.trim().is_empty()).count()
}

// shortcut: "signed in" is inferred from `agy models` succeeding with a list; agy has no whoami/status command, so no email or plan is shown.
pub(super) async fn test_connection(provider: &ProviderConfig) -> ConnectionTest {
    let Some(path) = cli_support::resolve_executable("agy", Some(&provider.executable)) else {
        return connection(false, "error", "Chưa tìm thấy Antigravity CLI (agy) trên máy", "irm https://antigravity.google/cli/install.ps1 | iex");
    };
    let home = crate::providers::antigravity_cli::clean_home(&provider.id).ok();
    let run = |args: &[&str]| {
        let mut command = cli_support::command_for(&path, &args.iter().map(Into::into).collect::<Vec<_>>());
        if let Some(home) = &home {
            command.env("USERPROFILE", home).env("HOME", home);
        }
        command.stdin(Stdio::null()).kill_on_drop(true);
        async move { timeout(Duration::from_secs(25), command.output()).await }
    };
    match run(&["models"]).await {
        Err(_) => connection(false, "error", "Antigravity không phản hồi", "Hết thời gian chờ `agy models`"),
        Ok(Err(e)) => connection(false, "error", "Không chạy được agy", &e.to_string()),
        Ok(Ok(output)) => match model_count(&String::from_utf8_lossy(&output.stdout)) {
            count if output.status.success() && count > 0 => {
                // `/usage` prints the plan's remaining limits (one tab-separated line each); best effort, shown as the detail.
                let usage = match run(&["--print=/usage", "--output-format", "text"]).await {
                    Ok(Ok(out)) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_owned(),
                    _ => String::new(),
                };
                connection(true, "connected", &format!("Đã đăng nhập Antigravity · {count} model"), &usage)
            }
            _ => connection(false, "not_configured", "Antigravity chưa đăng nhập", "Chạy `agy` trong terminal và đăng nhập"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::model_count;

    #[test]
    fn counts_only_listed_models() {
        assert_eq!(model_count("gemini-4-neon-high\tGemini 4 Neon (High)\n\ngemini-4-carbon-low\tGemini 4 Carbon (Low)\n"), 2);
        assert_eq!(model_count("  \n"), 0);
    }
}
