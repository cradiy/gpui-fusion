use std::{env, fs, path::Path};

fn collect(root: &Path, directory: &Path, entries: &mut Vec<String>) {
    let mut paths: Vec<_> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    for path in paths {
        assert!(!path.is_symlink(), "bundled templates cannot be symlinks");
        if path.is_dir() {
            collect(root, &path, entries);
        } else {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            entries.push(format!(
                "({name:?}, include_bytes!({:?})),",
                path.to_str().unwrap()
            ));
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=templates/android");
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("templates/android");
    let mut entries = Vec::new();
    collect(&root, &root, &mut entries);
    let source = format!(
        "pub const ANDROID_FILES: &[(&str, &[u8])] = &[{}];",
        entries.join("\n")
    );
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("android_files.rs"),
        source,
    )
    .unwrap();
}
