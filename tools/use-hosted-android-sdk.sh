#!/usr/bin/env bash
set -euo pipefail

android_home="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/usr/local/lib/android/sdk}}"
if [[ ! -d "$android_home" ]]; then
  echo "Android SDK directory not found: $android_home" >&2
  exit 1
fi

cmdline_bin="$(find "$android_home/cmdline-tools" \
  -mindepth 2 \
  -maxdepth 2 \
  -type f \
  -name sdkmanager \
  -printf '%h\n' \
  | sort -V \
  | tail -n 1)"
if [[ -z "$cmdline_bin" ]]; then
  echo "No sdkmanager found under $android_home/cmdline-tools" >&2
  exit 1
fi

{
  echo "ANDROID_HOME=$android_home"
  echo "ANDROID_SDK_ROOT=$android_home"
} >> "$GITHUB_ENV"

{
  echo "$cmdline_bin"
  echo "$android_home/platform-tools"
  echo "$android_home/emulator"
} >> "$GITHUB_PATH"

"$cmdline_bin/sdkmanager" --version
