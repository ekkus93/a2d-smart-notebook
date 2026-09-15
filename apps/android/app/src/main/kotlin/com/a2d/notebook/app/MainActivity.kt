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
 * Android composition root. Rust owns persistent/business state; this Activity owns the
 * app-lifetime native client and passes that same open-library handle into production navigation.
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
