use super::{verify::folder::project_folder, ApiError, ApiResult};
use axum::{extract::Query, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, Metadata},
    io,
    path::{Path, PathBuf},
};

const MAX_DEPTH: usize = 8;
const IGNORED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "vendor",
];

#[derive(Deserialize)]
pub(super) struct DemoQuery {
    folder: String,
}

#[derive(Serialize)]
pub(super) struct DemoDir {
    path: String,
    files: u64,
    bytes: u64,
}

#[derive(Default, Serialize)]
pub(super) struct DemoList {
    dirs: Vec<DemoDir>,
    files: u64,
    bytes: u64,
}

#[derive(Serialize)]
pub(super) struct DemoDelete {
    #[serde(flatten)]
    remaining: DemoList,
    skipped: Vec<String>,
}

fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & REPARSE_POINT != 0 {
            return true;
        }
    }
    metadata.is_symlink()
}

fn relative_name(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn discover(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    let mut pending = vec![(PathBuf::new(), 1)];
    while let Some((relative, depth)) = pending.pop() {
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            if !metadata.is_dir() || is_link(&metadata) {
                continue;
            }
            let name = entry.file_name();
            let path = relative.join(&name);
            match name.to_str() {
                Some("__mutis-demo" | ".mutis-demo") => dirs.push(path),
                Some(name) if IGNORED.contains(&name) => {}
                _ if depth < MAX_DEPTH => pending.push((path, depth + 1)),
                _ => {}
            }
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn summarize(root: &Path) -> io::Result<DemoList> {
    let mut result = DemoList::default();
    for path in discover(root)? {
        let mut demo = DemoDir {
            path: relative_name(&path),
            files: 0,
            bytes: 0,
        };
        let mut pending = vec![root.join(path)];
        while let Some(dir) = pending.pop() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let metadata = fs::symlink_metadata(entry.path())?;
                if is_link(&metadata) {
                    continue;
                }
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else if metadata.is_file() {
                    demo.files += 1;
                    demo.bytes += metadata.len();
                }
            }
        }
        result.files += demo.files;
        result.bytes += demo.bytes;
        result.dirs.push(demo);
    }
    Ok(result)
}

async fn blocking<T: Send + 'static>(
    action: impl FnOnce() -> io::Result<T> + Send + 'static,
) -> ApiResult<T> {
    tokio::task::spawn_blocking(action)
        .await
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Tác vụ demo bị gián đoạn",
            )
        })?
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không thể xử lý thư mục demo",
            )
        })
}

pub(super) async fn list(Query(query): Query<DemoQuery>) -> ApiResult<Json<DemoList>> {
    let root = blocking(move || Ok(project_folder(&query.folder))).await??;
    blocking(move || summarize(&root)).await.map(Json)
}

fn remove_demo(root: &Path, relative: &Path) -> io::Result<bool> {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Ok(false);
    }
    let canonical = path.canonicalize().map(crate::core::text::strip_verbatim)?;
    if canonical != path || !canonical.starts_with(root) {
        return Ok(false);
    }
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

async fn delete_demos(root: PathBuf) -> ApiResult<DemoDelete> {
    use crate::core::process::git_output;

    let scan_root = root.clone();
    let (dirs, has_git_marker) = blocking(move || {
        let mut has_git_marker = false;
        for ancestor in scan_root.ancestors() {
            match fs::symlink_metadata(ancestor.join(".git")) {
                Ok(_) => {
                    has_git_marker = true;
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok((discover(&scan_root)?, has_git_marker))
    })
    .await?;
    let in_git = !dirs.is_empty()
        && (has_git_marker
            || git_output(&root, &["rev-parse", "--is-inside-work-tree"])
                .await
                .is_ok_and(|output| output.status.success()));

    let mut skipped = Vec::new();
    for dir in dirs {
        let name = relative_name(&dir);
        if in_git {
            let pathspec = format!(":(literal){name}");
            let untracked = dir.to_str().is_some()
                && git_output(&root, &["ls-files", "-z", "--", &pathspec])
                    .await
                    .is_ok_and(|output| output.status.success() && output.stdout.is_empty());
            if !untracked {
                skipped.push(name);
                continue;
            }
        }
        let delete_root = root.clone();
        if !blocking(move || remove_demo(&delete_root, &dir)).await? {
            skipped.push(name);
        }
    }
    let remaining = blocking(move || summarize(&root)).await?;
    Ok(DemoDelete { remaining, skipped })
}

pub(super) async fn delete(Query(query): Query<DemoQuery>) -> ApiResult<Json<DemoDelete>> {
    let root = blocking(move || Ok(project_folder(&query.folder))).await??;
    delete_demos(root).await.map(Json)
}

#[cfg(test)]
mod tests;
