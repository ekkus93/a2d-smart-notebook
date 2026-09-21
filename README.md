# A2D Smart Notebook

A2D Smart Notebook is an Android app for turning handwritten A2D Smart Pages and A2D notebooks into a local, searchable digital library.

A2D pages use four printed Corner Markers and a Page Code. The Android scanner uses those markers to locate and rectify the page, resolves its identity, and stores the scan in the local library. The app also supports local OCR and OCR search, review workflows, and encrypted `.atnb` backup and restore.

The app is **local-first and accountless**. Core library, scanning, OCR, search, review, backup, and restore workflows do not require an A2D account or server.

The canonical application logic, persistence, image processing, OCR records, search data, and domain rules live in the **Rust core**. The Android UI is written in Kotlin with Jetpack Compose and calls the Rust core through UniFFI.

## Repository layout

```text
crates/       Rust workspace: domain, storage, image, OCR, search, backup, FFI, and related crates
apps/android/ Kotlin + Jetpack Compose Android application
fixtures/     Compatibility and test fixtures
tools/        Build, generation, verification, and test-support scripts
docs/         Architecture, design, and implementation documentation
```

## Development environment

The repository currently uses:

- Rust **1.94.1**, pinned by `rust-toolchain.toml`, with `rustfmt` and `clippy`
- JDK **21** for the Android/Gradle toolchain
- Android SDK with API **35**
- Android NDK **27.0.12077973**
- `cargo-ndk` **4.1.2**
- Android platform tools, including `adb`
- Git

Install Rust with [rustup](https://rustup.rs/) and install JDK 21 plus the Android SDK using Android Studio or the Android command-line tools.

After installing the Android SDK, make sure its location is exported. A typical Linux setup is:

```sh
export ANDROID_SDK_ROOT="$HOME/Android/Sdk"
export ANDROID_HOME="$ANDROID_SDK_ROOT"

sdkmanager --install \
  "platform-tools" \
  "platforms;android-35" \
  "ndk;27.0.12077973"

export ANDROID_NDK_HOME="$ANDROID_SDK_ROOT/ndk/27.0.12077973"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
```

Install the pinned `cargo-ndk` version:

```sh
cargo install cargo-ndk --version 4.1.2 --locked
```

Clone the repository and verify the toolchains:

```sh
git clone https://github.com/ekkus93/a2d-smart-notebook.git
cd a2d-smart-notebook

rustc --version
cargo --version
java -version
adb version
cargo ndk --version
```

The repository's `rust-toolchain.toml` causes rustup to select the pinned Rust compiler and install the required Rust components.

## Build the app

### Rust workspace

Build all Rust crates from the repository root:

```sh
cargo build --workspace
```

### Android native library and UniFFI binding

Before building the Android application, build the Rust shared libraries and generate the Kotlin UniFFI binding:

```sh
bash tools/build-android-native.sh "arm64-v8a x86_64"
```

This generates:

- Android native libraries under `apps/android/app/src/main/jniLibs/`
- the Kotlin UniFFI binding under `apps/android/app/src/main/kotlin/uniffi/`

These are generated build artifacts and are intentionally not committed. Do not hand-edit the generated Kotlin binding.

### Debug APK

Build the Android debug APK:

```sh
cd apps/android
./gradlew assembleDebug --no-daemon
```

The APK is written to:

```text
apps/android/app/build/outputs/apk/debug/app-debug.apk
```

## Lint and test

Run the Rust formatting check, Clippy, and full Rust test suite from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Before running Android checks, generate the Android native libraries and UniFFI binding if they are not already present:

```sh
bash tools/build-android-native.sh "arm64-v8a x86_64"
```

Then run Android lint, unit tests, and an APK build:

```sh
cd apps/android
./gradlew lint test assembleDebug --no-daemon
```

To run Android instrumentation/UI tests, connect a device or start an emulator first, then run:

```sh
cd apps/android
./gradlew connectedDebugAndroidTest --no-daemon
```

The permanent CI workflow also performs dependency/license checks, generated-binding verification, APK/native-library verification, storage-failure tests, and the Android emulator test suite.

## Install the app

The Android app requires **Android 8.0 / API 26 or newer**.

Enable USB debugging on a physical Android device or start an emulator, then verify that `adb` can see it:

```sh
adb devices
```

Build the native libraries and APK if needed:

```sh
bash tools/build-android-native.sh "arm64-v8a x86_64"

cd apps/android
./gradlew assembleDebug --no-daemon
```

Install or replace the debug build:

```sh
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Alternatively, from `apps/android/`, Gradle can build and install the debug app directly:

```sh
./gradlew installDebug --no-daemon
```

The app requests camera permission when scanning pages.
