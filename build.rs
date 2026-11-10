use std::{env, fs, path::Path};

fn main() {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("manifest dir")).join("ui");
    let out = Path::new(&env::var("OUT_DIR").expect("output dir")).join("ui_assets.rs");
    let mut files = Vec::new();
    walk(&root, &mut files);
    files.sort();

    let mut generated = String::from("static UI_ASSETS: &[(&str, &[u8])] = &[\n");
    for file in files {
        let name = file
            .strip_prefix(&root)
            .expect("asset under ui")
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
    fs::write(out, generated).expect("write UI asset table");
    println!("cargo:rerun-if-changed=ui");
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
