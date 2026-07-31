package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.rustbridge.StoredScanPolicy
import java.util.concurrent.atomic.AtomicReference

internal object RustScannerPolicySession {
    private val current = AtomicReference<StoredScanPolicy?>(null)
    private val reviewed = AtomicReference<StoredScanPolicy?>(null)

    fun update(policy: StoredScanPolicy) {
        val previous = current.getAndSet(policy)
        if (previous?.identity() != policy.identity()) {
            reviewed.set(null)
        }
    }

    fun clear() {
        current.set(null)
        reviewed.set(null)
    }

    fun currentPolicy(): StoredScanPolicy? = current.get()

    fun requireCurrentPolicy(): StoredScanPolicy =
        requireNotNull(current.get()) { "Rust scan policy has not been resolved for this page" }

    fun markReviewed(policy: StoredScanPolicy) {
        check(current.get()?.identity() == policy.identity()) {
            "scan policy changed while the full-resolution preview was being processed"
        }
        reviewed.set(policy)
    }

    fun registrationEvidence(): List<String> {
        val policy = requireNotNull(reviewed.get()) {
            "registration requires a Rust-issued policy identity from the reviewed preview"
        }
        return listOf(
            "A2D_POLICY_LAYOUT=${policy.layoutId}",
            "A2D_POLICY_VERSION=${policy.processingPolicyVersion}",
            "A2D_PIPELINE_VERSION=${policy.pipelineVersion}",
        )
    }

    private fun StoredScanPolicy.identity(): Triple<String, Int, Int> =
        Triple(layoutId, processingPolicyVersion, pipelineVersion)
}
