package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.home.HomeBackupStatus
import com.a2d.notebook.feature.home.HomeDashboardState
import com.a2d.notebook.feature.home.HomeNotebookSummary
import com.a2d.notebook.feature.home.HomeScreen
import com.a2d.notebook.feature.home.HomeScreenTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class HomeScreenUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun emptyHomeExplainsLocalFirstLibraryAndShowsPrimaryActions() {
        composeRule.activity.setContent {
            MaterialTheme {
                HomeScreen(
                    onScanPage = {},
                    onBatchScan = {},
                    onOpenNotebooks = {},
                    onCreateSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithTag(HomeScreenTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.LOCAL_FIRST).assertIsDisplayed()
        composeRule.onNodeWithText("No account or A2D server", substring = true).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.PRIMARY_ACTIONS).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.SCAN_PAGE).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.BATCH_SCAN).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.LIBRARY).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.NOTEBOOKS).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.SMART_PAGES).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.IMPORT).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.EMPTY_STATE).assertIsDisplayed()
        composeRule.onNodeWithText("Manual backup and restore", substring = true).assertIsDisplayed()
    }

    @Test
    fun populatedHomeSurfacesCountsBackupStateAndRecentNotebooks() {
        composeRule.activity.setContent {
            MaterialTheme {
                HomeScreen(
                    state =
                        HomeDashboardState(
                            hasLocalLibrary = true,
                            recentNotebooks =
                                listOf(
                                    HomeNotebookSummary(
                                        id = "notebook-a",
                                        displayName = "Field Notes",
                                        pageSummary = "3 scanned pages",
                                        needsReviewCount = 2,
                                    ),
                                ),
                            unfinishedScanCount = 1,
                            needsReviewCount = 4,
                            generatedSmartPageCount = 7,
                            backupStatus = HomeBackupStatus.AttentionNeeded,
                        ),
                    onScanPage = {},
                    onBatchScan = {},
                    onOpenNotebooks = {},
                    onCreateSmartPages = {},
                )
            }
        }

        composeRule.onNodeWithTag(HomeScreenTestTags.POPULATED_STATE).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.UNFINISHED_COUNT).assertIsDisplayed()
        composeRule.onNodeWithText("Unfinished scans: 1").assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.REVIEW_COUNT).assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review: 4").assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.SMART_PAGE_COUNT).assertIsDisplayed()
        composeRule.onNodeWithText("Generated Smart Pages: 7").assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.BACKUP_STATUS).assertIsDisplayed()
        composeRule.onNodeWithText("Backup: attention needed").assertIsDisplayed()
        composeRule.onAllNodesWithTag(HomeScreenTestTags.RECENT_NOTEBOOK)[0].assertIsDisplayed()
        composeRule.onNodeWithText("Field Notes").assertIsDisplayed()
        composeRule.onNodeWithText("Needs Review in this Notebook: 2").assertIsDisplayed()
    }
}
