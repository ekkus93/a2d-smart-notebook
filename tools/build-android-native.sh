#!/usr/bin/env bash
# Builds Android Rust libraries and regenerates the Kotlin UniFFI binding used by the Android build.
# The native libraries and Kotlin binding are generated artifacts and are intentionally untracked;
# Rust is the sole FFI source of truth. Run this before Gradle after any a2d-ffi API change.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

ABIS="${1:-${A2D_ANDROID_ABIS:-arm64-v8a x86_64}}"
bash ./tools/build-android-native-libs.sh "$ABIS"

first_abi=$(echo "$ABIS" | awk '{print $1}')
lib_path="apps/android/app/src/main/jniLibs/$first_abi/liba2d_ffi.so"

binding_root="apps/android/app/src/main/kotlin/uniffi"
rm -rf "$binding_root"
out_dir=$(mktemp -d "${TMPDIR:-/tmp}/a2d-uniffi-kt-regen.XXXXXX")
cleanup() {
  rm -rf "$out_dir"
}
trap cleanup EXIT

cargo run -p a2d-ffi --bin uniffi-bindgen -- generate \
  --library "$lib_path" \
  --language kotlin \
  --out-dir "$out_dir" \
  --no-format

generated_binding="$out_dir/uniffi/a2d_ffi/a2d_ffi.kt"
installed_binding="$binding_root/a2d_ffi/a2d_ffi.kt"
if [ ! -s "$generated_binding" ]; then
  echo "Kotlin UniFFI binding was not generated: $generated_binding" >&2
  exit 1
fi
install -D -m 0644 "$generated_binding" "$installed_binding"

printf 'Rebuilt native lib(s) for: %s\n' "$ABIS"
echo "Regenerated $installed_binding"
