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
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.FileProvider
import com.google.zxing.BarcodeFormat
import com.google.zxing.BinaryBitmap
import com.google.zxing.DecodeHintType
import com.google.zxing.MultiFormatReader
import com.google.zxing.RGBLuminanceSource
import com.google.zxing.common.HybridBinarizer
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private const val MAX_CAPTURE_PIXELS = 48_000_000L
private const val MAX_DECODE_SIDE = 2_048

data class QrCapture(val file: File, val uri: Uri)

fun createQrCapture(context: Context, prefix: String): QrCapture {
    val directory = context.cacheDir.resolve("qr-capture").apply {
        check(exists() || mkdirs()) { "could not create QR capture directory" }
    }
    val file = File.createTempFile(prefix, ".jpg", directory)
    return QrCapture(
        file = file,
        uri = FileProvider.getUriForFile(context, "${context.packageName}.files", file),
    )
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
        val source = RGBLuminanceSource(bitmap.width, bitmap.height, colors)
        val binary = BinaryBitmap(HybridBinarizer(source))
        val hints = mapOf(
            DecodeHintType.POSSIBLE_FORMATS to listOf(BarcodeFormat.QR_CODE),
            DecodeHintType.TRY_HARDER to true,
        )
        return MultiFormatReader().decode(binary, hints).text
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
    var pending by remember { mutableStateOf<QrCapture?>(null) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { ok ->
        val capture = pending
        pending = null
        if (!ok || capture == null) {
            capture?.file?.delete()
            onFailure(null)
            return@rememberLauncherForActivityResult
        }
        scope.launch {
            val result = runCatching {
                withContext(Dispatchers.IO) { decodeQrImage(capture.file) }
            }
            capture.file.delete()
            result.onSuccess(onDecoded).onFailure(onFailure)
        }
    }

    Button(
        onClick = {
            runCatching { createQrCapture(context, prefix) }
                .onSuccess {
                    pending = it
                    launcher.launch(it.uri)
                }
                .onFailure(onFailure)
        },
    ) {
        Text(label)
    }
}
