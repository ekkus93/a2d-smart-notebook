package com.a2d.notebook.feature.ocr

import android.content.Context
import android.graphics.BitmapFactory
import java.io.File
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ThreadFactory
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.CompleteOcrJobRequest as FfiCompleteOcrJobRequest
import uniffi.a2d_ffi.EnqueueOcrJobRequest as FfiEnqueueOcrJobRequest
import uniffi.a2d_ffi.OcrInputKind as FfiOcrInputKind
import uniffi.a2d_ffi.OcrProviderAvailability as FfiOcrProviderAvailability
import uniffi.a2d_ffi.OcrQueueJob as FfiOcrQueueJob
import uniffi.a2d_ffi.OcrQueueJobStatus as FfiOcrQueueJobStatus
import uniffi.a2d_ffi.RegisteredScan

/** Android projection of one Rust-owned durable OCR queue row. */
data class AndroidOcrQueueJob(
    val jobId: String,
    val scanId: String,
    val inputAssetId: String,
    val inputKind: OcrInputKind,
    val mediaType: String,
    val relativePath: String,
    val byteLength: ULong,
    val widthPx: UInt,
    val heightPx: UInt,
    val status: AndroidOcrQueueJobStatus,
    val attemptCount: UInt,
    val retryable: Boolean,
    val nextRetryAtMs: Long?,
    val lastErrorCode: String?,
    val lastErrorMessage: String?,
    val provider: String?,
    val providerVersion: String?,
    val modelName: String?,
    val providerAvailability: AndroidOcrProviderAvailability,
    val lastOcrRunId: String?,
    val cancellationRequested: Boolean,
) {
    fun preparedInput(): PreparedAndroidOcrInput =
        PreparedAndroidOcrInput(
            scanId = scanId,
            inputAssetId = inputAssetId,
            inputKind = inputKind,
            mediaType = mediaType,
            relativePath = relativePath,
            byteLength = byteLength,
            widthPx = widthPx,
            heightPx = heightPx,
        )
}

enum class AndroidOcrQueueJobStatus {
    Queued,
    Running,
    Recognized,
    Unavailable,
    Cancelled,
}

enum class AndroidOcrProviderAvailability {
    Unknown,
    Available,
    Unavailable,
    Failed,
}

interface AndroidOcrQueueGateway {
    fun enqueue(
        scanId: String,
        inputKind: OcrInputKind,
        widthPx: UInt,
        heightPx: UInt,
    ): AndroidOcrQueueJob

    fun claimNext(): AndroidOcrQueueJob?

    fun get(jobId: String): AndroidOcrQueueJob

    fun requestCancellation(jobId: String): AndroidOcrQueueJob

    /** Compatibility/test-only split completion; the processor must use transactional finalization. */
    fun complete(
        jobId: String,
        ocrRunId: String,
        retryable: Boolean,
    ): AndroidOcrQueueJob
}

class FfiAndroidOcrQueueGateway(private val client: A2dClient) : AndroidOcrQueueGateway {
    override fun enqueue(
        scanId: String,
        inputKind: OcrInputKind,
        widthPx: UInt,
        heightPx: UInt,
    ): AndroidOcrQueueJob =
        client
            .enqueueOcrJob(
                FfiEnqueueOcrJobRequest(
                    scanId = scanId,
                    inputKind = inputKind.toQueueFfi(),
                    widthPx = widthPx,
                    heightPx = heightPx,
                ),
            ).toAndroid()

    override fun claimNext(): AndroidOcrQueueJob? = client.claimNextOcrJob()?.toAndroid()

    override fun get(jobId: String): AndroidOcrQueueJob = client.getOcrJob(jobId).toAndroid()

    override fun requestCancellation(jobId: String): AndroidOcrQueueJob =
        client.requestOcrJobCancellation(jobId).toAndroid()

    override fun complete(
        jobId: String,
        ocrRunId: String,
        retryable: Boolean,
    ): AndroidOcrQueueJob =
        client
            .completeOcrJob(
                FfiCompleteOcrJobRequest(
                    jobId = jobId,
                    ocrRunId = ocrRunId,
                    retryable = retryable,
                ),
            ).toAndroid()
}

internal sealed interface AndroidOcrQueueStep {
    data object Idle : AndroidOcrQueueStep

    data class Completed(val job: AndroidOcrQueueJob) : AndroidOcrQueueStep

    data class RecoverableFailure(
        val job: AndroidOcrQueueJob,
        val message: String,
    ) : AndroidOcrQueueStep
}

/**
 * Executes exactly one Rust-claimed OCR job. The queue transition remains Rust-owned; Android only
 * invokes the local provider and submits its outcome to one Rust transactional finalizer.
 */
internal class AndroidOcrQueueProcessor(
    private val gateway: AndroidOcrQueueGateway,
    private val workflow: AndroidOcrWorkflow,
) {
    fun processNext(onClaimed: (AndroidOcrQueueJob) -> Unit = {}): AndroidOcrQueueStep {
        val claimed = gateway.claimNext() ?: return AndroidOcrQueueStep.Idle
        onClaimed(claimed)
        val result =
            workflow.runClaimedJob(
                AndroidClaimedOcrStartRequest(
                    jobId = claimed.jobId,
                    attemptCount = claimed.attemptCount,
                    startRequest =
                        AndroidOcrStartRequest(
                            scanId = claimed.scanId,
                            inputKind = claimed.inputKind,
                            widthPx = claimed.widthPx,
                            heightPx = claimed.heightPx,
                        ),
                ),
            )
        val finalizedJob =
            result.finalizedJob
                ?: return AndroidOcrQueueStep.RecoverableFailure(
                    job = claimed,
                    message = result.message ?: "OCR result could not be finalized",
                )
        return AndroidOcrQueueStep.Completed(finalizedJob)
    }
}

/**
 * Serial in-process executor for the Rust-owned durable OCR queue.
 *
 * Queue rows survive process death in SQLite. On app startup [resume] asks Rust for due work;
 * Rust first reconciles any job that was left Running by a prior process. Provider waits happen on
 * this executor thread, never the main thread. A running cancellation request interrupts that
 * thread so the ML Kit adapter can return its explicit Cancelled outcome, which is then recorded
 * and committed through Rust.
 */
class AndroidOcrQueueExecutor internal constructor(
    private val libraryRoot: File,
    private val gateway: AndroidOcrQueueGateway,
    private val processor: AndroidOcrQueueProcessor,
    private val scheduler: ScheduledExecutorService,
    private val nowMs: () -> Long,
    private val closeProvider: () -> Unit,
) : AutoCloseable {
    private val closed = AtomicBoolean(false)

    @Volatile
    private var runningJobId: String? = null

    @Volatile
    private var workerThread: Thread? = null

    fun resume() {
        scheduleDrain(0L)
    }

    fun enqueueRegisteredScan(scan: RegisteredScan): AndroidOcrQueueJob {
        val (widthPx, heightPx) = readRegisteredOcrDimensions(libraryRoot, scan.ocrPath)
        val job =
            gateway.enqueue(
                scanId = scan.scanId,
                inputKind = OcrInputKind.OcrOptimized,
                widthPx = widthPx,
                heightPx = heightPx,
            )
        scheduleDrain(0L)
        return job
    }

    fun cancel(jobId: String): AndroidOcrQueueJob {
        val job = gateway.requestCancellation(jobId)
        if (runningJobId == jobId) {
            workerThread?.interrupt()
        }
        scheduleDrain(0L)
        return job
    }

    private fun scheduleDrain(delayMs: Long) {
        if (closed.get()) return
        scheduler.schedule(
            { drain() },
            max(0L, delayMs),
            TimeUnit.MILLISECONDS,
        )
    }

    private fun drain() {
        if (closed.get()) return
        workerThread = Thread.currentThread()
        try {
            while (!closed.get()) {
                when (
                    val step = processor.processNext { claimed ->
                        runningJobId = claimed.jobId
                    }
                ) {
                    AndroidOcrQueueStep.Idle -> return
                    is AndroidOcrQueueStep.RecoverableFailure -> {
                        runningJobId = null
                        Thread.interrupted()
                        scheduleDrain(RECOVERY_RETRY_DELAY_MS)
                        return
                    }
                    is AndroidOcrQueueStep.Completed -> {
                        runningJobId = null
                        Thread.interrupted()
                        if (
                            step.job.status == AndroidOcrQueueJobStatus.Queued &&
                                step.job.nextRetryAtMs != null
                        ) {
                            scheduleDrain(step.job.nextRetryAtMs - nowMs())
                        }
                    }
                }
            }
        } catch (_: InterruptedException) {
            Thread.interrupted()
            scheduleDrain(0L)
        } catch (_: Exception) {
            Thread.interrupted()
            scheduleDrain(RECOVERY_RETRY_DELAY_MS)
        } finally {
            runningJobId = null
            workerThread = null
        }
    }

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        workerThread?.interrupt()
        scheduler.shutdownNow()
        closeProvider()
    }

    companion object {
        internal fun create(
            context: Context,
            libraryRoot: File,
            client: A2dClient,
        ): AndroidOcrQueueExecutor {
            val provider = MlKitAndroidOcrProvider.create(context, libraryRoot)
            val queueGateway = FfiAndroidOcrQueueGateway(client)
            val workflow = AndroidOcrWorkflow(FfiRustOcrGateway(client), provider)
            return AndroidOcrQueueExecutor(
                libraryRoot = libraryRoot,
                gateway = queueGateway,
                processor = AndroidOcrQueueProcessor(queueGateway, workflow),
                scheduler = Executors.newSingleThreadScheduledExecutor(ocrThreadFactory()),
                nowMs = System::currentTimeMillis,
                closeProvider = provider::close,
            )
        }
    }
}

/** App-lifetime access to the local OCR queue executor. */
object AndroidOcrQueueRuntime {
    @Volatile
    private var executor: AndroidOcrQueueExecutor? = null

    fun resume(
        context: Context,
        client: A2dClient,
        libraryRoot: File,
    ) {
        getOrCreate(context, client, libraryRoot).resume()
    }

    fun enqueueRegisteredScan(
        context: Context,
        client: A2dClient,
        libraryRoot: File,
        scan: RegisteredScan,
    ): Result<AndroidOcrQueueJob> =
        runCatching {
            getOrCreate(context, client, libraryRoot).enqueueRegisteredScan(scan)
        }

    fun cancel(
        context: Context,
        client: A2dClient,
        libraryRoot: File,
        jobId: String,
    ): Result<AndroidOcrQueueJob> =
        runCatching {
            getOrCreate(context, client, libraryRoot).cancel(jobId)
        }

    private fun getOrCreate(
        context: Context,
        client: A2dClient,
        libraryRoot: File,
    ): AndroidOcrQueueExecutor {
        executor?.let { return it }
        synchronized(this) {
            executor?.let { return it }
            return AndroidOcrQueueExecutor
                .create(
                    context = context.applicationContext,
                    libraryRoot = libraryRoot,
                    client = client,
                ).also { executor = it }
        }
    }
}

internal fun readRegisteredOcrDimensions(
    libraryRoot: File,
    ocrPath: String,
): Pair<UInt, UInt> {
    val root = libraryRoot.canonicalFile
    val file = File(ocrPath).canonicalFile
    require(file.toPath().startsWith(root.toPath())) {
        "registered OCR asset is outside the app library root"
    }
    require(file.isFile) { "registered OCR asset is missing" }

    val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(file.absolutePath, options)
    require(options.outWidth > 0 && options.outHeight > 0) {
        "registered OCR asset dimensions are unavailable"
    }
    return options.outWidth.toUInt() to options.outHeight.toUInt()
}

internal fun FfiOcrQueueJob.toAndroid(): AndroidOcrQueueJob =
    AndroidOcrQueueJob(
        jobId = jobId,
        scanId = scanId,
        inputAssetId = inputAssetId,
        inputKind = inputKind.toQueueAndroid(),
        mediaType = mediaType,
        relativePath = relativePath,
        byteLength = byteLength,
        widthPx = widthPx,
        heightPx = heightPx,
        status =
            when (status) {
                FfiOcrQueueJobStatus.QUEUED -> AndroidOcrQueueJobStatus.Queued
                FfiOcrQueueJobStatus.RUNNING -> AndroidOcrQueueJobStatus.Running
                FfiOcrQueueJobStatus.RECOGNIZED -> AndroidOcrQueueJobStatus.Recognized
                FfiOcrQueueJobStatus.UNAVAILABLE -> AndroidOcrQueueJobStatus.Unavailable
                FfiOcrQueueJobStatus.CANCELLED -> AndroidOcrQueueJobStatus.Cancelled
            },
        attemptCount = attemptCount,
        retryable = retryable,
        nextRetryAtMs = nextRetryAtMs,
        lastErrorCode = lastErrorCode,
        lastErrorMessage = lastErrorMessage,
        provider = provider,
        providerVersion = providerVersion,
        modelName = modelName,
        providerAvailability =
            when (providerAvailability) {
                FfiOcrProviderAvailability.UNKNOWN -> AndroidOcrProviderAvailability.Unknown
                FfiOcrProviderAvailability.AVAILABLE -> AndroidOcrProviderAvailability.Available
                FfiOcrProviderAvailability.UNAVAILABLE -> AndroidOcrProviderAvailability.Unavailable
                FfiOcrProviderAvailability.FAILED -> AndroidOcrProviderAvailability.Failed
            },
        lastOcrRunId = lastOcrRunId,
        cancellationRequested = cancellationRequested,
    )

private fun OcrInputKind.toQueueFfi(): FfiOcrInputKind =
    when (this) {
        OcrInputKind.Original -> FfiOcrInputKind.ORIGINAL
        OcrInputKind.Corrected -> FfiOcrInputKind.CORRECTED
        OcrInputKind.OcrOptimized -> FfiOcrInputKind.OCR_OPTIMIZED
    }

private fun FfiOcrInputKind.toQueueAndroid(): com.a2d.notebook.feature.ocr.OcrInputKind =
    when (this) {
        FfiOcrInputKind.ORIGINAL -> com.a2d.notebook.feature.ocr.OcrInputKind.Original
        FfiOcrInputKind.CORRECTED -> com.a2d.notebook.feature.ocr.OcrInputKind.Corrected
        FfiOcrInputKind.OCR_OPTIMIZED -> com.a2d.notebook.feature.ocr.OcrInputKind.OcrOptimized
    }

private fun ocrThreadFactory(): ThreadFactory =
    ThreadFactory { runnable ->
        Thread(runnable, "a2d-local-ocr-queue").apply { isDaemon = true }
    }

private const val RECOVERY_RETRY_DELAY_MS = 2_000L
