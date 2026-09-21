# A2D Smart Notebook

A2D Smart Notebook is an Android app for turning handwritten paper pages into a local, searchable digital notebook.

Printed A2D Smart Pages use four Corner Markers and a Page Code so the app can locate a page, correct its perspective, identify it, and file the scan automatically. The app keeps its canonical library on the device, supports OCR-backed search, and provides encrypted `.atnb` backup and restore without requiring an account.

The application is built from:

- a shared Rust core for canonical data, persistence, image processing, OCR coordination, search, backup, and domain rules;
- a Kotlin + Jetpack Compose Android UI;
- a UniFFI boundary between Kotlin and the Rust core.

## Repository layout

```text
crates/       Rust workspace
apps/android/ Kotlin + Jetpack Compose Android application
fixtures/     Compatibility and scan fixtures
tools/        Build, generation, and verification scripts
docs/         Technical specifications and implementation notes
```

## Development environment

The project currently builds and tests on Linux in CI. A normal Android development workstation needs:

- Git
- Rust through `rustup`
- JDK 21
- Android SDK Platform 35
- Android SDK Platform Tools
- Android NDK `27.0.12077973`
- `cargo-ndk` `4.1.2`
- Python 3
- Android Studio is optional, but convenient for SDK/device management

The repository pins Rust `1.94.1` plus `rustfmt` and `clippy` in `rust-toolchain.toml`.

### 1. Clone the repository

```sh
git clone https://github.com/ekkus93/a2d-smart-notebook.git
cd a2d-smart-notebook
```

### 2. Install the Rust toolchain

Install Rust with `rustup` if it is not already installed. Then, from the repository root:

```sh
rustup show
rustc --version
cargo --version
```

Running a Rust command in this repository causes `rustup` to use the pinned toolchain from `rust-toolchain.toml`.

Install the Android Cargo helper used by the project:

```sh
cargo install cargo-ndk --version 4.1.2 --locked
```

### 3. Install the Android SDK and NDK

Using Android Studio's SDK Manager or `sdkmanager`, install:

```text
platform-tools
platforms;android-35
build-tools;35.0.0
ndk;27.0.12077973
```

With the command-line SDK manager:

```sh
sdkmanager --install \
  "platform-tools" \
  "platforms;android-35" \
  "build-tools;35.0.0" \
  "ndk;27.0.12077973"

sdkmanager --licenses
```

Set the SDK and NDK environment variables. Adjust the SDK path if yours is elsewhere:

```sh
export ANDROID_SDK_ROOT="$HOME/Android/Sdk"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
export ANDROID_NDK_HOME="$ANDROID_SDK_ROOT/ndk/27.0.12077973"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
export PATH="$ANDROID_SDK_ROOT/platform-tools:$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$PATH"
```

Verify the tools are visible:

```sh
java -version
adb version
cargo ndk --version
```

## Build the app

The Android app packages Rust native libraries for `arm64-v8a` and `x86_64`. Build those libraries and regenerate the Kotlin UniFFI binding before running Gradle:

```sh
bash tools/build-android-native.sh "arm64-v8a x86_64"
```

The generated native libraries under `apps/android/app/src/main/jniLibs/` and the generated Kotlin UniFFI binding under `apps/android/app/src/main/kotlin/uniffi/` are build artifacts and are intentionally not committed.

Build the debug APK:

```sh
cd apps/android
./gradlew assembleDebug --no-daemon
cd ../..
```

The APK is written to:

```text
apps/android/app/build/outputs/apk/debug/app-debug.apk
```

To build only the Rust workspace:

```sh
cargo build --workspace
```

## Lint and test

### Rust

Check formatting without modifying files:

```sh
cargo fmt --all -- --check
```

Run Clippy with the same warning policy used by CI:

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run the Rust test suite:

```sh
cargo test --workspace --all-features
```

### Android

Generate the Android native libraries and UniFFI binding first:

```sh
bash tools/build-android-native.sh "arm64-v8a x86_64"
```

Then run Android lint, JVM unit tests, and build verification:

```sh
cd apps/android
./gradlew lint test assembleDebug --no-daemon
cd ../..
```

Verify the produced APK's native libraries and packaged production artifacts:

```sh
python tools/verify-android-apk.py \
  apps/android/app/build/outputs/apk/debug/app-debug.apk
```

### Instrumentation and UI tests

Start an Android device or emulator and confirm that ADB can see it:

```sh
adb devices
```

Then run the connected Android test suite:

```sh
cd apps/android
./gradlew connectedDebugAndroidTest --no-daemon
cd ../..
```

The hosted CI pipeline additionally runs dependency/license checks, generated-binding checks, storage failure-path tests, APK verification, and Android emulator integration tests.

## Install the app

A2D Smart Notebook supports Android API 26 and newer.

Connect a device with USB debugging enabled, or start an emulator, then confirm it is available:

```sh
adb devices
```

After building the APK, install or replace the debug build with:

```sh
adb install -r apps/android/app/build/outputs/apk/debug/app-debug.apk
```

Alternatively, after the Rust native libraries and UniFFI binding have been generated:

```sh
cd apps/android
./gradlew installDebug
```

The app requests camera permission for page scanning.

## UniFFI regeneration

Whenever the exported `a2d-ffi` API changes, regenerate the Android native libraries and Kotlin binding:

```sh
bash tools/build-android-native.sh
```

Do not hand-edit the generated Kotlin binding. Rust is the source of truth for the FFI surface.
