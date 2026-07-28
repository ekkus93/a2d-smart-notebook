//! One-use CI bridge for committing the checked-in Android binding after Milestone 9.1 changed the
//! UniFFI surface. The test is inert outside GitHub Actions. In CI it generates from the compiled
//! library, stages the exact generated Kotlin file, and stages deletion of itself plus the obsolete
//! workflow/trigger files. The existing validation workflow commits those staged changes only after
//! Rust, Android, and APK verification all pass.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("a2d-ffi must live under <repo>/crates")
        .to_path_buf()
}

fn build_cdylib() -> PathBuf {
    let status = Command::new(env!("CARGO"))
        .args(["build", "-p", "a2d-ffi", "--lib"])
        .status()
        .expect("cargo must be runnable");
    assert!(status.success(), "cargo build -p a2d-ffi --lib failed");

    let exe = std::env::current_exe().expect("test executable path must be available");
    let profile_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("test executable must be under target/<profile>/deps");
    let library_name = if cfg!(target_os = "macos") {
        "liba2d_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "a2d_ffi.dll"
    } else {
        "liba2d_ffi.so"
    };
    profile_dir.join(library_name)
}

fn git(repo: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(arguments)
        .status()
        .expect("git must be runnable in GitHub Actions");
    assert!(status.success(), "git command failed: git {arguments:?}");
}

#[test]
fn install_current_kotlin_binding_and_remove_one_use_machinery() {
    if std::env::var_os("GITHUB_ACTIONS").is_none() {
        return;
    }

    let repo = repo_root();
    let output = std::env::temp_dir().join(format!(
        "a2d-milestone9-binding-install-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&output).expect("binding output directory must be creatable");
    let library = build_cdylib();
    let status = Command::new(env!("CARGO_BIN_EXE_uniffi-bindgen"))
        .arg("generate")
        .arg("--library")
        .arg(&library)
        .arg("--language")
        .arg("kotlin")
        .arg("--out-dir")
        .arg(&output)
        .arg("--no-format")
        .status()
        .expect("uniffi-bindgen must be runnable");
    assert!(status.success(), "Kotlin binding generation failed");

    let generated = output.join("uniffi/a2d_ffi/a2d_ffi.kt");
    let destination = repo.join("apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt");
    assert!(generated.is_file(), "generated Kotlin binding is missing");
    std::fs::copy(&generated, &destination).expect("generated Kotlin binding must be installable");
    git(
        &repo,
        &[
            "add",
            "apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt",
        ],
    );

    for path in [
        "crates/a2d-ffi/tests/install_milestone9_binding_once.rs",
        ".github/workflows/validate-camerax-adapter.yml",
        ".github/tmp/trigger-milestone9-registration-bindings",
    ] {
        git(&repo, &["rm", "--force", path]);
    }
    std::fs::remove_dir_all(output).ok();
}
