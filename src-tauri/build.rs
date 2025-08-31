fn main() {
    // Copy GPU resources for ggml/whisper before tauri_build runs so that
    // tauri-build can validate listed resources exist.
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let resources_dir = manifest_dir.join("resources");
    let default_metallib_src = resources_dir.join("default.metallib");

    if default_metallib_src.exists() {
        // Ensure a copy at crate root so bundler can place it at Resources/ default.metallib
        let root_copy = manifest_dir.join("default.metallib");
        let _ = fs::copy(&default_metallib_src, &root_copy);

        // Also copy next to target binary for dev runs
        let dest_root = manifest_dir.join("target").join(&profile).join("default.metallib");
        let _ = fs::copy(&default_metallib_src, dest_root);
    }

    // Ensure Tauri build steps run (after preparing resources)
    tauri_build::build();

    // Make build script rerun if metallib changes
    println!("cargo:rerun-if-changed={}", resources_dir.join("default.metallib").display());
}
