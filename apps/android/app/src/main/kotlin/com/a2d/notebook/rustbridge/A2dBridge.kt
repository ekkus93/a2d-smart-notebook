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
 * A single [A2dClient] is opened lazily against a library directory under the app's private
 * files dir and reused -- `A2dClient.open` is cheap but not free (it touches the filesystem),
 * and TODO 2.4's `A2dClient` is meant to be a long-lived handle, not something reopened per call.
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
