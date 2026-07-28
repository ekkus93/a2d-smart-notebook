package com.a2d.notebook.feature.scanner.camera

import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.NotFoundException
import com.google.zxing.PlanarYUVLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ThreadFactory

private const val MAX_LIVE_QR_PIXELS = 4_194_304L

private fun qrThreadFactory(): ThreadFactory = ThreadFactory { runnable ->
    Thread(runnable, "a2d-live-qr-decode").apply { isDaemon = true }
}

enum class LiveQrDropReason {
    SUPERSEDED_BEFORE_START,
    SUPERSEDED_IN_FLIGHT,
    CANCELLED,
    CLOSED,
}

sealed interface LiveQrCodeEvent {
    data class Found(
        val frameSequence: Long,
        val frameTimestampNanos: Long,
        val payload: String,
        val pixelBufferCopies: Int,
    ) : LiveQrCodeEvent

    data class NotFound(
        val frameSequence: Long,
        val frameTimestampNanos: Long,
        val pixelBufferCopies: Int,
    ) : LiveQrCodeEvent

    data class Failed(
        val frameSequence: Long,
        val frameTimestampNanos: Long,
        val message: String,
        val cause: Exception,
    ) : LiveQrCodeEvent

    data class Dropped(
        val frameSequence: Long,
        val reason: LiveQrDropReason,
    ) : LiveQrCodeEvent

    data class StaleResultDiscarded(
        val frameSequence: Long,
        val reason: LiveQrDropReason,
    ) : LiveQrCodeEvent

    data class SubmissionRejected(
        val frameSequence: Long,
        val message: String,
    ) : LiveQrCodeEvent

    data object Closed : LiveQrCodeEvent
}

fun interface GrayQrCodeDecoder {
    @Throws(Exception::class)
    fun decode(frame: CameraAnalysisFrame): String?
}

/**
 * Bounded ZXing decoder for one owned CameraX luminance frame.
 *
 * ZXing's planar source requires a heap byte array, so this boundary performs exactly one explicit
 * copy from the frame's owned direct buffer. Rotation is normalized before decoding; the source
 * frame remains unchanged. Returned text is untrusted and must be resolved by Rust before identity
 * or capture eligibility is updated.
 */
object ZxingGrayQrCodeDecoder : GrayQrCodeDecoder {
    override fun decode(frame: CameraAnalysisFrame): String? {
        val pixels = Math.multiplyExact(frame.width.toLong(), frame.height.toLong())
        require(pixels <= MAX_LIVE_QR_PIXELS) { "live Page Code frame exceeds the QR pixel limit" }
        val input = ByteArray(Math.toIntExact(pixels)).also(frame.luminanceBuffer()::get)
        val rotated = rotateGray8(input, frame.width, frame.height, frame.rotationDegrees)
        val source =
            PlanarYUVLuminanceSource(
                rotated.bytes,
                rotated.width,
                rotated.height,
                0,
                0,
                rotated.width,
                rotated.height,
                false,
            )
        val reader = MultiFormatReader()
        return try {
            reader.setHints(
                mapOf(
                    DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
                    DecodeHintType.TRY_HARDER to true,
                ),
            )
            reader.decodeWithState(BinaryBitmap(HybridBinarizer(source))).text
        } catch (_: NotFoundException) {
            null
        } finally {
            reader.reset()
        }
    }
}

internal data class RotatedGray8(
    val width: Int,
    val height: Int,
    val bytes: ByteArray,
)

internal fun rotateGray8(
    source: ByteArray,
    width: Int,
    height: Int,
    rotationDegrees: Int,
): RotatedGray8 {
    require(width > 0 && height > 0) { "grayscale dimensions must be positive" }
    require(source.size == Math.multiplyExact(width, height)) {
        "grayscale byte count does not match its dimensions"
    }
    require(rotationDegrees in setOf(0, 90, 180, 270)) {
        "rotation must be 0, 90, 180, or 270 degrees"
    }
    if (rotationDegrees == 0) return RotatedGray8(width, height, source)

    val outputWidth = if (rotationDegrees == 90 || rotationDegrees == 270) height else width
    val outputHeight = if (rotationDegrees == 90 || rotationDegrees == 270) width else height
    val output = ByteArray(source.size)
    for (y in 0 until height) {
        for (x in 0 until width) {
            val (targetX, targetY) =
                when (rotationDegrees) {
                    90 -> (height - 1 - y) to x
                    180 -> (width - 1 - x) to (height - 1 - y)
                    270 -> y to (width - 1 - x)
                    else -> error("validated rotation became invalid")
                }
            output[targetY * outputWidth + targetX] = source[y * width + x]
        }
    }
    return RotatedGray8(outputWidth, outputHeight, output)
}

/** Single-flight, keep-latest scheduler for live Page Code decoding. */
class LiveQrCodeScheduler(
    private val decoder: GrayQrCodeDecoder = ZxingGrayQrCodeDecoder,
    private val onEvent: (LiveQrCodeEvent) -> Unit,
    private val executor: ExecutorService = Executors.newSingleThreadExecutor(qrThreadFactory()),
) : AutoCloseable {
    private data class PendingFrame(
        val sequence: Long,
        val generation: Long,
        val frame: CameraAnalysisFrame,
    )

    private val lock = Any()
    private var nextSequence = 1L
    private var latestSequence = 0L
    private var generation = 0L
    private var pending: PendingFrame? = null
    private var workerScheduled = false
    private var closed = false

    fun submit(frame: CameraAnalysisFrame): Long {
        var dropped: PendingFrame? = null
        var schedule = false
        val sequence: Long
        synchronized(lock) {
            sequence = nextSequence
            nextSequence = Math.incrementExact(nextSequence)
            if (closed) {
                publish(LiveQrCodeEvent.SubmissionRejected(sequence, "live QR scheduler is closed"))
                return sequence
            }
            latestSequence = sequence
            dropped = pending
            pending = PendingFrame(sequence, generation, frame)
            if (!workerScheduled) {
                workerScheduled = true
                schedule = true
            }
        }
        dropped?.let {
            publish(LiveQrCodeEvent.Dropped(it.sequence, LiveQrDropReason.SUPERSEDED_BEFORE_START))
        }
        if (schedule) {
            try {
                executor.execute(::drain)
            } catch (failure: RejectedExecutionException) {
                synchronized(lock) {
                    generation = Math.incrementExact(generation)
                    pending = null
                    workerScheduled = false
                }
                publish(
                    LiveQrCodeEvent.Failed(
                        sequence,
                        frame.timestampNanos,
                        "live QR executor rejected its worker",
                        failure,
                    ),
                )
            }
        }
        return sequence
    }

    fun cancel() {
        val dropped = synchronized(lock) {
            if (closed) return
            generation = Math.incrementExact(generation)
            pending.also { pending = null }
        }
        dropped?.let { publish(LiveQrCodeEvent.Dropped(it.sequence, LiveQrDropReason.CANCELLED)) }
    }

    override fun close() {
        val dropped = synchronized(lock) {
            if (closed) return
            closed = true
            generation = Math.incrementExact(generation)
            pending.also { pending = null }
        }
        dropped?.let { publish(LiveQrCodeEvent.Dropped(it.sequence, LiveQrDropReason.CLOSED)) }
        executor.shutdownNow()
        publish(LiveQrCodeEvent.Closed)
    }

    private fun drain() {
        while (true) {
            val work = synchronized(lock) {
                pending.also {
                    pending = null
                    if (it == null) workerScheduled = false
                }
            } ?: return
            val outcome = runCatching { decoder.decode(work.frame) }
            val stale = synchronized(lock) {
                when {
                    closed -> LiveQrDropReason.CLOSED
                    work.generation != generation -> LiveQrDropReason.CANCELLED
                    work.sequence != latestSequence -> LiveQrDropReason.SUPERSEDED_IN_FLIGHT
                    else -> null
                }
            }
            if (stale != null) {
                publish(LiveQrCodeEvent.StaleResultDiscarded(work.sequence, stale))
            } else {
                outcome.fold(
                    onSuccess = { payload ->
                        if (payload == null) {
                            publish(
                                LiveQrCodeEvent.NotFound(
                                    work.sequence,
                                    work.frame.timestampNanos,
                                    pixelBufferCopies = 1,
                                ),
                            )
                        } else {
                            publish(
                                LiveQrCodeEvent.Found(
                                    work.sequence,
                                    work.frame.timestampNanos,
                                    payload,
                                    pixelBufferCopies = 1,
                                ),
                            )
                        }
                    },
                    onFailure = { failure ->
                        val exception = failure as? Exception ?: throw failure
                        publish(
                            LiveQrCodeEvent.Failed(
                                work.sequence,
                                work.frame.timestampNanos,
                                exception.message ?: "live Page Code decoding failed",
                                exception,
                            ),
                        )
                    },
                )
            }
        }
    }

    private fun publish(event: LiveQrCodeEvent) {
        onEvent(event)
    }
}
