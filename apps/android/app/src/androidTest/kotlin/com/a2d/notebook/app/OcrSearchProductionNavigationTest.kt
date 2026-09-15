package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.home.HomeScreenTestTags
import com.a2d.notebook.feature.library.LibraryHubTestTags
import com.a2d.notebook.feature.library.PageViewerScreen
import com.a2d.notebook.feature.library.PageViewerTestTags
import com.a2d.notebook.feature.ocr.AndroidOcrSearchController
import com.a2d.notebook.feature.ocr.AndroidOcrSearchDocumentKind
import com.a2d.notebook.feature.ocr.AndroidOcrSearchGateway
import com.a2d.notebook.feature.ocr.AndroidOcrSearchHit
import com.a2d.notebook.feature.ocr.AndroidOcrSearchRequest
import com.a2d.notebook.feature.ocr.AndroidOcrSearchResults
import com.a2d.notebook.feature.ocr.OcrSearchScreen
import com.a2d.notebook.feature.ocr.OcrSearchTestTags
import com.a2d.notebook.navigation.A2dNavHost
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OpenLibraryRequest

@RunWith(AndroidJUnit4::class)
class OcrSearchProductionNavigationTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun realNavGraphUsesProductionControllerAndPreservesRustSearchErrors() {
        val root = composeRule.activity.filesDir.resolve("ocr-search-nav-${UUID.randomUUID()}")
        val client = A2dClient.open(OpenLibraryRequest(libraryPath = root.absolutePath))

        try {
            composeRule.activity.setContent {
                MaterialTheme {
                    A2dNavHost(
                        navController = rememberNavController(),
                        client = client,
                    )
                }
            }

            composeRule.onNodeWithTag(HomeScreenTestTags.LIBRARY).performScrollTo().performClick()
            composeRule.onNodeWithTag(LibraryHubTestTags.OCR_SEARCH).performScrollTo().performClick()
            composeRule.onNodeWithTag(OcrSearchTestTags.TITLE).assertIsDisplayed()

            // '*' is accepted by the text field but rejected by SQLite FTS query parsing. This
            // proves the real nav destination is using the FFI-backed production controller rather
            // than the nullable/not-connected fallback or an isolated fake presentation state.
            composeRule.onNodeWithTag(OcrSearchTestTags.QUERY_FIELD).performTextInput("*")
            composeRule.onNodeWithTag(OcrSearchTestTags.SUBMIT).performClick()
            composeRule.waitUntil(timeoutMillis = 10_000) {
                composeRule.onAllNodesWithTag(OcrSearchTestTags.ERROR).fetchSemanticsNodes().isNotEmpty()
            }
            composeRule.onNodeWithTag(OcrSearchTestTags.ERROR).assertIsDisplayed()
            composeRule.onNodeWithText("not connected", substring = true, ignoreCase = true).assertDoesNotExist()
        } finally {
            root.deleteRecursively()
        }
    }

    @Test
    fun searchHitSelectionUsesRealNavGraphToOpenPageViewer() {
        val pageId = "01ARZ3NDEKTSV4RRFFQ69G5FAV"
        val controller =
            AndroidOcrSearchController(
                gateway =
                    object : AndroidOcrSearchGateway {
                        override fun searchOcrText(request: AndroidOcrSearchRequest) =
                            AndroidOcrSearchResults(
                                query = request.query,
                                hits =
                                    listOf(
                                        AndroidOcrSearchHit(
                                            pageId = pageId,
                                            scanId = "01ARZ3NDEKTSV4RRFFQ69G5FB0",
                                            ocrRunId = "01ARZ3NDEKTSV4RRFFQ69G5FB1",
                                            textRegionId = "01ARZ3NDEKTSV4RRFFQ69G5FB2",
                                            documentKind = AndroidOcrSearchDocumentKind.TextRegion,
                                            snippet = "known [notebook] hit",
                                        ),
                                    ),
                            )
                    },
                searchDispatcher = Dispatchers.Unconfined,
            )

        composeRule.activity.setContent {
            MaterialTheme {
                val navController = rememberNavController()
                NavHost(
                    navController = navController,
                    startDestination = "search",
                ) {
                    composable("search") {
                        OcrSearchScreen(
                            onBack = {},
                            onOpenPage = { navController.navigate("page/$it") },
                            searchController = controller,
                        )
                    }
                    composable(
                        route = "page/{pageId}",
                        arguments = listOf(navArgument("pageId") { type = NavType.StringType }),
                    ) { entry ->
                        PageViewerScreen(
                            pageId = requireNotNull(entry.arguments?.getString("pageId")),
                            onBack = {},
                            onOpenVersions = {},
                            onOpenNeedsReview = {},
                        )
                    }
                }
            }
        }

        composeRule.onNodeWithTag(OcrSearchTestTags.QUERY_FIELD).performTextInput("notebook")
        composeRule.onNodeWithTag(OcrSearchTestTags.SUBMIT).performClick()
        composeRule.waitUntil(timeoutMillis = 10_000) {
            composeRule.onAllNodesWithTag(OcrSearchTestTags.OPEN_PAGE).fetchSemanticsNodes().isNotEmpty()
        }
        composeRule.onNodeWithTag(OcrSearchTestTags.OPEN_PAGE).performScrollTo().performClick()
        composeRule.onNodeWithTag(PageViewerTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithText("Page ID: $pageId").assertIsDisplayed()
    }
}
