use super::super::bad;
use crate::{api::ApiResult, features::addons};
use std::{path::PathBuf, process::Stdio, time::Duration};

/// Removes the downloaded repository when dropped, however the request ends.
pub(super) struct TempCheckout(pub(super) PathBuf);

impl TempCheckout {
    async fn git_output(&self, args: &[&str]) -> ApiResult<Vec<u8>> {
        // Clone without a worktree first, then materialize it without user/system filters or hooks.
        let mut git = checkout_git();
        git.current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.0.join(".git/mutis-empty-config"))
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .args(args);
        let output = crate::core::process::run_with_timeout(&mut git, Duration::from_secs(30))
            .await
            .map_err(|error| bad(format!("Không đọc được bản sao kho mã: {error}")))?;
        if !output.status.success() {
            return Err(bad("Không đọc được bản sao kho mã"));
        }
        Ok(output.stdout)
    }
}

impl Drop for TempCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn checkout_git() -> tokio::process::Command {
    let mut git = tokio::process::Command::new("git");
    git.env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    for name in [
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        git.env_remove(name);
    }
    crate::core::process::configure_hidden(&mut git);
    git
}

pub(super) struct Checkout {
    pub(super) source: String,
    pub(super) dir: TempCheckout,
    pub(super) found: addons::LinkFound,
    pub(super) tree: Vec<u8>,
}

/// Download and inspect a shallow GitHub checkout.
pub(super) async fn checkout(link: &str) -> ApiResult<Checkout> {
    let source = addons::github_source(link).map_err(|error| bad(error.to_string()))?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|error| bad(format!("Không tìm được thư mục tạm: {error}")))?;
    let path = temp.join(format!("mutis-addon-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&path)
        .map_err(|error| bad(format!("Không tạo được thư mục tạm: {error}")))?;
    let dir = TempCheckout(path);
    let mut git = checkout_git();
    git.args([
        "clone",
        "--depth",
        "1",
        "--no-checkout",
        "--template=",
        "--",
        &source,
    ])
    .arg(&dir.0);
    let output = crate::core::process::run_with_timeout(&mut git, Duration::from_secs(90))
        .await
        .map_err(|error| bad(format!("Không tải được repository: {error}")))?;
    if !output.status.success() {
        return Err(bad(
            "Không tải được repository (link sai, repository riêng tư hoặc máy không có git)",
        ));
    }
    dir.git_output(&["checkout", "--detach", "HEAD"]).await?;
    let tree = dir.git_output(&["ls-tree", "-r", "-z", "HEAD"]).await?;
    let found = addons::inspect_checkout(&dir.0, &source, &tree);
    Ok(Checkout {
        source,
        dir,
        found,
        tree,
    })
}
