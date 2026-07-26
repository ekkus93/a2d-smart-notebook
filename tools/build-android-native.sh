#!/usr/bin/env bash
# Cross-compiles a2d-ffi for Android and regenerates the Kotlin bindings the app module
# consumes. Not yet wired into the Gradle build itself (no Cargo/NDK integration there) -- run
# this manually after any a2d-ffi/a2d-core/a2d-domain/a2d-identity change before building the
# Android app. Wiring this into Gradle automatically (e.g. via a task the mozilla/
# rust-android-gradle plugin or similar provides) is a documented follow-up, not yet done.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

ABIS="${1:-x86_64}"  # space-separated cargo-ndk target names, e.g. "x86_64 arm64-v8a"

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  # Pick the newest installed NDK if the caller hasn't set one.
  export ANDROID_NDK_HOME
  ANDROID_NDK_HOME=$(find "${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}/ndk" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
fi
echo "Using ANDROID_NDK_HOME=$ANDROID_NDK_HOME"

# shellcheck disable=SC2086
cargo ndk -t $ABIS -o apps/android/app/src/main/jniLibs build -p a2d-ffi --lib

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

echo "Rebuilt native lib(s) for: $ABIS"
echo "Regenerated apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt"
