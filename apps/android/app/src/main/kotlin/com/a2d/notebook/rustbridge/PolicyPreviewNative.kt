package com.a2d.notebook.rustbridge

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer

internal class PolicyPagePreviewCancellation : AutoCloseable {
    private val lock = Any()
    private var handle: Pointer? =
        requireNotNull(policyPreviewNativeLibrary.a2d_policy_preview_cancellation_new()) {
            "Rust policy preview cancellation allocation returned null"
        }
    private var activeBorrows = 0
    private var closeRequested = false

    fun cancel() {
        synchronized(lock) {
            handle?.let(policyPreviewNativeLibrary::a2d_policy_preview_cancellation_cancel)
        }
    }

    fun <T> withPointer(block: (Pointer) -> T): T {
        val pointer =
            synchronized(lock) {
                check(!closeRequested) { "policy preview cancellation has already been closed" }
                activeBorrows = Math.incrementExact(activeBorrows)
                requireNotNull(handle) { "policy preview cancellation handle is unavailable" }
            }
        try {
            return block(pointer)
        } finally {
            val released =
                synchronized(lock) {
                    check(activeBorrows > 0) { "policy preview cancellation borrow underflow" }
                    activeBorrows--
                    if (closeRequested && activeBorrows == 0) {
                        handle.also { handle = null }
                    } else {
                        null
                    }
                }
            released?.let(policyPreviewNativeLibrary::a2d_policy_preview_cancellation_free)
        }
    }

    override fun close() {
        val released =
            synchronized(lock) {
                if (closeRequested) return
                closeRequested = true
                handle?.let(policyPreviewNativeLibrary::a2d_policy_preview_cancellation_cancel)
                if (activeBorrows == 0) {
                    handle.also { handle = null }
                } else {
                    null
                }
            }
        released?.let(policyPreviewNativeLibrary::a2d_policy_preview_cancellation_free)
    }
}

internal interface PolicyPreviewNativeLibrary : Library {
    fun a2d_policy_preview_cancellation_new(): Pointer?

    fun a2d_policy_preview_cancellation_cancel(cancellation: Pointer?)

    fun a2d_policy_preview_cancellation_free(cancellation: Pointer?)

    fun a2d_process_encoded_page_preview_v2(
        bytes: ByteArray,
        bytesLen: Long,
        formatCode: Int,
        rotationDegrees: Int,
        layoutIdBytes: ByteArray,
        layoutIdLen: Long,
        processingPolicyVersion: Int,
        cancellation: Pointer?,
        status: PreviewProcessingStatus,
    ): PreviewProcessingBuffer.ByValue

    fun a2d_preview_buffer_free(buffer: PreviewProcessingBuffer.ByValue)
}

internal val policyPreviewNativeLibrary: PolicyPreviewNativeLibrary by lazy(
    LazyThreadSafetyMode.SYNCHRONIZED,
) {
    Native.load("a2d_ffi", PolicyPreviewNativeLibrary::class.java)
}
