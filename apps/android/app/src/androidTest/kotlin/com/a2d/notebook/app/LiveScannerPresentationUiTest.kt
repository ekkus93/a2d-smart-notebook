package com.a2d.notebook.app

import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertDoesNotExist
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.scanner.presentation.IdentityAutoCaptureGate
import com.a2d.notebook.feature.scanner.presentation.IdentityCaptureBlockReason
import com.a2d.notebook.feature.scanner.presentation.LiveScannerChrome
import com.a2d.notebook.feature.scanner.presentation.LiveScannerPresentationState
import com.a2d.notebook.feature.scanner.presentation.LiveScannerTestTags
import com.a2d.notebook.feature.scanner.presentation.ScannerGuidance
import com.a2d.notebook.feature.scanner.presentation.ScannerGuidanceCode
import com.a2d.notebook.feature.scanner.presentation.ScannerGuidanceSeverity
import com.a2d.notebook.feature.scanner.singlepage.CalibrationSummary
import com.a2d.notebook.feature.scanner.singlepage.SinglePageScannerTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.NotebookSummary

@RunWith(AndroidJUnit4::class)
class LiveScannerPresentationUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun activeNotebookAndIdentityConflictAreProminentAndExplicit() {
        composeRule.activity.setContent {
            MaterialTheme {
                LiveScannerChrome(
                    state =
                        LiveScannerPresentationState(
                            activeNotebook =
                                NotebookSummary(
                                    id = "notebook-a",
                                    designId = "design-a",
                                    displayName = "Field Notes",
                                    archived = false,
                                    active = true,
                                ),
                            guidance =
                                ScannerGuidance(
                                    code = ScannerGuidanceCode.WRONG_NOTEBOOK,
                                    severity = ScannerGuidanceSeverity.BLOCKING,
                                ),
                            identityGate =
                                IdentityAutoCaptureGate(
                                    allowed = false,
                                    blockReason = IdentityCaptureBlockReason.WRONG_NOTEBOOK,
                                ),
                            overlay = null,
                        ),
                    modifier = Modifier.fillMaxSize(),
                    preview = { Box(Modifier.fillMaxSize()) },
                )
            }
        }

        composeRule.onNodeWithTag(LiveScannerTestTags.ACTIVE_NOTEBOOK).assertIsDisplayed()
        composeRule.onNodeWithText("Field Notes").fetchSemanticsNode()
        composeRule.onNodeWithTag(LiveScannerTestTags.GUIDANCE).assertIsDisplayed()
        composeRule.onNodeWithText("Wrong Notebook", substring = true).fetchSemanticsNode()
        composeRule.onNodeWithTag(LiveScannerTestTags.IDENTITY_GATE).assertIsDisplayed()
        composeRule.onNodeWithText("Auto-capture blocked", substring = true).fetchSemanticsNode()
    }

    @Test
    fun verifiedIdentityIsShownWithoutChangingTheDestination() {
        composeRule.activity.setContent {
            MaterialTheme {
                LiveScannerChrome(
                    state =
                        LiveScannerPresentationState(
                            activeNotebook =
                                NotebookSummary(
                                    id = "notebook-a",
                                    designId = "design-a",
                                    displayName = "Field Notes",
                                    archived = false,
                                    active = true,
                                ),
                            guidance =
                                ScannerGuidance(
                                    code = ScannerGuidanceCode.PAGE_ALIGNED,
                                    severity = ScannerGuidanceSeverity.POSITIVE,
                                ),
                            identityGate = IdentityAutoCaptureGate(allowed = true, blockReason = null),
                            overlay = null,
                        ),
                    modifier = Modifier.fillMaxSize(),
                    preview = { Box(Modifier.fillMaxSize()) },
                )
            }
        }

        composeRule.onNodeWithTag(LiveScannerTestTags.ACTIVE_NOTEBOOK).assertIsDisplayed()
        composeRule.onNodeWithText("Field Notes").fetchSemanticsNode()
        composeRule.onNodeWithTag(LiveScannerTestTags.IDENTITY_GATE).assertIsDisplayed()
        composeRule.onNodeWithText("Page identity matches", substring = true).fetchSemanticsNode()
    }

    @Test
    fun provisionalQualityIsNeverPresentedAsCalibratedProductionAcceptance() {
        composeRule.activity.setContent {
            MaterialTheme {
                CalibrationSummary()
            }
        }

        composeRule
            .onNodeWithTag(SinglePageScannerTestTags.QUALITY_CALIBRATION)
            .assertIsDisplayed()
        composeRule.onNodeWithText("PROVISIONAL", substring = true).assertIsDisplayed()
        composeRule
            .onNodeWithText("SYNTHETIC_FIXTURE_REGRESSION", substring = true)
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("QUALITY_THRESHOLDS_UNCALIBRATED", substring = true)
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("Production quality classification is unavailable", substring = true)
            .assertIsDisplayed()
        composeRule
            .onNodeWithText("Quality calibration: CALIBRATED", substring = true)
            .assertDoesNotExist()
    }
}
