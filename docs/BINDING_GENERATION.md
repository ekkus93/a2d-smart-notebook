# UniFFI Binding Generation and Ownership

A2D uses UniFFI to expose the authoritative Rust core to Android/Kotlin and a future iOS/Swift client. Generated artifacts do not all have the same ownership policy.

## Committed Android Kotlin binding

The Android Kotlin binding is committed at:

```text
apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt
```

It is generated source, not handwritten source. Do not edit it manually.

Any change to the exported `a2d-ffi` API must regenerate and commit this Kotlin file in the same change. The permanent Android binding-drift CI job rebuilds the Android native library, regenerates the binding, and fails if the generated file differs from the committed copy.

Use the repository's Android generator:

```sh
bash tools/build-android-native.sh
```

By default it builds `arm64-v8a` and `x86_64`, then generates Kotlin from the first built Android library. To select ABIs explicitly:

```sh
A2D_ANDROID_ABIS="arm64-v8a x86_64" bash tools/build-android-native.sh
```

After generation:

```sh
git diff -- apps/android/app/src/main/kotlin/uniffi/a2d_ffi/a2d_ffi.kt
```

Review the exported API change, but never hand-correct converter metadata, checksums, symbol names, or record layouts.

Regenerate a second time before finishing. The second generation must leave no diff.

## Ephemeral native Android libraries

Android native libraries generated under:

```text
apps/android/app/src/main/jniLibs/
```

are local build artifacts and are not committed. CI rebuilds the required ABIs and verifies the packaged APK symbols and notices.

## Desktop Kotlin and Swift smoke output

`tools/generate-bindings.sh` builds the desktop `a2d-ffi` library and writes disposable Kotlin and Swift bindings under `target/bindings/` by default:

```sh
bash tools/generate-bindings.sh
```

An alternate output directory may be supplied as the first argument:

```sh
bash tools/generate-bindings.sh target/my-binding-check
```

These desktop-generated files are not committed. They exist for inspection and for the Kotlin/Swift binding-generation smoke tests.

The permanent Rust test suite runs:

```sh
cargo test -p a2d-ffi --test binding_generation
```

That test verifies that both Kotlin and Swift generation succeeds and that the expected exported API symbols are present. Swift generation is mandatory even though the SwiftUI application is deferred.

## Required checks after an FFI change

Run the narrow checks first:

```sh
cargo test -p a2d-ffi --test binding_generation
bash tools/build-android-native.sh
./gradlew -p apps/android :app:compileDebugKotlin
```

Then run the repository's full required checks, or push to `master` and verify every permanent CI job for the exact commit.

A change is not complete when:

- Rust exports a new type or method but the committed Kotlin file is stale.
- Local Android compilation depends on an uncommitted regenerated file.
- A generated file was edited by hand.
- Swift generation was skipped because no iOS UI exists yet.
- CI binding drift or APK symbol verification has not passed for the exact final commit.

## Troubleshooting stale bindings

Symptoms include unresolved Kotlin types or methods, a failing binding-drift job, or an APK symbol check that expects an export absent from the packaged native library.

1. Confirm the Rust FFI surface compiles.
2. Run `cargo test -p a2d-ffi --test binding_generation`.
3. Run `bash tools/build-android-native.sh`.
4. Commit the generated Kotlin file verbatim.
5. Re-run the generator and confirm the working tree remains clean.
6. Verify Android compilation and the permanent CI jobs.

Do not resolve stale-binding failures by deleting expected-symbol checks or manually patching generated Kotlin.
