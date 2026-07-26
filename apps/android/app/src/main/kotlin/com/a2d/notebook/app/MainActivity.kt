package com.a2d.notebook.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.navigation.compose.rememberNavController
import com.a2d.notebook.navigation.A2dNavHost

/**
 * Placeholder shell (TODO 1.2). Rust owns persistent/business state (spec section 25); this
 * Activity only hosts the Compose navigation graph.
 */
class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Surface {
                    A2dNavHost(navController = rememberNavController())
                }
            }
        }
    }
}
