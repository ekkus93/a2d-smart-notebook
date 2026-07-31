package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.feature.scanner.camera.CameraAdapterState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.NotebookSummary
import uniffi.a2d_ffi.ScannerRecoveryPhase
import uniffi.a2d_ffi.ScannerRecoveryRecord

class ScannerRecoveryUiPolicyTest {
    @Test
    fun capturedAndPreviewReadyRecordsRequireReviewAndAllowExplicitDiscard() {
        listOf(ScannerRecoveryPhase.CAPTURED, ScannerRecoveryPhase.PREVIEW_READY).forEach { phase ->
            val record = recovery(phase)
            assertEquals(ScannerRecoveryPrimaryAction.REVIEW, record.primaryAction())
            assertTrue(record.canDiscard())
        }
    }

    @Test
    fun registeringMustReconcileAndCommittedMustAcknowledgeWithoutDiscard() {
        val registering = recovery(ScannerRecoveryPhase.REGISTERING)
        val committed = recovery(ScannerRecoveryPhase.COMMITTED, registeredScanId = "scan-a")

        assertEquals(ScannerRecoveryPrimaryAction.RECONCILE, registering.primaryAction())
        assertFalse(registering.canDiscard())
        assertEquals(ScannerRecoveryPrimaryAction.ACKNOWLEDGE, committed.primaryAction())
        assertFalse(committed.canDiscard())
    }

    @Test
    fun pendingRecoveryBlocksNewManualCapture() {
        val notebook =
            NotebookSummary(
                id = "notebook-a",
                designId = "design-a",
                displayName = "Field Notes",
                archived = false,
                active = true,
            )
        val state =
            SinglePageScannerUiState(
                activeNotebook = notebook,
                cameraState = CameraAdapterState.Bound(torchAvailable = true, torchEnabled = false),
                scannerRecoveries = listOf(recovery(ScannerRecoveryPhase.CAPTURED)),
                recoveryLoading = false,
            )

        assertFalse(state.canCaptureManually)
    }

    @Test
    fun stagingOwnershipBlocksNavigationUntilProcessingOrRegistrationFinishes() {
        assertFalse(SinglePageScannerUiState().navigationBlocked)
        assertTrue(SinglePageScannerUiState(processing = true).navigationBlocked)
        assertTrue(SinglePageScannerUiState(registrationInProgress = true).navigationBlocked)
        assertTrue(SinglePageScannerUiState(recoveryOperationInProgress = true).navigationBlocked)
    }

    private fun recovery(
        phase: ScannerRecoveryPhase,
        registeredScanId: String? = null,
    ): ScannerRecoveryRecord =
        ScannerRecoveryRecord(
            token = "recovery-a",
            stagingPath = "/private/tmp/scanner-staging/recovery.jpg",
            pageId = "page-a",
            notebookId = "notebook-a",
            capturedAtMs = 1L,
            layoutId = "USLETTER-LINED",
            processingPolicyVersion = 1u,
            phase = phase,
            registeredScanId = registeredScanId,
            updatedAtMs = 2L,
        )
}
