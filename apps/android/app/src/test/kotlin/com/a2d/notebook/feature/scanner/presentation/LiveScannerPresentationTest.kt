package com.a2d.notebook.feature.scanner.presentation

import androidx.compose.ui.geometry.Size
import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import com.a2d.notebook.rustbridge.AnalyzedPageQuality
import com.a2d.notebook.rustbridge.EncodedPageAnalysisResult
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.NotebookDesignSummary
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.PageResolution

class LiveScannerPresentationTest {
    private val policy =
        LiveScannerGuidancePolicy(
            minimumMarkerDecisionMargin = 20.0,
            minimumFocusLaplacianVariance = 40.0,
            minimumMeanLuminance = 50.0,
            maximumHighlightFraction = 0.15,
            maximumTileHighlightFraction = 0.35,
            minimumPageCoverageFraction = 0.20,
            maximumPageCoverageFraction = 0.85,
            minimumEdgeMarginFraction = 0.03,
        )

    @Test
    fun matchingRustResolutionAllowsIdentityGateAndShowsPositiveGuidance() {
        val state =
            buildLiveScannerPresentation(
                activeNotebook = notebook(id = "notebook-a", active = true),
                pageResolution =
                    PageResolution.Resolved(
                        pageId = "page-a",
                        notebookId = "notebook-a",
                    ),
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )

        assertTrue(state.identityGate.allowed)
        assertNull(state.identityGate.blockReason)
        assertEquals(ScannerGuidanceCode.PAGE_ALIGNED, state.guidance.code)
        assertEquals(ScannerGuidanceSeverity.POSITIVE, state.guidance.severity)
        assertEquals(listOf("TL", "TR", "BR", "BL"), state.overlay?.markers?.map { it.role })
        assertFalse(requireNotNull(state.overlay).conflict)
    }

    @Test
    fun conflictingRustResolutionBlocksAutoCaptureAndOverridesQualityGuidance() {
        val active = notebook(id = "notebook-a", active = true)
        val state =
            buildLiveScannerPresentation(
                activeNotebook = active,
                pageResolution =
                    PageResolution.ConflictingActiveNotebook(
                        active = active,
                        detectedDesign = "design-b",
                    ),
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )

        assertFalse(state.identityGate.allowed)
        assertEquals(IdentityCaptureBlockReason.WRONG_NOTEBOOK, state.identityGate.blockReason)
        assertEquals(ScannerGuidanceCode.WRONG_NOTEBOOK, state.guidance.code)
        assertEquals(ScannerGuidanceSeverity.BLOCKING, state.guidance.severity)
        assertTrue(requireNotNull(state.overlay).conflict)
    }

    @Test
    fun resolvedPageForDifferentNotebookIsStillBlockedWithoutGuessing() {
        val state =
            buildLiveScannerPresentation(
                activeNotebook = notebook(id = "notebook-a", active = true),
                pageResolution =
                    PageResolution.Resolved(
                        pageId = "page-b",
                        notebookId = "notebook-b",
                    ),
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )

        assertFalse(state.identityGate.allowed)
        assertEquals(IdentityCaptureBlockReason.WRONG_NOTEBOOK, state.identityGate.blockReason)
        assertEquals(ScannerGuidanceCode.WRONG_NOTEBOOK, state.guidance.code)
    }

    @Test
    fun ambiguousAndUnregisteredNotebookStatesRemainBlocked() {
        val active = notebook(id = "notebook-a", active = true)
        val ambiguous =
            buildLiveScannerPresentation(
                activeNotebook = active,
                pageResolution =
                    PageResolution.RequiresNotebookSelection(
                        candidates = listOf(active, notebook(id = "notebook-b", active = false)),
                    ),
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )
        val unregistered =
            buildLiveScannerPresentation(
                activeNotebook = active,
                pageResolution =
                    PageResolution.RequiresNotebookRegistration(
                        design =
                            NotebookDesignSummary(
                                id = "design-b",
                                name = "Different Design",
                                designVersion = 1u,
                                logicalPageCount = 100u,
                                trusted = true,
                            ),
                    ),
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )

        assertEquals(
            IdentityCaptureBlockReason.NOTEBOOK_SELECTION_REQUIRED,
            ambiguous.identityGate.blockReason,
        )
        assertEquals(ScannerGuidanceCode.SELECT_MATCHING_NOTEBOOK, ambiguous.guidance.code)
        assertEquals(
            IdentityCaptureBlockReason.NOTEBOOK_REGISTRATION_REQUIRED,
            unregistered.identityGate.blockReason,
        )
        assertEquals(ScannerGuidanceCode.REGISTER_NOTEBOOK, unregistered.guidance.code)
    }

    @Test
    fun noActiveNotebookAndNoPageCodeAreExplicitBlockingStates() {
        val noNotebook =
            buildLiveScannerPresentation(
                activeNotebook = null,
                pageResolution = null,
                analysis = null,
                analysisFailure = null,
                policy = policy,
            )
        val unresolvedCode =
            buildLiveScannerPresentation(
                activeNotebook = notebook(id = "notebook-a", active = true),
                pageResolution = null,
                analysis = goodAnalysis(),
                analysisFailure = null,
                policy = policy,
            )

        assertEquals(IdentityCaptureBlockReason.NO_ACTIVE_NOTEBOOK, noNotebook.identityGate.blockReason)
        assertEquals(ScannerGuidanceCode.SELECT_NOTEBOOK, noNotebook.guidance.code)
        assertEquals(
            IdentityCaptureBlockReason.PAGE_CODE_UNRESOLVED,
            unresolvedCode.identityGate.blockReason,
        )
        assertEquals(ScannerGuidanceCode.FIND_PAGE_CODE, unresolvedCode.guidance.code)
    }

    @Test
    fun guidanceIsActionableForMarkerFramingFocusLightingAndGlare() {
        val active = notebook(id = "notebook-a", active = true)
        val identity = PageResolution.Resolved(pageId = "page-a", notebookId = active.id)

        fun guidance(analysis: EncodedPageAnalysisResult): ScannerGuidanceCode =
            buildLiveScannerPresentation(active, identity, analysis, null, policy).guidance.code

        assertEquals(
            ScannerGuidanceCode.SHOW_ALL_CORNERS,
            guidance(goodAnalysis().copy(markers = goodAnalysis().markers.dropLast(1))),
        )
        assertEquals(
            ScannerGuidanceCode.MOVE_CLOSER,
            guidance(goodAnalysis(boundaryInset = 420.0)),
        )
        assertEquals(
            ScannerGuidanceCode.MOVE_FARTHER,
            guidance(goodAnalysis(boundaryInset = 5.0)),
        )
        assertEquals(
            ScannerGuidanceCode.HOLD_STEADY,
            guidance(
                goodAnalysis().copy(
                    quality = goodAnalysis().quality.copy(focusLaplacianVariance = 10.0),
                ),
            ),
        )
        assertEquals(
            ScannerGuidanceCode.ADD_LIGHT,
            guidance(goodAnalysis().copy(quality = goodAnalysis().quality.copy(meanLuminance = 20.0))),
        )
        assertEquals(
            ScannerGuidanceCode.REDUCE_GLARE,
            guidance(
                goodAnalysis().copy(
                    quality = goodAnalysis().quality.copy(maxTileHighlightFraction = 0.8),
                ),
            ),
        )
    }

    @Test
    fun overlayMapperUsesFillCenterAndCameraRotation() {
        val identityMapper =
            PreviewCoordinateMapper(
                frameWidth = 100,
                frameHeight = 100,
                rotationDegrees = 0,
                previewSize = Size(200f, 100f),
            )
        val rotatedMapper =
            PreviewCoordinateMapper(
                frameWidth = 100,
                frameHeight = 200,
                rotationDegrees = 90,
                previewSize = Size(200f, 100f),
            )

        val centered = identityMapper.map(AnalyzedPagePoint(50.0, 50.0))
        assertEquals(100f, centered.x, 0.001f)
        assertEquals(50f, centered.y, 0.001f)

        val rotatedTopLeft = rotatedMapper.map(AnalyzedPagePoint(0.0, 0.0))
        assertEquals(200f, rotatedTopLeft.x, 0.001f)
        assertEquals(0f, rotatedTopLeft.y, 0.001f)
    }

    @Test
    fun invalidPresentationThresholdsAreRejected() {
        val failure =
            runCatching {
                policy.copy(
                    minimumPageCoverageFraction = 0.9,
                    maximumPageCoverageFraction = 0.8,
                )
            }.exceptionOrNull()

        assertNotNull(failure)
        assertTrue(failure is IllegalArgumentException)
    }

    private fun notebook(id: String, active: Boolean) =
        NotebookSummary(
            id = id,
            designId = "design-a",
            displayName = if (active) "Field Notes" else "Lab Notes",
            archived = false,
            active = active,
        )

    private fun goodAnalysis(boundaryInset: Double = 200.0): EncodedPageAnalysisResult {
        val max = 1000.0 - boundaryInset
        val centers =
            mapOf(
                "TL" to AnalyzedPagePoint(boundaryInset, boundaryInset),
                "TR" to AnalyzedPagePoint(max, boundaryInset),
                "BR" to AnalyzedPagePoint(max, max),
                "BL" to AnalyzedPagePoint(boundaryInset, max),
            )
        return EncodedPageAnalysisResult(
            width = 1000,
            height = 1000,
            sourceRotationDegrees = 0,
            resolvedOrientationDegrees = 0,
            markers =
                listOf("TL", "TR", "BR", "BL").mapIndexed { index, role ->
                    val center = requireNotNull(centers[role])
                    AnalyzedPageMarker(
                        role = role,
                        family = "tagStandard41h12",
                        id = index.toLong(),
                        hammingErrors = 0,
                        decisionMargin = 50.0,
                        center = center,
                        corners =
                            listOf(
                                AnalyzedPagePoint(center.x - 10.0, center.y - 10.0),
                                AnalyzedPagePoint(center.x + 10.0, center.y - 10.0),
                                AnalyzedPagePoint(center.x + 10.0, center.y + 10.0),
                                AnalyzedPagePoint(center.x - 10.0, center.y + 10.0),
                            ),
                    )
                },
            unexpectedTagIds = emptyList(),
            quality =
                AnalyzedPageQuality(
                    focusLaplacianVariance = 100.0,
                    focusInteriorSampleCount = 1000u,
                    meanLuminance = 120.0,
                    luminanceStandardDeviation = 40.0,
                    darkFraction = 0.01,
                    highlightFraction = 0.01,
                    maxTileHighlightFraction = 0.02,
                    populatedTileCount = 64,
                ),
        )
    }
}
