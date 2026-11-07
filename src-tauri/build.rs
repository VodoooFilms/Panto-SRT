fn main() {
    // Tauri validates bundle resource paths at compile time. The engine
    // preparation scripts fill these directories later; create them here so a
    // plain `cargo check` works on a fresh clone too.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let root = std::path::Path::new(&manifest);
        if let Some(root) = root.parent() {
            let _ = std::fs::create_dir_all(root.join("build/whisper/lib"));
            let _ = std::fs::create_dir_all(root.join("build/nemo"));
        }
    }
    tauri_build::build()
}
