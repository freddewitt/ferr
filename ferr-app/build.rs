use std::process::Command;
use std::path::PathBuf;

fn main() {
    // ── Rebuild ferr-cli sidecar before every Tauri build ───────────────────
    // This ensures ferr-app always ships with the latest CLI binary.
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("ferr-app must be inside the workspace")
        .to_path_buf();

    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "ferr-cli"])
        .current_dir(&workspace_root)
        .status()
        .expect("failed to invoke cargo to build ferr-cli");

    if !status.success() {
        panic!("ferr-cli build failed — aborting Tauri build");
    }

    // Detect target triple for the sidecar filename
    let target = std::env::var("TARGET")
        .unwrap_or_else(|_| "aarch64-apple-darwin".into());

    let src = workspace_root
        .join("target")
        .join("release")
        .join("ferr");

    let dest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("ferr-cli-{target}"));

    std::fs::create_dir_all(dest.parent().unwrap())
        .expect("failed to create binaries/ dir");

    std::fs::copy(&src, &dest)
        .unwrap_or_else(|e| panic!("failed to copy {src:?} → {dest:?}: {e}"));

    println!("cargo:warning=sidecar updated: {}", dest.display());

    // Tell cargo to re-run this script when ferr-cli source changes
    println!("cargo:rerun-if-changed=../ferr-cli/src");
    println!("cargo:rerun-if-changed=../ferr-core/src");
    println!("cargo:rerun-if-changed=../ferr-verify/src");

    tauri_build::build()
}
