package com.a2d.notebook.feature.scanner.capture

import com.a2d.notebook.feature.scanner.presentation.IdentityAutoCaptureGate
import com.a2d.notebook.feature.scanner.presentation.IdentityCaptureBlockReason
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AutoCaptureStateMachineTest {
    private val policy =
        AutoCapturePolicy(
            stableIntervalNanos = 1_000_000_000L,
            maximumInterFrameGapNanos = 600_000_000L,
            repeatDebounceNanos = 5_000_000_000L,
        )

    @Test
    fun acceptableFramesMustRemainContinuousForConfiguredInterval() {
        val machine = startedMachine()

        val first = machine.observeFrame(frame(timestampNanos = 0L))
        assertTrue(first.snapshot.phase is AutoCapturePhase.CandidateStable)
        assertTrue(first.effects.isEmpty())

        val middle = machine.observeFrame(frame(timestampNanos = 500_000_000L))
        assertTrue(middle.snapshot.phase is AutoCapturePhase.CandidateStable)
        assertTrue(middle.effects.isEmpty())

        val stable = machine.observeFrame(frame(timestampNanos = 1_000_000_000L))
        val request = stable.captureRequest()
        assertEquals(CaptureTrigger.AUTO, request.trigger)
        assertEquals("page-a", request.pageId)
        assertEquals("notebook-a", request.activeNotebookId)
        assertTrue(stable.snapshot.phase is AutoCapturePhase.Capturing)
    }

    @Test
    fun candidateRestartsAfterFrameGapPageChangeOrUnacceptableFrame() {
        val machine = startedMachine()
        machine.observeFrame(frame(timestampNanos = 0L))

        val afterGap = machine.observeFrame(frame(timestampNanos = 700_000_000L))
        val restarted = afterGap.snapshot.phase as AutoCapturePhase.CandidateStable
        assertEquals(700_000_000L, restarted.stableSinceNanos)

        val pageChanged = machine.observeFrame(frame(timestampNanos = 900_000_000L, pageId = "page-b"))
        val changed = pageChanged.snapshot.phase as AutoCapturePhase.CandidateStable
        assertEquals("page-b", changed.pageId)
        assertEquals(900_000_000L, changed.stableSinceNanos)

        val unacceptable =
            machine.observeFrame(
                frame(
                    timestampNanos = 1_000_000_000L,
                    pageId = "page-b",
                    accepted = false,
                ),
            )
        assertEquals(AutoCapturePhase.Searching, unacceptable.snapshot.phase)

        val restartedAgain =
            machine.observeFrame(frame(timestampNanos = 1_100_000_000L, pageId = "page-b"))
        val candidate = restartedAgain.snapshot.phase as AutoCapturePhase.CandidateStable
        assertEquals(1_100_000_000L, candidate.stableSinceNanos)
    }

    @Test
    fun successfulCaptureDebouncesTheSamePageButNotADifferentPage() {
        val machine = startedMachine()
        val firstRequest = triggerAutoCapture(machine)
        machine.captureSucceeded(firstRequest.token, CapturedImage("/staging/page-a.jpg"))
        machine.processingCompleted(
            firstRequest.token,
            AutoCaptureProcessingOutcome.Accepted(scanId = "scan-a"),
        )
        machine.continueScanning(nowNanos = 1_100_000_000L)

        val repeated = machine.observeFrame(frame(timestampNanos = 1_200_000_000L))
        assertEquals(AutoCapturePhase.Searching, repeated.snapshot.phase)
        val debounced = repeated.effects.filterIsInstance<AutoCaptureEffect.CaptureDebounced>().single()
        assertEquals("page-a", debounced.pageId)
        assertEquals(6_000_000_000L, debounced.untilNanos)

        val differentFirst =
            machine.observeFrame(frame(timestampNanos = 1_300_000_000L, pageId = "page-b"))
        assertTrue(differentFirst.snapshot.phase is AutoCapturePhase.CandidateStable)
        machine.observeFrame(frame(timestampNanos = 1_800_000_000L, pageId = "page-b"))
        val differentStable =
            machine.observeFrame(frame(timestampNanos = 2_300_000_000L, pageId = "page-b"))
        assertEquals("page-b", differentStable.captureRequest().pageId)
    }

    @Test
    fun explicitRetakeCanClearSamePageDebounce() {
        val machine = startedMachine()
        val request = triggerAutoCapture(machine)
        machine.captureSucceeded(request.token, CapturedImage("/staging/page-a.jpg"))
        machine.processingCompleted(
            request.token,
            AutoCaptureProcessingOutcome.Rejected("blurred final capture"),
        )
        machine.continueScanning(
            nowNanos = 1_100_000_000L,
            allowImmediateSamePageRetake = true,
        )

        machine.observeFrame(frame(timestampNanos = 1_200_000_000L))
        machine.observeFrame(frame(timestampNanos = 1_700_000_000L))
        val retake = machine.observeFrame(frame(timestampNanos = 2_200_000_000L))
        assertEquals(CaptureTrigger.AUTO, retake.captureRequest().trigger)
    }

    @Test
    fun manualCaptureRequiresWarningAndConfirmationButCannotOverrideIdentity() {
        val machine = startedMachine()
        machine.observeFrame(frame(timestampNanos = 0L, accepted = false))

        val requested = machine.requestManualCapture(nowNanos = 100L)
        assertEquals(
            AutoCapturePhase.Paused(AutoCapturePauseReason.AWAITING_MANUAL_CONFIRMATION),
            requested.snapshot.phase,
        )
        val warning =
            requested.effects
                .filterIsInstance<AutoCaptureEffect.ManualCaptureWarningRequired>()
                .single()
                .warning
        assertTrue(
            ManualCaptureWarningCode.BYPASSES_STABILITY_CHECK in warning.warningCodes,
        )
        assertTrue(
            ManualCaptureWarningCode.CAPTURE_POLICY_NOT_ACCEPTED in warning.warningCodes,
        )
        assertTrue(requested.snapshot.pendingManualWarning == warning)

        val confirmed = machine.confirmManualCapture(warning.token, nowNanos = 200L)
        val manualRequest = confirmed.captureRequest()
        assertEquals(CaptureTrigger.MANUAL, manualRequest.trigger)
        assertNull(confirmed.snapshot.pendingManualWarning)

        val conflictingMachine = startedMachine()
        conflictingMachine.observeFrame(
            frame(
                timestampNanos = 0L,
                identityGate = blockedIdentity(),
            ),
        )
        val denied = conflictingMachine.requestManualCapture(nowNanos = 1L)
        assertEquals(AutoCapturePhase.Searching, denied.snapshot.phase)
        assertEquals(
            ManualCaptureDeniedReason.IDENTITY_NOT_VERIFIED,
            denied.effects.filterIsInstance<AutoCaptureEffect.ManualCaptureDenied>().single().reason,
        )
    }

    @Test
    fun dismissingManualWarningReturnsToSearchingWithoutCapturing() {
        val machine = startedMachine()
        machine.observeFrame(frame(timestampNanos = 0L, accepted = false))
        val warning =
            machine.requestManualCapture(nowNanos = 1L)
                .effects
                .filterIsInstance<AutoCaptureEffect.ManualCaptureWarningRequired>()
                .single()
                .warning

        val dismissed = machine.dismissManualCapture(warning.token)
        assertEquals(AutoCapturePhase.Searching, dismissed.snapshot.phase)
        assertNull(dismissed.snapshot.pendingManualWarning)
        assertTrue(dismissed.effects.isEmpty())
    }

    @Test
    fun navigationPauseInvalidatesLateCaptureCallbacksAndPreservesContext() {
        val machine = startedMachine()
        val request = triggerAutoCapture(machine)

        val paused = machine.pauseForNavigation()
        assertEquals(
            AutoCapturePhase.Paused(AutoCapturePauseReason.NAVIGATION),
            paused.snapshot.phase,
        )
        assertEquals("notebook-a", paused.snapshot.context?.activeNotebookId)
        assertEquals(
            request.token,
            paused.effects.filterIsInstance<AutoCaptureEffect.CancelActiveWork>().single().token,
        )

        val late =
            machine.captureSucceeded(
                request.token,
                CapturedImage("/staging/late.jpg"),
            )
        assertEquals(
            AutoCapturePhase.Paused(AutoCapturePauseReason.NAVIGATION),
            late.snapshot.phase,
        )
        assertEquals(
            "captureSucceeded",
            late.effects.filterIsInstance<AutoCaptureEffect.StaleCallbackIgnored>().single().callback,
        )

        val resumed = machine.resumeAfterNavigation()
        assertEquals(AutoCapturePhase.Searching, resumed.snapshot.phase)
        assertEquals("notebook-a", resumed.snapshot.context?.activeNotebookId)
        assertTrue(resumed.snapshot.generation > request.token.generation)
    }

    @Test
    fun captureFailureReturnsToSearchingWithExplicitFailureAndSameContext() {
        val machine = startedMachine()
        val firstRequest = triggerAutoCapture(machine)

        val failed =
            machine.captureFailed(
                firstRequest.token,
                AutoCaptureFailure("camera disconnected", retryable = true),
            )
        assertEquals(AutoCapturePhase.Searching, failed.snapshot.phase)
        assertEquals("notebook-a", failed.snapshot.context?.activeNotebookId)
        assertEquals("camera disconnected", failed.snapshot.lastFailure?.message)
        assertNull(failed.snapshot.latestFrame)

        machine.observeFrame(frame(timestampNanos = 2_000_000_000L))
        machine.observeFrame(frame(timestampNanos = 2_500_000_000L))
        val retried = machine.observeFrame(frame(timestampNanos = 3_000_000_000L))
        val secondRequest = retried.captureRequest()
        assertNotEquals(firstRequest.token, secondRequest.token)
        assertEquals("notebook-a", secondRequest.activeNotebookId)
    }

    @Test
    fun processingOutcomesUseExplicitTerminalStates() {
        fun processedMachine(): Pair<AutoCaptureStateMachine, AutoCaptureRequest> {
            val machine = startedMachine()
            val request = triggerAutoCapture(machine)
            machine.captureSucceeded(request.token, CapturedImage("/staging/page.jpg"))
            return machine to request
        }

        val (acceptedMachine, acceptedRequest) = processedMachine()
        val accepted =
            acceptedMachine.processingCompleted(
                acceptedRequest.token,
                AutoCaptureProcessingOutcome.Accepted("scan-a"),
            )
        assertEquals(
            AutoCapturePhase.Accepted(acceptedRequest, "scan-a"),
            accepted.snapshot.phase,
        )

        val (reviewMachine, reviewRequest) = processedMachine()
        val review =
            reviewMachine.processingCompleted(
                reviewRequest.token,
                AutoCaptureProcessingOutcome.NeedsReview("identity ambiguity"),
            )
        assertEquals(
            AutoCapturePhase.NeedsReview(reviewRequest, "identity ambiguity"),
            review.snapshot.phase,
        )

        val (rejectedMachine, rejectedRequest) = processedMachine()
        val rejected =
            rejectedMachine.processingCompleted(
                rejectedRequest.token,
                AutoCaptureProcessingOutcome.Rejected("unsupported layout"),
            )
        assertEquals(
            AutoCapturePhase.Rejected(rejectedRequest, "unsupported layout"),
            rejected.snapshot.phase,
        )
    }

    @Test
    fun durableReviewRegistrationTransitionsToAcceptedOnlyForMatchingToken() {
        val machine = startedMachine()
        val request = triggerAutoCapture(machine)
        machine.captureSucceeded(request.token, CapturedImage("/staging/page.jpg"))
        machine.processingCompleted(
            request.token,
            AutoCaptureProcessingOutcome.NeedsReview("explicit user approval required"),
        )

        val accepted = machine.reviewRegistrationCompleted(request.token, "scan-durable")
        assertEquals(
            AutoCapturePhase.Accepted(request, "scan-durable"),
            accepted.snapshot.phase,
        )

        val stale = machine.reviewRegistrationCompleted(request.token, "scan-second")
        assertEquals(
            AutoCapturePhase.Accepted(request, "scan-durable"),
            stale.snapshot.phase,
        )
        assertEquals(
            "reviewRegistrationCompleted",
            stale.effects.filterIsInstance<AutoCaptureEffect.StaleCallbackIgnored>().single().callback,
        )
    }

    @Test
    fun staleProcessingCallbackCannotMutateNewGeneration() {
        val machine = startedMachine()
        val request = triggerAutoCapture(machine)
        machine.captureSucceeded(request.token, CapturedImage("/staging/page.jpg"))
        machine.pauseForNavigation()

        val stale =
            machine.processingCompleted(
                request.token,
                AutoCaptureProcessingOutcome.Accepted("scan-stale"),
            )
        assertEquals(
            AutoCapturePhase.Paused(AutoCapturePauseReason.NAVIGATION),
            stale.snapshot.phase,
        )
        assertEquals(
            "processingCompleted",
            stale.effects.filterIsInstance<AutoCaptureEffect.StaleCallbackIgnored>().single().callback,
        )
    }

    @Test
    fun stopClearsContextAndReturnsIdle() {
        val machine = startedMachine()
        val request = triggerAutoCapture(machine)

        val stopped = machine.stop()
        assertEquals(AutoCapturePhase.Idle, stopped.snapshot.phase)
        assertNull(stopped.snapshot.context)
        assertNull(stopped.snapshot.latestFrame)
        assertNotNull(
            stopped.effects.filterIsInstance<AutoCaptureEffect.CancelActiveWork>().singleOrNull(),
        )

        val late =
            machine.captureFailed(
                request.token,
                AutoCaptureFailure("late", retryable = false),
            )
        assertEquals(AutoCapturePhase.Idle, late.snapshot.phase)
        assertTrue(late.effects.single() is AutoCaptureEffect.StaleCallbackIgnored)
    }

    @Test
    fun invalidPolicyAndNonMonotonicFramesAreRejected() {
        val invalidPolicy =
            runCatching {
                AutoCapturePolicy(
                    stableIntervalNanos = 0L,
                    maximumInterFrameGapNanos = 1L,
                    repeatDebounceNanos = 0L,
                )
            }.exceptionOrNull()
        assertTrue(invalidPolicy is IllegalArgumentException)

        val machine = startedMachine()
        machine.observeFrame(frame(timestampNanos = 10L))
        val nonMonotonic =
            runCatching { machine.observeFrame(frame(timestampNanos = 9L)) }.exceptionOrNull()
        assertTrue(nonMonotonic is IllegalArgumentException)
    }

    private fun startedMachine(): AutoCaptureStateMachine =
        AutoCaptureStateMachine(policy).also {
            val started = it.start(AutoCaptureContext(activeNotebookId = "notebook-a"))
            assertEquals(AutoCapturePhase.Searching, started.snapshot.phase)
        }

    private fun triggerAutoCapture(
        machine: AutoCaptureStateMachine,
        pageId: String = "page-a",
    ): AutoCaptureRequest {
        machine.observeFrame(frame(timestampNanos = 0L, pageId = pageId))
        machine.observeFrame(frame(timestampNanos = 500_000_000L, pageId = pageId))
        return machine
            .observeFrame(frame(timestampNanos = 1_000_000_000L, pageId = pageId))
            .captureRequest()
    }

    private fun frame(
        timestampNanos: Long,
        pageId: String = "page-a",
        accepted: Boolean = true,
        identityGate: IdentityAutoCaptureGate = allowedIdentity(),
    ) =
        AutoCaptureFrameAssessment(
            timestampNanos = timestampNanos,
            pageId = pageId,
            identityGate = identityGate,
            acceptedByCapturePolicy = accepted,
        )

    private fun allowedIdentity() = IdentityAutoCaptureGate(allowed = true, blockReason = null)

    private fun blockedIdentity() =
        IdentityAutoCaptureGate(
            allowed = false,
            blockReason = IdentityCaptureBlockReason.WRONG_NOTEBOOK,
        )

    private fun AutoCaptureTransition.captureRequest(): AutoCaptureRequest =
        effects.filterIsInstance<AutoCaptureEffect.CaptureRequested>().single().request
}
