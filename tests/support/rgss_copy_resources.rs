//! Synthetic bytes for project-generation/copy tests only. Never load or compile
//! these files as native libraries, and never include them in a release bundle.
use std::path::{Path, PathBuf};

pub fn create(destination: &Path, source: &Path) -> PathBuf {
    for rel in ["toolchain.lock", "schemas", "runtimes/manifests", "runtimes/themes", "compatibility-db", "android/shell"] {
        copy_tree(&source.join(rel), &destination.join(rel));
    }
    let jni = destination.join("runtimes/rgss/jni/arm64-v8a");
    std::fs::create_dir_all(&jni).unwrap();
    std::fs::write(jni.join("libmkxp-z.so"), b"RuntimePack TEST COPY SENTINEL - NOT A NATIVE LIBRARY\n").unwrap();
    destination.to_path_buf()
}

fn copy_tree(source: &Path, destination: &Path) {
    if source.is_dir() {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if ["build", ".gradle", ".kotlin", "jniLibs"].contains(&entry.file_name().to_string_lossy().as_ref()) { continue; }
            assert!(!entry.file_type().unwrap().is_symlink());
            copy_tree(&entry.path(), &destination.join(entry.file_name()));
        }
    } else {
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(source, destination).unwrap();
    }
}
