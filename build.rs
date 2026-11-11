use std::{env, fs, path::Path};

fn main() {
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    let output = env::var("OUT_DIR").expect("output dir");
    generate(
        &Path::new(&manifest).join("ui"),
        &Path::new(&output).join("ui_assets.rs"),
        "UI_ASSETS",
    );
    generate(
        &Path::new(&manifest).join("web/out"),
        &Path::new(&output).join("web_assets.rs"),
        "WEB_ASSETS",
    );
    println!("cargo:rerun-if-changed=ui");
    println!("cargo:rerun-if-changed=web/out");
}

fn generate(root: &Path, out: &Path, table: &str) {
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    let mut generated = format!("static {table}: &[(&str, &[u8])] = &[\n");
    for file in files {
        let name = file
            .strip_prefix(root)
            .expect("asset under root")
            .to_string_lossy()
            .replace('\\', "/");
        let absolute = file
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        generated.push_str(&format!(
            "    (\"{name}\", include_bytes!(\"{absolute}\")),\n"
        ));
    }
    generated.push_str("];\n");
    fs::write(out, generated).expect("write asset table");
}

fn walk(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}
