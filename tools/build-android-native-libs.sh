#!/usr/bin/env bash
# Builds the Rust UniFFI cdylib for the Android ABIs packaged by the v0.1 app.
# This script writes only generated native libraries under app/src/main/jniLibs;
# it does not regenerate or modify committed Kotlin bindings.
#
# Production callers leave A2D_FFI_CARGO_FEATURES unset. Dedicated test jobs may provide an explicit
# whitespace-separated feature list; the script never enables defect-injection features implicitly.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 2
fi
if ! cargo ndk --version >/dev/null 2>&1; then
  echo "cargo-ndk is required" >&2
  exit 2
fi

if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
  if [ -d "$sdk_root/ndk" ]; then
    ANDROID_NDK_HOME=$(find "$sdk_root/ndk" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
    export ANDROID_NDK_HOME
  fi
fi
if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "ANDROID_NDK_HOME does not identify an installed NDK" >&2
  exit 2
fi

abis="${1:-${A2D_ANDROID_ABIS:-arm64-v8a x86_64}}"
rust_targets=()
ndk_args=()
for abi in $abis; do
  case "$abi" in
    arm64-v8a)
      rust_targets+=(aarch64-linux-android)
      ;;
    x86_64)
      rust_targets+=(x86_64-linux-android)
      ;;
    *)
      echo "Unsupported A2D Android ABI: $abi" >&2
      exit 2
      ;;
  esac
  ndk_args+=(-t "$abi")
done

rustup target add "${rust_targets[@]}"
output_dir="apps/android/app/src/main/jniLibs"
mkdir -p "$output_dir"

cargo_args=(build -p a2d-ffi --lib)
if [ -n "${A2D_FFI_CARGO_FEATURES:-}" ]; then
  cargo_args+=(--features "$A2D_FFI_CARGO_FEATURES")
fi
cargo ndk "${ndk_args[@]}" -o "$output_dir" "${cargo_args[@]}"

for abi in $abis; do
  library="$output_dir/$abi/liba2d_ffi.so"
  if [ ! -s "$library" ]; then
    echo "Expected native library was not produced: $library" >&2
    exit 1
  fi
done

printf 'Built Android native libraries for: %s\n' "$abis"
if [ -n "${A2D_FFI_CARGO_FEATURES:-}" ]; then
  printf 'Enabled explicit a2d-ffi Cargo features: %s\n' "$A2D_FFI_CARGO_FEATURES"
fi
