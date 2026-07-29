#!/usr/bin/env bash
# Generates disposable desktop Kotlin and Swift UniFFI bindings for inspection and smoke testing.
#
# These outputs are NOT the committed Android binding. The committed Android Kotlin file is
# regenerated from an Android native library by tools/build-android-native.sh and guarded by the
# permanent binding-drift CI job. See docs/BINDING_GENERATION.md.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

cargo build -p a2d-ffi --lib

case "$(uname -s)" in
  Darwin) LIB="target/debug/liba2d_ffi.dylib" ;;
  Linux)  LIB="target/debug/liba2d_ffi.so" ;;
  *)      LIB="target/debug/a2d_ffi.dll" ;;
esac

OUT_DIR="${1:-target/bindings}"
rm -rf "$OUT_DIR"

cargo run -p a2d-ffi --bin uniffi-bindgen -- generate --library "$LIB" --language kotlin --out-dir "$OUT_DIR/kotlin"
cargo run -p a2d-ffi --bin uniffi-bindgen -- generate --library "$LIB" --language swift --out-dir "$OUT_DIR/swift"

echo "Disposable desktop bindings generated under $OUT_DIR"
