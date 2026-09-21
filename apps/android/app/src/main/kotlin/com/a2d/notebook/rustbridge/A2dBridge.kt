package com.a2d.notebook.rustbridge

import android.content.Context
import java.io.File
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OpenLibraryRequest

/**
 * Thin façade over the generated UniFFI bindings (spec §25: `rustbridge` package, "ViewModels
 * MUST call typed Rust use cases"). Feature code calls through here rather than importing
 * `uniffi.a2d_ffi` directly, so the generated-binding package name stays in one place.
 *
 * Production currently supports exactly one app-private library for the lifetime of the Android
 * process. A single [A2dClient] is opened lazily for that library and reused by the Activity,
 * navigation, OCR search/readback/correction, and the OCR queue runtime. Recomposition therefore
 * never opens a second native client.
 *
 * Runtime library switching is intentionally unsupported in this milestone. Consequently there is
 * no mid-process close/reopen path: the process owns this handle until process teardown, when the
 * OS releases the process and its native resources. If runtime library switching is introduced,
 * this singleton must be replaced by an explicit owner that stops the queue runtime, disposes the
 * old client/gateways, opens the new library, and recreates all client-bound controllers together.
 */
object A2dBridge {
    fun libraryDirectory(context: Context): File = context.filesDir.resolve("library")

    @Volatile
    private var client: A2dClient? = null

    fun client(context: Context): A2dClient {
        client?.let { return it }
        synchronized(this) {
            client?.let { return it }
            val libraryPath = libraryDirectory(context).absolutePath
            val opened = A2dClient.open(OpenLibraryRequest(libraryPath = libraryPath))
            client = opened
            return opened
        }
    }
}
