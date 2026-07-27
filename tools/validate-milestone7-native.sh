#!/usr/bin/env bash
# Reproducibly validates the Milestone 7 AprilTag native dependency for the
# Android ABIs supported by v0.1. The script intentionally builds a2d-image
# directly: a2d-ffi does not expose image processing yet, so the existing
# binding-regeneration build would otherwise fail to compile this dependency.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required" >&2
  exit 2
fi
if ! cargo ndk --version >/dev/null 2>&1; then
  echo "cargo-ndk is required (cargo install cargo-ndk --locked)" >&2
  exit 2
fi
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  sdk_root="${ANDROID_SDK_ROOT:-${ANDROID_HOME:-$HOME/Android/Sdk}}"
  ANDROID_NDK_HOME=$(find "$sdk_root/ndk" -maxdepth 1 -mindepth 1 -type d | sort -V | tail -1)
  export ANDROID_NDK_HOME
fi
if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "ANDROID_NDK_HOME does not identify an installed NDK" >&2
  exit 2
fi

abis=${A2D_ANDROID_ABIS:-"arm64-v8a x86_64"}
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

printf 'Using ANDROID_NDK_HOME=%s\n' "$ANDROID_NDK_HOME"
printf 'Validating Android ABIs: %s\n' "$abis"
rustup target add "${rust_targets[@]}"

# .cargo/config.toml forces apriltag-sys to compile its bundled C sources
# statically, avoiding accidental use of a workstation-installed library.
cargo ndk "${ndk_args[@]}" build -p a2d-image --lib

# Exercise the real official detector on generated grayscale input and print the
# measured elapsed time. This is evidence collection, not a guessed threshold.
cargo test -p a2d-image official_detector_finds_and_resolves_four_generated_tags -- --nocapture
