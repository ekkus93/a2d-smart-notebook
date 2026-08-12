#!/usr/bin/env bash
# Prepares a disposable Android instrumentation build that contains the intentional UniFFI panic
# endpoint. This script is for the dedicated CI emulator job only. Production builds omit the
# feature; the generated Kotlin binding is untracked build output in both cases.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

export A2D_FFI_CARGO_FEATURES="ffi-test-panic"
bash ./tools/build-android-native-libs.sh "x86_64"

library="apps/android/app/src/main/jniLibs/x86_64/liba2d_ffi.so"
if [ ! -s "$library" ]; then
  echo "Feature-enabled Android library was not produced: $library" >&2
  exit 1
fi

out_dir=$(mktemp -d "${TMPDIR:-/tmp}/a2d-ffi-panic-bindings.XXXXXX")
cleanup() {
  rm -rf "$out_dir"
}
trap cleanup EXIT

cargo run -p a2d-ffi --features ffi-test-panic --bin uniffi-bindgen -- generate \
  --library "$library" \
  --language kotlin \
  --out-dir "$out_dir" \
  --no-format

generated_binding="$out_dir/uniffi/a2d_ffi/a2d_ffi.kt"
android_binding="apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt"
if [ ! -s "$generated_binding" ]; then
  echo "Feature-enabled Kotlin binding was not generated: $generated_binding" >&2
  exit 1
fi
if ! grep -q 'triggerPanicForTesting' "$generated_binding"; then
  echo "Feature-enabled Kotlin binding does not expose triggerPanicForTesting" >&2
  exit 1
fi
install -D -m 0644 "$generated_binding" "$android_binding"

panic_test_source="tools/ffi-panic-test/PanicPropagationTest.kt"
panic_test_destination="apps/android/app/src/androidTest/kotlin/com/a2d/notebook/app/PanicPropagationTest.kt"
if [ ! -s "$panic_test_source" ]; then
  echo "CI-only panic instrumentation source is missing: $panic_test_source" >&2
  exit 1
fi
install -D -m 0644 "$panic_test_source" "$panic_test_destination"

printf 'Prepared feature-enabled Android FFI panic test workspace\n'
