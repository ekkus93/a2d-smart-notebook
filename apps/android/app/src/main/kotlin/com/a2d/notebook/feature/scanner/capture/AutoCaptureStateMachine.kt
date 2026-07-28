package com.a2d.notebook.feature.scanner.capture

import com.a2d.notebook.feature.scanner.presentation.IdentityAutoCaptureGate

/**
 * Timing policy for the deterministic auto-capture state machine.
 *
 * All values use monotonic nanoseconds supplied by the caller. The state machine never reads wall
 * time, so tests and lifecycle recovery are deterministic.
 */
data class AutoCapturePolicy(
    val stableIntervalNanos: Long,
    val maximumInterFrameGapNanos: Long,
    val repeatDebounceNanos: Long,
) {
    init {
        require(stableIntervalNanos > 0L) { "stable interval must be positive" }
        require(maximumInterFrameGapNanos > 0L) { "maximum inter-frame gap must be positive" }
        require(repeatDebounceNanos >= 0L) { "repeat debounce must not be negative" }
    }
}

data class AutoCaptureContext(
    val activeNotebookId: String,
    val sessionId: String? = null,
) {
    init {
        require(activeNotebookId.isNotBlank()) { "active Notebook ID must not be blank" }
        require(sessionId == null || sessionId.isNotBlank()) { "session ID must not be blank" }
    }
}

/**
 * One authoritative frame assessment supplied by the scanner workflow.
 *
 * [acceptedByCapturePolicy] is intentionally separate from presentation guidance. The caller must
 * set it only from the reviewed capture-acceptance policy; the state machine does not promote UI
 * guidance thresholds into canonical capture rules. [identityGate] remains the strict Rust-derived
 * Notebook/page identity gate from Milestone 8.2B.
 */
data class AutoCaptureFrameAssessment(
    val timestampNanos: Long,
    val pageId: String?,
    val identityGate: IdentityAutoCaptureGate,
    val acceptedByCapturePolicy: Boolean,
) {
    init {
        require(timestampNanos >= 0L) { "frame timestamp must not be negative" }
        require(pageId == null || pageId.isNotBlank()) { "page ID must not be blank" }
    }

    val eligibleForAutoCapture: Boolean
        get() = identityGate.allowed && acceptedByCapturePolicy && pageId != null
}

data class CaptureRequestToken(
    val generation: Long,
    val sequence: Long,
)

enum class CaptureTrigger {
    AUTO,
    MANUAL,
}

data class AutoCaptureRequest(
    val token: CaptureRequestToken,
    val pageId: String,
    val activeNotebookId: String,
    val trigger: CaptureTrigger,
    val requestedAtNanos: Long,
)

data class CapturedImage(
    val stagingPath: String,
    val savedUri: String? = null,
) {
    init {
        require(stagingPath.isNotBlank()) { "capture staging path must not be blank" }
        require(savedUri == null || savedUri.isNotBlank()) { "saved URI must not be blank" }
    }
}

data class AutoCaptureFailure(
    val message: String,
    val retryable: Boolean,
) {
    init {
        require(message.isNotBlank()) { "capture failure message must not be blank" }
    }
}

enum class ManualCaptureWarningCode {
    BYPASSES_STABILITY_CHECK,
    CAPTURE_POLICY_NOT_ACCEPTED,
    REPEATS_RECENT_PAGE,
}

data class ManualCaptureWarningToken(
    val generation: Long,
    val sequence: Long,
)

data class ManualCaptureWarning(
    val token: ManualCaptureWarningToken,
    val pageId: String,
    val warningCodes: Set<ManualCaptureWarningCode>,
) {
    init {
        require(pageId.isNotBlank()) { "manual capture page ID must not be blank" }
        require(warningCodes.isNotEmpty()) { "manual capture requires an explicit warning" }
    }
}

enum class ManualCaptureDeniedReason {
    NOT_RUNNING,
    NO_CURRENT_FRAME,
    IDENTITY_NOT_VERIFIED,
    CAPTURE_ALREADY_ACTIVE,
    PAUSED,
}

enum class AutoCapturePauseReason {
    AWAITING_MANUAL_CONFIRMATION,
    NAVIGATION,
}

sealed interface AutoCapturePhase {
    data object Idle : AutoCapturePhase
    data object Searching : AutoCapturePhase

    data class CandidateStable(
        val pageId: String,
        val stableSinceNanos: Long,
        val lastFrameAtNanos: Long,
    ) : AutoCapturePhase

    data class Capturing(
        val request: AutoCaptureRequest,
    ) : AutoCapturePhase

    data class Processing(
        val request: AutoCaptureRequest,
        val capturedImage: CapturedImage,
    ) : AutoCapturePhase

    data class Accepted(
        val request: AutoCaptureRequest,
        val scanId: String?,
    ) : AutoCapturePhase

    data class NeedsReview(
        val request: AutoCaptureRequest,
        val reason: String,
    ) : AutoCapturePhase

    data class Rejected(
        val request: AutoCaptureRequest,
        val reason: String,
    ) : AutoCapturePhase

    data class Paused(
        val reason: AutoCapturePauseReason,
    ) : AutoCapturePhase
}

data class AutoCaptureSnapshot(
    val phase: AutoCapturePhase,
    val context: AutoCaptureContext?,
    val generation: Long,
    val latestFrame: AutoCaptureFrameAssessment?,
    val pendingManualWarning: ManualCaptureWarning?,
    val lastFailure: AutoCaptureFailure?,
)

sealed interface AutoCaptureProcessingOutcome {
    data class Accepted(
        val scanId: String? = null,
    ) : AutoCaptureProcessingOutcome

    data class NeedsReview(
        val reason: String,
    ) : AutoCaptureProcessingOutcome

    data class Rejected(
        val reason: String,
    ) : AutoCaptureProcessingOutcome
}

sealed interface AutoCaptureEffect {
    data class CaptureRequested(
        val request: AutoCaptureRequest,
    ) : AutoCaptureEffect

    data class ManualCaptureWarningRequired(
        val warning: ManualCaptureWarning,
    ) : AutoCaptureEffect

    data class ManualCaptureDenied(
        val reason: ManualCaptureDeniedReason,
    ) : AutoCaptureEffect

    data class CaptureDebounced(
        val pageId: String,
        val untilNanos: Long,
    ) : AutoCaptureEffect

    data class CancelActiveWork(
        val token: CaptureRequestToken,
    ) : AutoCaptureEffect

    data class StaleCallbackIgnored(
        val callback: String,
        val token: CaptureRequestToken,
    ) : AutoCaptureEffect
}

data class AutoCaptureTransition(
    val snapshot: AutoCaptureSnapshot,
    val effects: List<AutoCaptureEffect> = emptyList(),
)

private data class PageDebounce(
    val pageId: String,
    val untilNanos: Long,
)

/**
 * Thread-safe explicit scanner state machine for Milestone 8.3.
 *
 * The machine owns no Android objects, files, database handles, or canonical business rules. It
 * emits [AutoCaptureEffect.CaptureRequested] for a controller to execute through [CameraXAdapter]
 * and accepts tokened callbacks. Navigation increments the generation so late camera/processing
 * callbacks cannot mutate a new or paused scanner session.
 */
class AutoCaptureStateMachine(
    private val policy: AutoCapturePolicy,
) {
    private var phase: AutoCapturePhase = AutoCapturePhase.Idle
    private var context: AutoCaptureContext? = null
    private var generation = 0L
    private var requestSequence = 0L
    private var warningSequence = 0L
    private var latestFrame: AutoCaptureFrameAssessment? = null
    private var lastObservedFrameNanos: Long? = null
    private var pendingManualWarning: ManualCaptureWarning? = null
    private var lastFailure: AutoCaptureFailure? = null
    private var debounce: PageDebounce? = null

    @Synchronized
    fun snapshot(): AutoCaptureSnapshot = currentSnapshot()

    @Synchronized
    fun start(newContext: AutoCaptureContext): AutoCaptureTransition {
        generation = nextGeneration(generation)
        context = newContext
        phase = AutoCapturePhase.Searching
        latestFrame = null
        lastObservedFrameNanos = null
        pendingManualWarning = null
        lastFailure = null
        debounce = null
        return transition()
    }

    @Synchronized
    fun observeFrame(frame: AutoCaptureFrameAssessment): AutoCaptureTransition {
        if (phase !is AutoCapturePhase.Searching && phase !is AutoCapturePhase.CandidateStable) {
            return transition()
        }
        if (context == null) {
            phase = AutoCapturePhase.Idle
            return transition()
        }

        val previousTimestamp = lastObservedFrameNanos
        require(previousTimestamp == null || frame.timestampNanos >= previousTimestamp) {
            "frame timestamps must be monotonic"
        }
        lastObservedFrameNanos = frame.timestampNanos
        latestFrame = frame

        val pageId = frame.pageId
        if (!frame.eligibleForAutoCapture || pageId == null) {
            phase = AutoCapturePhase.Searching
            return transition()
        }

        activeDebounce(pageId, frame.timestampNanos)?.let { currentDebounce ->
            phase = AutoCapturePhase.Searching
            return transition(
                AutoCaptureEffect.CaptureDebounced(pageId, currentDebounce.untilNanos),
            )
        }

        val currentPhase = phase
        if (currentPhase is AutoCapturePhase.CandidateStable && currentPhase.pageId == pageId) {
            val interFrameGap = frame.timestampNanos - currentPhase.lastFrameAtNanos
            if (interFrameGap > policy.maximumInterFrameGapNanos) {
                phase = candidate(pageId, frame.timestampNanos)
                return transition()
            }

            if (frame.timestampNanos - currentPhase.stableSinceNanos >= policy.stableIntervalNanos) {
                return beginCapture(pageId, CaptureTrigger.AUTO, frame.timestampNanos)
            }

            phase = currentPhase.copy(lastFrameAtNanos = frame.timestampNanos)
            return transition()
        }

        phase = candidate(pageId, frame.timestampNanos)
        return transition()
    }

    @Synchronized
    fun requestManualCapture(nowNanos: Long): AutoCaptureTransition {
        require(nowNanos >= 0L) { "manual capture timestamp must not be negative" }
        when (phase) {
            AutoCapturePhase.Idle ->
                return transition(
                    AutoCaptureEffect.ManualCaptureDenied(ManualCaptureDeniedReason.NOT_RUNNING),
                )

            is AutoCapturePhase.Capturing,
            is AutoCapturePhase.Processing,
            is AutoCapturePhase.Accepted,
            is AutoCapturePhase.NeedsReview,
            is AutoCapturePhase.Rejected,
            ->
                return transition(
                    AutoCaptureEffect.ManualCaptureDenied(
                        ManualCaptureDeniedReason.CAPTURE_ALREADY_ACTIVE,
                    ),
                )

            is AutoCapturePhase.Paused ->
                return transition(
                    AutoCaptureEffect.ManualCaptureDenied(ManualCaptureDeniedReason.PAUSED),
                )

            AutoCapturePhase.Searching,
            is AutoCapturePhase.CandidateStable,
            -> Unit
        }

        val frame = latestFrame
            ?: return transition(
                AutoCaptureEffect.ManualCaptureDenied(ManualCaptureDeniedReason.NO_CURRENT_FRAME),
            )
        val pageId = frame.pageId
        if (!frame.identityGate.allowed || pageId == null) {
            return transition(
                AutoCaptureEffect.ManualCaptureDenied(
                    ManualCaptureDeniedReason.IDENTITY_NOT_VERIFIED,
                ),
            )
        }

        warningSequence = nextSequence(warningSequence)
        val warningCodes = buildSet {
            add(ManualCaptureWarningCode.BYPASSES_STABILITY_CHECK)
            if (!frame.acceptedByCapturePolicy) {
                add(ManualCaptureWarningCode.CAPTURE_POLICY_NOT_ACCEPTED)
            }
            if (activeDebounce(pageId, nowNanos) != null) {
                add(ManualCaptureWarningCode.REPEATS_RECENT_PAGE)
            }
        }
        val warning = ManualCaptureWarning(
            token = ManualCaptureWarningToken(generation, warningSequence),
            pageId = pageId,
            warningCodes = warningCodes,
        )
        pendingManualWarning = warning
        phase = AutoCapturePhase.Paused(AutoCapturePauseReason.AWAITING_MANUAL_CONFIRMATION)
        return transition(AutoCaptureEffect.ManualCaptureWarningRequired(warning))
    }

    @Synchronized
    fun confirmManualCapture(
        token: ManualCaptureWarningToken,
        nowNanos: Long,
    ): AutoCaptureTransition {
        require(nowNanos >= 0L) { "manual capture timestamp must not be negative" }
        val warning = pendingManualWarning
        if (
            warning == null ||
            warning.token != token ||
            token.generation != generation ||
            phase != AutoCapturePhase.Paused(AutoCapturePauseReason.AWAITING_MANUAL_CONFIRMATION)
        ) {
            return transition()
        }

        pendingManualWarning = null
        return beginCapture(warning.pageId, CaptureTrigger.MANUAL, nowNanos)
    }

    @Synchronized
    fun dismissManualCapture(token: ManualCaptureWarningToken): AutoCaptureTransition {
        val warning = pendingManualWarning
        if (
            warning != null &&
            warning.token == token &&
            token.generation == generation &&
            phase == AutoCapturePhase.Paused(AutoCapturePauseReason.AWAITING_MANUAL_CONFIRMATION)
        ) {
            pendingManualWarning = null
            phase = AutoCapturePhase.Searching
        }
        return transition()
    }

    @Synchronized
    fun captureSucceeded(
        token: CaptureRequestToken,
        capturedImage: CapturedImage,
    ): AutoCaptureTransition {
        val capturing = phase as? AutoCapturePhase.Capturing
        if (capturing == null || capturing.request.token != token || token.generation != generation) {
            return transition(AutoCaptureEffect.StaleCallbackIgnored("captureSucceeded", token))
        }

        val request = capturing.request
        debounce = PageDebounce(
            pageId = request.pageId,
            untilNanos = saturatingAdd(request.requestedAtNanos, policy.repeatDebounceNanos),
        )
        lastFailure = null
        latestFrame = null
        phase = AutoCapturePhase.Processing(request, capturedImage)
        return transition()
    }

    @Synchronized
    fun captureFailed(
        token: CaptureRequestToken,
        failure: AutoCaptureFailure,
    ): AutoCaptureTransition {
        val capturing = phase as? AutoCapturePhase.Capturing
        if (capturing == null || capturing.request.token != token || token.generation != generation) {
            return transition(AutoCaptureEffect.StaleCallbackIgnored("captureFailed", token))
        }

        lastFailure = failure
        latestFrame = null
        phase = AutoCapturePhase.Searching
        return transition()
    }

    @Synchronized
    fun processingCompleted(
        token: CaptureRequestToken,
        outcome: AutoCaptureProcessingOutcome,
    ): AutoCaptureTransition {
        val processing = phase as? AutoCapturePhase.Processing
        if (processing == null || processing.request.token != token || token.generation != generation) {
            return transition(AutoCaptureEffect.StaleCallbackIgnored("processingCompleted", token))
        }

        val request = processing.request
        phase = when (outcome) {
            is AutoCaptureProcessingOutcome.Accepted -> {
                require(outcome.scanId == null || outcome.scanId.isNotBlank()) {
                    "accepted scan ID must not be blank"
                }
                AutoCapturePhase.Accepted(request, outcome.scanId)
            }

            is AutoCaptureProcessingOutcome.NeedsReview -> {
                require(outcome.reason.isNotBlank()) { "review reason must not be blank" }
                AutoCapturePhase.NeedsReview(request, outcome.reason)
            }

            is AutoCaptureProcessingOutcome.Rejected -> {
                require(outcome.reason.isNotBlank()) { "rejection reason must not be blank" }
                AutoCapturePhase.Rejected(request, outcome.reason)
            }
        }
        return transition()
    }

    @Synchronized
    fun continueScanning(
        nowNanos: Long,
        allowImmediateSamePageRetake: Boolean = false,
    ): AutoCaptureTransition {
        require(nowNanos >= 0L) { "continue timestamp must not be negative" }
        val terminalRequest = when (val current = phase) {
            is AutoCapturePhase.Accepted -> current.request
            is AutoCapturePhase.NeedsReview -> current.request
            is AutoCapturePhase.Rejected -> current.request
            else -> return transition()
        }
        if (allowImmediateSamePageRetake && debounce?.pageId == terminalRequest.pageId) {
            debounce = null
        } else {
            debounce = debounce?.takeIf { nowNanos < it.untilNanos }
        }
        latestFrame = null
        lastFailure = null
        phase = AutoCapturePhase.Searching
        return transition()
    }

    @Synchronized
    fun pauseForNavigation(): AutoCaptureTransition {
        val activeToken = when (val current = phase) {
            is AutoCapturePhase.Capturing -> current.request.token
            is AutoCapturePhase.Processing -> current.request.token
            else -> null
        }
        generation = nextGeneration(generation)
        latestFrame = null
        pendingManualWarning = null
        phase = if (context == null) {
            AutoCapturePhase.Idle
        } else {
            AutoCapturePhase.Paused(AutoCapturePauseReason.NAVIGATION)
        }
        return if (activeToken == null) {
            transition()
        } else {
            transition(AutoCaptureEffect.CancelActiveWork(activeToken))
        }
    }

    @Synchronized
    fun resumeAfterNavigation(): AutoCaptureTransition {
        if (phase == AutoCapturePhase.Paused(AutoCapturePauseReason.NAVIGATION) && context != null) {
            latestFrame = null
            lastObservedFrameNanos = null
            phase = AutoCapturePhase.Searching
        }
        return transition()
    }

    @Synchronized
    fun stop(): AutoCaptureTransition {
        val activeToken = when (val current = phase) {
            is AutoCapturePhase.Capturing -> current.request.token
            is AutoCapturePhase.Processing -> current.request.token
            else -> null
        }
        generation = nextGeneration(generation)
        phase = AutoCapturePhase.Idle
        context = null
        latestFrame = null
        lastObservedFrameNanos = null
        pendingManualWarning = null
        lastFailure = null
        debounce = null
        return if (activeToken == null) {
            transition()
        } else {
            transition(AutoCaptureEffect.CancelActiveWork(activeToken))
        }
    }

    private fun beginCapture(
        pageId: String,
        trigger: CaptureTrigger,
        requestedAtNanos: Long,
    ): AutoCaptureTransition {
        val activeContext = requireNotNull(context) { "capture cannot start without scanner context" }
        requestSequence = nextSequence(requestSequence)
        val request = AutoCaptureRequest(
            token = CaptureRequestToken(generation, requestSequence),
            pageId = pageId,
            activeNotebookId = activeContext.activeNotebookId,
            trigger = trigger,
            requestedAtNanos = requestedAtNanos,
        )
        pendingManualWarning = null
        phase = AutoCapturePhase.Capturing(request)
        return transition(AutoCaptureEffect.CaptureRequested(request))
    }

    private fun candidate(pageId: String, timestampNanos: Long): AutoCapturePhase.CandidateStable =
        AutoCapturePhase.CandidateStable(
            pageId = pageId,
            stableSinceNanos = timestampNanos,
            lastFrameAtNanos = timestampNanos,
        )

    private fun activeDebounce(pageId: String, nowNanos: Long): PageDebounce? {
        val current = debounce ?: return null
        if (nowNanos >= current.untilNanos) {
            debounce = null
            return null
        }
        return current.takeIf { it.pageId == pageId }
    }

    private fun transition(vararg effects: AutoCaptureEffect): AutoCaptureTransition =
        AutoCaptureTransition(currentSnapshot(), effects.toList())

    private fun currentSnapshot(): AutoCaptureSnapshot =
        AutoCaptureSnapshot(
            phase = phase,
            context = context,
            generation = generation,
            latestFrame = latestFrame,
            pendingManualWarning = pendingManualWarning,
            lastFailure = lastFailure,
        )
}

private fun nextGeneration(value: Long): Long =
    if (value == Long.MAX_VALUE) 1L else value + 1L

private fun nextSequence(value: Long): Long =
    if (value == Long.MAX_VALUE) 1L else value + 1L

private fun saturatingAdd(value: Long, duration: Long): Long =
    if (duration > Long.MAX_VALUE - value) Long.MAX_VALUE else value + duration
