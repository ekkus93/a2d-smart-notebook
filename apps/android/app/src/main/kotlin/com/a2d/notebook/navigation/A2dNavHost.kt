package com.a2d.notebook.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.a2d.notebook.feature.home.HomeScreen
import com.a2d.notebook.feature.library.LibraryHubScreen
import com.a2d.notebook.feature.library.PageBrowserScreen
import com.a2d.notebook.feature.library.PageViewerScreen
import com.a2d.notebook.feature.notebook.NotebookDetailScreen
import com.a2d.notebook.feature.notebook.NotebookLibraryScreen
import com.a2d.notebook.feature.notebook.NotebookSetupScreen
import com.a2d.notebook.feature.notebook.PageCodeScreen
import com.a2d.notebook.feature.review.NeedsReviewScreen
import com.a2d.notebook.feature.scanner.singlepage.PolicyAwareBatchScannerRoute
import com.a2d.notebook.feature.scanner.singlepage.SinglePageScannerScreen
import com.a2d.notebook.feature.smartpage.SmartPageLibraryScreen
import com.a2d.notebook.feature.smartpage.SmartPagesScreen
import com.a2d.notebook.feature.version.VersionHistoryScreen

object A2dDestinations {
    const val HOME = "home"
    const val LIBRARY = "library"
    const val PAGES = "library/pages"
    const val PAGE_VIEWER_PATTERN = "library/pages/{pageId}"
    const val NEEDS_REVIEW = "library/needs-review"
    const val SMART_PAGE_LIBRARY = "library/smart-pages"
    const val SINGLE_PAGE_SCANNER = "scanner/single"
    const val BATCH_SCANNER = "scanner/batch"
    const val NOTEBOOKS = "notebooks"
    const val ADD_NOTEBOOK = "notebooks/add"
    const val NOTEBOOK_DETAIL_PATTERN = "notebooks/detail/{notebookId}"
    const val SMART_PAGES = "smart-pages"
    const val PAGE_CODE_PATTERN = "page-code/{notebookId}"
    const val VERSION_HISTORY_PATTERN = "versions/{pageId}"

    fun pageViewer(pageId: String) = "library/pages/$pageId"

    fun pageCode(notebookId: String) = "page-code/$notebookId"

    fun notebookDetail(notebookId: String) = "notebooks/detail/$notebookId"

    fun versionHistory(pageId: String) = "versions/$pageId"
}

@Composable
fun A2dNavHost(navController: NavHostController) {
    NavHost(navController = navController, startDestination = A2dDestinations.HOME) {
        composable(A2dDestinations.HOME) {
            HomeScreen(
                onScanPage = { navController.navigate(A2dDestinations.SINGLE_PAGE_SCANNER) },
                onBatchScan = { navController.navigate(A2dDestinations.BATCH_SCANNER) },
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
                onOpenLibrary = { navController.navigate(A2dDestinations.LIBRARY) },
            )
        }
        composable(A2dDestinations.LIBRARY) {
            LibraryHubScreen(
                onBack = { navController.navigateUp() },
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onOpenSmartPages = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenPages = { navController.navigate(A2dDestinations.PAGES) },
                onOpenPageSets = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenCollections = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenNeedsReview = { navController.navigate(A2dDestinations.NEEDS_REVIEW) },
            )
        }
        composable(A2dDestinations.PAGES) {
            PageBrowserScreen(
                onBack = { navController.navigateUp() },
                onOpenPage = { pageId ->
                    navController.navigate(A2dDestinations.pageViewer(pageId))
                },
                onOpenVersions = { pageId ->
                    navController.navigate(A2dDestinations.versionHistory(pageId))
                },
            )
        }
        composable(
            route = A2dDestinations.PAGE_VIEWER_PATTERN,
            arguments = listOf(navArgument("pageId") { type = NavType.StringType }),
        ) { entry ->
            PageViewerScreen(
                pageId = requireNotNull(entry.arguments?.getString("pageId")),
                onBack = { navController.navigateUp() },
                onOpenVersions = { pageId ->
                    navController.navigate(A2dDestinations.versionHistory(pageId))
                },
                onOpenNeedsReview = { navController.navigate(A2dDestinations.NEEDS_REVIEW) },
            )
        }
        composable(A2dDestinations.NEEDS_REVIEW) {
            NeedsReviewScreen(
                onBack = { navController.navigateUp() },
                onOpenVersions = { pageId ->
                    navController.navigate(A2dDestinations.versionHistory(pageId))
                },
            )
        }
        composable(A2dDestinations.SMART_PAGE_LIBRARY) {
            SmartPageLibraryScreen(
                onBack = { navController.navigateUp() },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
            )
        }
        composable(A2dDestinations.SINGLE_PAGE_SCANNER) {
            SinglePageScannerScreen(
                onBack = { navController.navigateUp() },
                onOpenVersions = { pageId ->
                    navController.navigate(A2dDestinations.versionHistory(pageId))
                },
            )
        }
        composable(A2dDestinations.BATCH_SCANNER) {
            PolicyAwareBatchScannerRoute(onBack = { navController.navigateUp() })
        }
        composable(A2dDestinations.NOTEBOOKS) {
            NotebookLibraryScreen(
                onBack = { navController.navigateUp() },
                onAddNotebook = { navController.navigate(A2dDestinations.ADD_NOTEBOOK) },
                onOpenNotebook = { notebookId ->
                    navController.navigate(A2dDestinations.notebookDetail(notebookId))
                },
            )
        }
        composable(
            route = A2dDestinations.NOTEBOOK_DETAIL_PATTERN,
            arguments = listOf(navArgument("notebookId") { type = NavType.StringType }),
        ) { entry ->
            NotebookDetailScreen(
                notebookId = requireNotNull(entry.arguments?.getString("notebookId")),
                onBack = { navController.navigateUp() },
                onScanPage = { navController.navigate(A2dDestinations.SINGLE_PAGE_SCANNER) },
                onBatchScan = { navController.navigate(A2dDestinations.BATCH_SCANNER) },
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
        composable(
            route = A2dDestinations.VERSION_HISTORY_PATTERN,
            arguments = listOf(navArgument("pageId") { type = NavType.StringType }),
        ) { entry ->
            VersionHistoryScreen(
                pageId = requireNotNull(entry.arguments?.getString("pageId")),
                onBack = { navController.navigateUp() },
            )
        }
    }
}
