use super::*;

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("mutis-demo-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(project_folder(path.to_str().unwrap()).unwrap())
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn lists_only_exact_names_outside_ignored_directories() {
    let project = Project::new();
    project.write("web/src/pages/__mutis-demo/one.tsx", b"123");
    project.write("web/src/pages/__mutis-demo/nested/two.tsx", b"45");
    project.write(".mutis-demo/one.html", b"6789");
    for ignored in IGNORED {
        project.write(&format!("{ignored}/__mutis-demo/file"), b"ignored");
    }
    project.write("mutis-demo/file", b"ignored");
    project.write("__mutis-demo-x/file", b"ignored");
    project.write("a/b/c/d/e/f/g/h/.mutis-demo/file", b"too deep");
    assert_eq!(
        serde_json::to_value(summarize(&project.0).unwrap()).unwrap(),
        serde_json::json!({
            "dirs": [
                {"path": ".mutis-demo", "files": 1, "bytes": 4},
                {"path": "web/src/pages/__mutis-demo", "files": 2, "bytes": 5}
            ],
            "files": 3,
            "bytes": 9
        })
    );
}

#[tokio::test]
async fn deletes_demo_directories_only() {
    let project = Project::new();
    project.write(".mutis-demo/demo.html", b"demo");
    project.write("web/src/pages/__mutis-demo/demo.tsx", b"demo");
    project.write("web/src/pages/index.tsx", b"keep");
    project.write("node_modules/__mutis-demo/file", b"keep");
    let result = delete_demos(project.0.clone()).await.unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::json!({"dirs": [], "files": 0, "bytes": 0, "skipped": []})
    );
    assert!(!project.0.join(".mutis-demo").exists());
    assert!(!project.0.join("web/src/pages/__mutis-demo").exists());
    assert_eq!(
        fs::read(project.0.join("web/src/pages/index.tsx")).unwrap(),
        b"keep"
    );
    assert!(project.0.join("node_modules/__mutis-demo/file").is_file());
}

#[tokio::test]
async fn skips_demo_directories_with_indexed_files() {
    use crate::core::process::git_output;

    let project = Project::new();
    project.write("web/src/pages/__mutis-demo/demo.tsx", b"tracked");
    project.write(".mutis-demo/demo.html", b"scratch");
    for args in [
        &["init", "--quiet"][..],
        &["add", "--force", "--", "web/src/pages/__mutis-demo/demo.tsx"][..],
    ] {
        assert!(git_output(&project.0, args).await.unwrap().status.success());
    }
    let result = delete_demos(project.0.clone()).await.unwrap();
    assert_eq!(result.skipped, ["web/src/pages/__mutis-demo"]);
    assert_eq!(result.remaining.files, 1);
    assert!(project.0.join("web/src/pages/__mutis-demo/demo.tsx").is_file());
    assert!(!project.0.join(".mutis-demo").exists());
}

#[cfg(unix)]
#[test]
fn does_not_follow_symlinks() {
    let project = Project::new();
    let other = Project::new();
    other.write("__mutis-demo/file", b"external");
    fs::create_dir(project.0.join(".mutis-demo")).unwrap();
    for (target, name) in [
        (other.0.clone(), "linked"),
        (other.0.join("__mutis-demo"), "__mutis-demo"),
        (other.0.join("__mutis-demo/file"), ".mutis-demo/file"),
    ] {
        std::os::unix::fs::symlink(target, project.0.join(name)).unwrap();
    }
    let result = summarize(&project.0).unwrap();
    assert_eq!(result.dirs.len(), 1);
    assert_eq!(result.files, 0);
    assert!(!remove_demo(&project.0, Path::new("__mutis-demo")).unwrap());
    assert!(other.0.join("__mutis-demo/file").is_file());
}
