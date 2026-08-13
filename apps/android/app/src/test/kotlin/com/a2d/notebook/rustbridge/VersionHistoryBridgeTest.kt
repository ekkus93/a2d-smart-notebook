package com.a2d.notebook.rustbridge

import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.a2d_ffi.PageVersionRecord
import uniffi.a2d_ffi.PageVersionTimeline
import uniffi.a2d_ffi.StoredScanQualityStatus

class VersionHistoryBridgeTest {
    private fun version(
        id: String,
        preferred: Boolean,
    ) =
        PageVersionRecord(
            scanId = id,
            capturedAtMs = 100L,
            preferred = preferred,
            physicalCopyId = null,
            supersedesScanId = null,
            qualityStatus = StoredScanQualityStatus.NEEDS_REVIEW,
            pipelineVersion = "pipeline-v1",
            decisionCode = null,
            originalAssetPath = "/library/assets/$id.jpg",
            correctedAssetPath = null,
            thumbnailAssetPath = null,
        )

    @Test
    fun paginatedTimelineMayProjectPreferredVersionOutsideCurrentItems() {
        val preferred = version("preferred", preferred = true)
        val timeline =
            PageVersionTimeline(
                pageId = "page",
                preferredScanId = "preferred",
                preferredVersion = preferred,
                items = listOf(version("newer", preferred = false)),
                hasMore = true,
                nextOffset = 1u,
            )

        requireVersionTimelineContract("page", timeline)
    }

    @Test
    fun timelineFailsClosedWhenPreferredPointerAndRecordDisagree() {
        val timeline =
            PageVersionTimeline(
                pageId = "page",
                preferredScanId = "preferred",
                preferredVersion = version("different", preferred = true),
                items = emptyList(),
                hasMore = false,
                nextOffset = null,
            )

        assertThrows(IllegalStateException::class.java) {
            requireVersionTimelineContract("page", timeline)
        }
    }
}
