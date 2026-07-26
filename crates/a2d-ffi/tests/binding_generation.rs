//! Binding-generation drift test (TODO 2.4). Not a golden-file diff against checked-in bindings
//! — codegen formatting/version changes would make that brittle, and generated bindings aren't
//! committed (they're build output; see `tools/generate-bindings.sh`). Instead this proves, on
//! every test run, that the current FFI interface can still be turned into Kotlin and Swift
//! bindings exposing the expected public API. If this stops passing, the interface has drifted
//! into something UniFFI can no longer bind.

use std::path::{Path, PathBuf};
use std::process::Command;

fn cdylib_path() -> PathBuf {
    let exe = std::env::current_exe().expect("test executable path must be available");
    let deps_dir = exe.parent().expect("deps dir");
    let profile_dir = deps_dir.parent().expect("profile dir");
    let name = if cfg!(target_os = "macos") {
        "liba2d_ffi.dylib"
    } else if cfg!(target_os = "windows") {
        "a2d_ffi.dll"
    } else {
        "liba2d_ffi.so"
    };
    profile_dir.join(name)
}

fn generate(language: &str, out_dir: &Path) {
    let library = cdylib_path();
    assert!(
        library.exists(),
        "expected cdylib at {library:?} — run `cargo build -p a2d-ffi --lib` first \
         (the crate's [lib] crate-type includes cdylib)"
    );
    let status = Command::new(env!("CARGO_BIN_EXE_uniffi-bindgen"))
        .arg("generate")
        .arg("--library")
        .arg(&library)
        .arg("--language")
        .arg(language)
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--no-format")
        .status()
        .expect("uniffi-bindgen must be runnable");
    assert!(
        status.success(),
        "uniffi-bindgen generate --language {language} failed"
    );
}

fn read_all_generated_text(dir: &Path) -> String {
    let mut combined = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(&current).expect("out dir must be readable") {
            let entry = entry.expect("dir entry must be readable");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                combined.push_str(&text);
                combined.push('\n');
            }
        }
    }
    combined
}

fn temp_out_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "a2d-ffi-bindgen-test-{label}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("must be able to create a temp out dir");
    dir
}

const EXPECTED_API_SYMBOLS: [&str; 4] = [
    "A2dClient",
    "OpenLibraryRequest",
    "generatePageId",
    "parsePageId",
];

#[test]
fn kotlin_bindings_generate_and_expose_the_expected_api() {
    let out = temp_out_dir("kotlin");
    generate("kotlin", &out);
    let contents = read_all_generated_text(&out);
    for symbol in EXPECTED_API_SYMBOLS {
        assert!(
            contents.contains(symbol),
            "generated Kotlin bindings are missing `{symbol}`"
        );
    }
    std::fs::remove_dir_all(&out).ok();
}

#[test]
fn swift_bindings_generate_and_expose_the_expected_api() {
    let out = temp_out_dir("swift");
    generate("swift", &out);
    let contents = read_all_generated_text(&out);
    for symbol in EXPECTED_API_SYMBOLS {
        assert!(
            contents.contains(symbol),
            "generated Swift bindings are missing `{symbol}`"
        );
    }
    std::fs::remove_dir_all(&out).ok();
}
