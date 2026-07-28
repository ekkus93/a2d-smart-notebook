plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val generatedLicenseAssets = layout.buildDirectory.dir("generated/licenseAssets")
val prepareLicenseAssets by tasks.registering(Copy::class) {
    from(rootProject.file("../../LICENSE"))
    into(generatedLicenseAssets)
    rename { "APACHE-2.0.txt" }
}

android {
    namespace = "com.a2d.notebook"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.a2d.notebook"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += setOf("arm64-v8a", "x86_64")
        }
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

    sourceSets {
        getByName("main").assets.srcDir(generatedLicenseAssets)
        getByName("androidTest").assets.srcDir(rootProject.file("../../fixtures/scans/generated"))
    }
}

tasks.named("preBuild").configure {
    dependsOn(prepareLicenseAssets)
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
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-ktx:2.8.7")

    // CameraX 1.6.x requires compileSdk 36 and AGP 8.9.1+. Keep the adapter on the newest
    // stable 1.5.x release compatible with this project's pinned compileSdk 35 / AGP 8.7.3.
    val cameraXVersion = "1.5.3"
    implementation("androidx.camera:camera-core:$cameraXVersion")
    implementation("androidx.camera:camera-camera2:$cameraXVersion")
    implementation("androidx.camera:camera-lifecycle:$cameraXVersion")
    implementation("androidx.camera:camera-view:$cameraXVersion")

    implementation("com.google.zxing:core:3.5.3")

    // UniFFI's generated Kotlin bindings load the Rust cdylib through JNA.
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    testImplementation("junit:junit:4.13.2")

    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.6.1")
    androidTestImplementation("com.google.zxing:core:3.5.3")
    androidTestImplementation(platform("androidx.compose:compose-bom:2024.10.00"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
}
