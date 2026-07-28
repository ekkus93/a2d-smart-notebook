package com.a2d.notebook.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.a2d.notebook.feature.home.HomeScreen
import com.a2d.notebook.feature.notebook.NotebookLibraryScreen
import com.a2d.notebook.feature.notebook.NotebookSetupScreen
import com.a2d.notebook.feature.notebook.PageCodeScreen
import com.a2d.notebook.feature.scanner.singlepage.SinglePageScannerScreen
import com.a2d.notebook.feature.smartpage.SmartPagesScreen

object A2dDestinations {
    const val HOME = "home"
    const val SINGLE_PAGE_SCANNER = "scanner/single"
    const val NOTEBOOKS = "notebooks"
    const val ADD_NOTEBOOK = "notebooks/add"
    const val SMART_PAGES = "smart-pages"
    const val PAGE_CODE_PATTERN = "page-code/{notebookId}"

    fun pageCode(notebookId: String) = "page-code/$notebookId"
}

@Composable
fun A2dNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = A2dDestinations.HOME) {
        composable(A2dDestinations.HOME) {
            HomeScreen(
                onScanPage = { navController.navigate(A2dDestinations.SINGLE_PAGE_SCANNER) },
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
            )
        }
        composable(A2dDestinations.SINGLE_PAGE_SCANNER) {
            SinglePageScannerScreen(onBack = { navController.navigateUp() })
        }
        composable(A2dDestinations.NOTEBOOKS) {
            NotebookLibraryScreen(
                onBack = { navController.navigateUp() },
                onAddNotebook = { navController.navigate(A2dDestinations.ADD_NOTEBOOK) },
            )
        }
        composable(A2dDestinations.ADD_NOTEBOOK) {
            NotebookSetupScreen(
                onBack = { navController.navigateUp() },
                onResolveFirstPage = { notebookId ->
                    navController.navigate(A2dDestinations.pageCode(notebookId))
                },
            )
        }
        composable(A2dDestinations.SMART_PAGES) {
            SmartPagesScreen(onBack = { navController.navigateUp() })
        }
        composable(
            route = A2dDestinations.PAGE_CODE_PATTERN,
            arguments = listOf(navArgument("notebookId") { type = NavType.StringType }),
        ) { entry ->
            PageCodeScreen(
                notebookId = requireNotNull(entry.arguments?.getString("notebookId")),
                onBack = { navController.navigateUp() },
            )
        }
    }
}
