package com.a2d.notebook.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import com.a2d.notebook.feature.home.HomeScreen

/**
 * The navigation shell TODO 1.2 asks for. A single "home" route today; later milestones add
 * routes here as their screens land (spec section 26's screen inventory) rather than this file
 * anticipating routes that don't exist yet.
 */
object A2dDestinations {
    const val HOME = "home"
}

@Composable
fun A2dNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = A2dDestinations.HOME) {
        composable(A2dDestinations.HOME) {
            HomeScreen()
        }
    }
}
