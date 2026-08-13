package com.a2d.notebook.feature.version

import androidx.activity.compose.setContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToIndex
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.app.MainActivity
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.PageVersionComparison
import uniffi.a2d_ffi.PageVersionRecord
import uniffi.a2d_ffi.PageVersionTimeline
import uniffi.a2d_ffi.ScanRevisionDecision
import uniffi.a2d_ffi.ScanRevisionProposal
import uniffi.a2d_ffi.StoredScanChangeRegion
import uniffi.a2d_ffi.StoredScanChangedCell
import uniffi.a2d_ffi.StoredScanComparisonConfidence
import uniffi.a2d_ffi.StoredScanComparisonEvidence
import uniffi.a2d_ffi.StoredScanComparisonReason
import uniffi.a2d_ffi.StoredScanQualityStatus

@RunWith(AndroidJUnit4::class)
class VersionHistoryUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun timelineComparisonAndRustAuthorizedActionsAreVisible() {
        composeRule.activity.setContent {
            MaterialTheme {
                VersionHistoryContent(
                    pageId = PAGE_ID,
                    state = unresolvedState(),
                    onBack = {},
                    onSelectVersion = {},
                    onLoadMore = {},
                    onDecision = { _, _ -> },
                    onMoveToReview = {},
                )
            }
        }

        composeRule.onNodeWithTag(VersionHistoryTestTags.TIMELINE).assertIsDisplayed()

        assertTimelineNodeIsDisplayed(PREFERRED_ITEM_INDEX, VersionHistoryTestTags.PREFERRED)
        assertTimelineNodeIsDisplayed(COMPARISON_ITEM_INDEX, VersionHistoryTestTags.COMPARISON)
        assertTimelineNodeIsDisplayed(COMPARISON_ITEM_INDEX, VersionHistoryTestTags.CHANGED_REGIONS)
        assertTimelineNodeIsDisplayed(ACTIONS_ITEM_INDEX, VersionHistoryTestTags.KEEP_BOTH)
        assertTimelineNodeIsDisplayed(ACTIONS_ITEM_INDEX, VersionHistoryTestTags.SET_PREFERRED)
        assertTimelineNodeIsDisplayed(ACTIONS_ITEM_INDEX, VersionHistoryTestTags.PHYSICAL_COPY)
        assertTimelineNodeIsDisplayed(ACTIONS_ITEM_INDEX, VersionHistoryTestTags.WRONG_SCAN)
        assertTimelineNodeIsDisplayed(ACTIONS_ITEM_INDEX, VersionHistoryTestTags.MOVE_TO_REVIEW)
    }

    @Test
    fun terminalDecisionRemainsComparableWithoutOfferingAnotherDecision() {
        val base = unresolvedState()
        val timeline = requireNotNull(base.timeline)
        val decidedCandidate = timeline.items[1].copy(decisionCode = "WRONG_SCAN")
        composeRule.activity.setContent {
            MaterialTheme {
                VersionHistoryContent(
                    pageId = PAGE_ID,
                    state =
                        base.copy(
                            timeline =
                                timeline.copy(
                                    items = listOf(timeline.items[0], decidedCandidate),
                                ),
                            proposal = null,
                        ),
                    onBack = {},
                    onSelectVersion = {},
                    onLoadMore = {},
                    onDecision = { _, _ -> },
                    onMoveToReview = {},
                )
            }
        }

        assertTimelineNodeIsDisplayed(COMPARISON_ITEM_INDEX, VersionHistoryTestTags.COMPARISON)

        scrollTimelineTo(ACTIONS_ITEM_INDEX)
        composeRule.onAllNodesWithTag(VersionHistoryTestTags.KEEP_BOTH).assertCountEquals(0)
        composeRule.onAllNodesWithTag(VersionHistoryTestTags.SET_PREFERRED).assertCountEquals(0)
        composeRule.onAllNodesWithTag(VersionHistoryTestTags.PHYSICAL_COPY).assertCountEquals(0)
        composeRule.onAllNodesWithTag(VersionHistoryTestTags.WRONG_SCAN).assertCountEquals(0)
        composeRule.onAllNodesWithTag(VersionHistoryTestTags.MOVE_TO_REVIEW).assertCountEquals(0)
    }

    private fun assertTimelineNodeIsDisplayed(index: Int, tag: String) {
        scrollTimelineTo(index)
        composeRule
            .onNodeWithTag(tag, useUnmergedTree = true)
            .performScrollTo()
            .assertIsDisplayed()
    }

    private fun scrollTimelineTo(index: Int) {
        composeRule.onNodeWithTag(VersionHistoryTestTags.TIMELINE).performScrollToIndex(index)
    }

    private fun unresolvedState(): VersionHistoryUiState {
        val preferred = version(PREFERRED_SCAN_ID, preferred = true)
        val candidate = version(CANDIDATE_SCAN_ID, preferred = false)
        val evidence =
            StoredScanComparisonEvidence(
                baselineScanId = PREFERRED_SCAN_ID,
                candidateScanId = CANDIDATE_SCAN_ID,
                pageId = PAGE_ID,
                baselinePipelineVersion = "pipeline-v1",
                candidatePipelineVersion = "pipeline-v1",
                pipelineVersionsMatch = true,
                baselineQualityStatus = StoredScanQualityStatus.NEEDS_REVIEW,
                candidateQualityStatus = StoredScanQualityStatus.NEEDS_REVIEW,
                baselinePreferred = true,
                candidatePreferred = false,
                baselinePhysicalCopyId = null,
                candidatePhysicalCopyId = null,
                samePhysicalCopy = null,
                minimumCellAbsoluteDifference = 16u,
                correctedAssetHashMatch = false,
                exactContentMatch = false,
                confidence = StoredScanComparisonConfidence.UNAVAILABLE_UNTIL_FIXTURE_CALIBRATION,
                reasons = listOf(StoredScanComparisonReason.PERCEPTUAL_CHANGE_REGIONS_DETECTED),
                meanAbsoluteDifference = 22.0,
                maximumAbsoluteDifference = 41u,
                changedCellCount = 2u,
                changeRegions =
                    listOf(
                        StoredScanChangeRegion(
                            leftColumn = 2u,
                            topRow = 3u,
                            rightColumnExclusive = 4u,
                            bottomRowExclusive = 5u,
                            changedCellCount = 2u,
                            meanAbsoluteDifference = 22.0,
                            maximumAbsoluteDifference = 41u,
                            cells =
                                listOf(
                                    StoredScanChangedCell(2u, 3u, 21u),
                                    StoredScanChangedCell(3u, 4u, 23u),
                                ),
                        ),
                    ),
            )
        return VersionHistoryUiState(
            timeline =
                PageVersionTimeline(
                    pageId = PAGE_ID,
                    preferredScanId = PREFERRED_SCAN_ID,
                    preferredVersion = preferred,
                    items = listOf(preferred, candidate),
                    hasMore = false,
                    nextOffset = null,
                ),
            selectedScanId = CANDIDATE_SCAN_ID,
            comparison = PageVersionComparison(16u, 24u, evidence),
            proposal =
                ScanRevisionProposal(
                    pageId = PAGE_ID,
                    baselineScanId = PREFERRED_SCAN_ID,
                    candidateScanId = CANDIDATE_SCAN_ID,
                    defaultDecision = ScanRevisionDecision.SAVE_AS_NEW_VERSION,
                    allowedDecisions =
                        listOf(
                            ScanRevisionDecision.SAVE_AS_NEW_VERSION,
                            ScanRevisionDecision.REPLACE_PREFERRED,
                            ScanRevisionDecision.ANOTHER_PHYSICAL_COPY,
                            ScanRevisionDecision.WRONG_SCAN,
                        ),
                    comparison = evidence,
                ),
        )
    }

    private fun version(scanId: String, preferred: Boolean) =
        PageVersionRecord(
            scanId = scanId,
            capturedAtMs = 1_700_000_000_000L,
            preferred = preferred,
            physicalCopyId = null,
            supersedesScanId = null,
            qualityStatus = StoredScanQualityStatus.NEEDS_REVIEW,
            pipelineVersion = "pipeline-v1",
            decisionCode = null,
            originalAssetPath = "/missing/$scanId.jpg",
            correctedAssetPath = null,
            thumbnailAssetPath = null,
        )

    companion object {
        private const val PAGE_ID = "PAGE-TEST"
        private const val PREFERRED_SCAN_ID = "SCAN-PREFERRED"
        private const val CANDIDATE_SCAN_ID = "SCAN-CANDIDATE"
        private const val PREFERRED_ITEM_INDEX = 1
        private const val COMPARISON_ITEM_INDEX = 4
        private const val ACTIONS_ITEM_INDEX = 5
    }
}
