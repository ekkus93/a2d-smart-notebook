package com.a2d.notebook.rustbridge

import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material3.Button
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.FileProvider
import java.io.File
import java.io.IOException
import java.nio.file.Files
import java.nio.file.LinkOption
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val MAX_CAPTURE_PIXELS = 48_000_000L
private const val MAX_DECODE_SIDE = 2_048
private const val QR_CAPTURE_DIRECTORY = "qr-capture"

data class QrCapture(val token: String, val file: File, val uri: Uri)

private fun qrCaptureDirectory(context: Context): File {
    val directory = context.cacheDir.resolve(QR_CAPTURE_DIRECTORY)
    if (!directory.exists() && !directory.mkdirs()) {
        throw IOException("could not create QR capture directory")
    }
    if (!directory.isDirectory) {
        throw IOException("QR capture cache path is not a directory")
    }
    return directory
}

fun createQrCapture(context: Context, prefix: String): QrCapture {
    require(prefix.length >= 3) { "QR capture prefix must contain at least three characters" }
    val token = UUID.randomUUID().toString()
    val directory = qrCaptureDirectory(context)
    val file = File.createTempFile(prefix, ".jpg", directory)
    return try {
        QrCapture(
            token = token,
            file = file,
            uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file),
        )
    } catch (failure: Exception) {
        deleteCaptureFile(file)?.let(failure::addSuppressed)
        throw failure
    }
}

/**
 * Restores and validates a pending QR capture path after Activity recreation. The saved path is
 * accepted only when it is a non-symlink regular file directly inside the approved cache root.
 */
fun resolvePendingQrCapture(context: Context, savedPath: String): File {
    val directory = qrCaptureDirectory(context).canonicalFile
    val candidate = File(savedPath)
    if (Files.isSymbolicLink(candidate.toPath())) {
        throw IOException("pending QR capture path is a symbolic link")
    }
    val canonicalCandidate = try {
        candidate.canonicalFile
    } catch (failure: IOException) {
        throw IOException("pending QR capture path cannot be canonicalized", failure)
    }
    if (canonicalCandidate.parentFile != directory) {
        throw IOException("pending QR capture path is outside the approved cache root")
    }
    if (!Files.isRegularFile(canonicalCandidate.toPath(), LinkOption.NOFOLLOW_LINKS)) {
        throw IOException("pending QR capture file is missing or not a regular file")
    }
    return canonicalCandidate
}

/** Returns abandoned regular files without deleting or modifying any of them. */
fun listOrphanedQrCaptureFiles(
    context: Context,
    pendingPath: String?,
): Result<List<File>> = try {
    val directory = qrCaptureDirectory(context).canonicalFile
    val pendingCanonical = pendingPath?.let { path -> File(path).canonicalFile }
    val entries = directory.listFiles()
        ?: throw IOException("could not list the QR capture cache directory")
    val orphans = entries
        .asSequence()
        .filter { file ->
            !Files.isSymbolicLink(file.toPath()) &&
                Files.isRegularFile(file.toPath(), LinkOption.NOFOLLOW_LINKS)
        }
        .map(File::getCanonicalFile)
        .filter { file -> file != pendingCanonical }
        .sortedBy(File::getName)
        .toList()
    Result.success(orphans)
} catch (failure: Exception) {
    Result.failure(failure)
}

private fun deleteCaptureFile(file: File): IOException? {
    if (!file.exists()) return null
    return if (file.delete()) {
        null
    } else {
        IOException("QR capture cleanup failed for ${file.name}")
    }
}

private fun combineFailure(primary: Throwable?, cleanup: IOException?): Throwable? {
    if (primary == null) return cleanup
    if (cleanup != null) primary.addSuppressed(cleanup)
    return primary
}

/**
 * Decodes one QR code locally. The returned text is never trusted here: every caller immediately
 * sends it to Rust for canonical grammar, checksum, identity, layout, and workflow validation.
 */
fun decodeQrImage(file: File): String {
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(file.absolutePath, bounds)
    require(bounds.outWidth > 0 && bounds.outHeight > 0) { "captured image is not decodable" }
    val pixels = bounds.outWidth.toLong() * bounds.outHeight.toLong()
    require(pixels <= MAX_CAPTURE_PIXELS) { "captured image exceeds the decode safety limit" }

    var sampleSize = 1
    while (bounds.outWidth / sampleSize > MAX_DECODE_SIDE ||
        bounds.outHeight / sampleSize > MAX_DECODE_SIDE
    ) {
        sampleSize *= 2
    }
    val bitmap = BitmapFactory.decodeFile(
        file.absolutePath,
        BitmapFactory.Options().apply { inSampleSize = sampleSize },
    ) ?: error("captured image could not be decoded")

    try {
        val colors = IntArray(bitmap.width * bitmap.height)
        bitmap.getPixels(colors, 0, bitmap.width, 0, 0, bitmap.width, bitmap.height)
        return decodeQrPixels(bitmap.width, bitmap.height, colors)
    } finally {
        bitmap.recycle()
    }
}

@Composable
fun QrCaptureButton(
    label: String,
    prefix: String,
    onDecoded: (String) -> Unit,
    onFailure: (Throwable?) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    // File identity and callback token survive Activity recreation. No Uri is trusted or restored;
    // only the saved file path is re-resolved through the approved cache root.
    var pendingToken by rememberSaveable(prefix) { mutableStateOf<String?>(null) }
    var pendingPath by rememberSaveable(prefix) { mutableStateOf<String?>(null) }

    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { ok ->
        val callbackToken = pendingToken
        val callbackPath = pendingPath
        pendingToken = null
        pendingPath = null

        if (callbackToken == null || callbackPath == null) {
            onFailure(IllegalStateException("QR capture callback has no matching pending token"))
            return@rememberLauncherForActivityResult
        }

        val captureFile = try {
            resolvePendingQrCapture(context, callbackPath)
        } catch (failure: Exception) {
            onFailure(failure)
            return@rememberLauncherForActivityResult
        }

        if (!ok) {
            onFailure(deleteCaptureFile(captureFile))
            return@rememberLauncherForActivityResult
        }

        scope.launch {
            var decoded: Result<String>? = null
            var cleanupFailure: IOException? = null
            try {
                decoded = catchingOperationFailure {
                    withContext(Dispatchers.IO) { decodeQrImage(captureFile) }
                }
            } catch (cancellation: CancellationException) {
                throw cancellation
            } finally {
                cleanupFailure = withContext(NonCancellable + Dispatchers.IO) {
                    deleteCaptureFile(captureFile)
                }
            }

            currentCoroutineContext().ensureActive()
            val result = requireNotNull(decoded) { "QR decode completed without a result" }
            val failure = combineFailure(result.exceptionOrNull(), cleanupFailure)
            if (failure != null) {
                onFailure(failure)
            } else {
                onDecoded(result.getOrThrow())
            }
        }
    }

    Button(
        enabled = pendingPath == null,
        onClick = {
            if (pendingPath != null) {
                onFailure(IllegalStateException("a QR capture is already pending"))
                return@Button
            }
            try {
                val capture = createQrCapture(context, prefix)
                pendingToken = capture.token
                pendingPath = capture.file.absolutePath
                launcher.launch(capture.uri)
            } catch (failure: Exception) {
                pendingToken = null
                pendingPath = null
                onFailure(failure)
            }
        },
    ) {
        Text(label)
    }
}
