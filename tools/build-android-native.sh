#!/usr/bin/env bash
# Builds Android Rust libraries and regenerates the committed Kotlin UniFFI bindings.
# Run this after any a2d-ffi API change. The native libraries remain generated artifacts under
# app/src/main/jniLibs and are not committed; the Kotlin binding is committed and checked for drift.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

ABIS="${1:-${A2D_ANDROID_ABIS:-arm64-v8a x86_64}}"
./tools/build-android-native-libs.sh "$ABIS"

first_abi=$(echo "$ABIS" | awk '{print $1}')
lib_path="apps/android/app/src/main/jniLibs/$first_abi/liba2d_ffi.so"

rm -rf apps/android/app/src/main/kotlin/uniffi
mkdir -p /tmp/a2d-uniffi-kt-regen
cargo run -p a2d-ffi --bin uniffi-bindgen -- generate \
  --library "$lib_path" \
  --language kotlin \
  --out-dir /tmp/a2d-uniffi-kt-regen \
  --no-format
mkdir -p apps/android/app/src/main/kotlin/uniffi/a2d_ffi
cp /tmp/a2d-uniffi-kt-regen/uniffi/a2d_ffi/a2d_ffi.kt apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt
rm -rf /tmp/a2d-uniffi-kt-regen

printf 'Rebuilt native lib(s) for: %s\n' "$ABIS"
echo "Regenerated apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt"
