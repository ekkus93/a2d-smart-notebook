package com.a2d.notebook.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.navigation.compose.rememberNavController
import com.a2d.notebook.feature.ocr.AndroidOcrQueueRuntime
import com.a2d.notebook.navigation.A2dNavHost
import com.a2d.notebook.rustbridge.A2dBridge

/**
 * Android composition root. Rust owns persistent/business state; this Activity obtains the one
 * process-lifetime native client from [A2dBridge] and passes that same handle to both the OCR queue
 * runtime and production navigation. Search, Page Viewer readback/actions, and correction gateways
 * are all derived from this handle inside `A2dNavHost`.
 *
 * Runtime active-library switching is intentionally unsupported in the current product. The
 * process-lifetime handle is therefore not closed on Activity recreation; doing so would invalidate
 * the queue runtime and other client-bound work that deliberately outlives one Activity instance.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val client = A2dBridge.client(applicationContext)
        AndroidOcrQueueRuntime.resume(
            context = applicationContext,
            client = client,
            libraryRoot = A2dBridge.libraryDirectory(applicationContext),
        )
        setContent {
            MaterialTheme {
                Surface {
                    A2dNavHost(
                        navController = rememberNavController(),
                        client = client,
                    )
                }
            }
        }
    }
}
