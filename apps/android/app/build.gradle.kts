plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.a2d.notebook"
    // 35, not the 36 platform/build-tools/AVD image also installed here: AGP 8.7.3 (the version
    // this project pins) is only tested up through compileSdk 35 and warns on 36. The app still
    // runs fine on the API 36 emulator regardless -- compileSdk governs what the code compiles
    // against, not what the device/emulator's own platform version is.
    compileSdk = 35

    defaultConfig {
        applicationId = "com.a2d.notebook"
        // Minimum API 26 (Android 8.0, Oreo): an open decision (spec/TODO leave min API
        // unspecified). Picked as a modern-but-broad floor -- still covers the large majority of
        // active devices while getting notification channels, adaptive icons, and other
        // platform features later milestones (background OCR/backup work, notifications) will
        // want, without the compatibility burden of supporting pre-Oreo devices. Flagged per
        // CLAUDE.md's open-decision policy; revisit if real device-share data says otherwise.
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildFeatures {
        compose = true
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation(platform("androidx.compose:compose-bom:2024.10.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.navigation:navigation-compose:2.8.4")

    // UniFFI's generated Kotlin bindings (uniffi/a2d_ffi/a2d_ffi.kt) load the Rust cdylib
    // (src/main/jniLibs/<abi>/liba2d_ffi.so) via JNA. The @aar classifier is the Android-packaged
    // variant -- the plain desktop JNA jar doesn't work on-device.
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    // ZXing: used only by the ADR 0001 QR decoder spike (androidTest) to encode and decode a
    // real QR image round-trip against Rust-generated canonical payload text. Not a main-source
    // dependency -- production QR rendering/decoding library choice is still open (Milestone
    // 7.4/12's job), this is just the spike's proxy for "a real Android decoder."
    androidTestImplementation("com.google.zxing:core:3.5.3")

    testImplementation("junit:junit:4.13.2")

    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation(platform("androidx.compose:compose-bom:2024.10.00"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
}
