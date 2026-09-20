package com.a2d.notebook.feature.ocr

import android.graphics.BitmapFactory
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.State
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import kotlin.math.roundToInt
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

object OcrRegionOverlayTestTags {
    const val CARD = "ocr_region_overlay_card"
    const val CANVAS = "ocr_region_overlay_canvas"
    const val DISABLED = "ocr_region_overlay_disabled"
    const val SELECTED = "ocr_region_overlay_selected"
}

@Composable
fun OcrRegionOverlayCard(
    overlay: OcrRegionOverlayState,
    modifier: Modifier = Modifier,
) {
    var selectedRegionId by rememberSaveable(overlay.renderableRegions.map { it.textRegionId }) {
        mutableStateOf<String?>(null)
    }
    val frame = overlay.coordinateFrame
    val sourceImage by rememberOcrSourceImage(overlay.sourceImagePath)
    val selectedRegion =
        overlay.renderableRegions.firstOrNull { it.textRegionId == selectedRegionId }

    Card(modifier.fillMaxWidth().testTag(OcrRegionOverlayTestTags.CARD)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_viewer_ocr_region_overlay_title),
                style = MaterialTheme.typography.titleMedium,
            )
            val disabledReason = overlay.disabledReason(frame, sourceImage)
            if (disabledReason != null) {
                Text(
                    text = disabledReason,
                    modifier = Modifier.testTag(OcrRegionOverlayTestTags.DISABLED),
                )
                return@Column
            }

            Text(
                stringResource(
                    R.string.page_viewer_ocr_region_overlay_detail,
                    overlay.renderableRegions.size,
                ),
            )
            val outlineColor = MaterialTheme.colorScheme.primary
            val selectedFillColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f)
            Canvas(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .height(320.dp)
                        .testTag(OcrRegionOverlayTestTags.CANVAS)
                        .pointerInput(frame, overlay.renderableRegions) {
                            detectTapGestures { tap ->
                                val transform = OcrOverlayTransform.contentFit(
                                    sourceWidthPx = frame!!.width,
                                    sourceHeightPx = frame.height,
                                    viewportWidthPx = size.width.toFloat(),
                                    viewportHeightPx = size.height.toFloat(),
                                )
                                val sourcePoint = transform.viewportToSource(OcrOverlayPoint(tap.x, tap.y))
                                selectedRegionId = sourcePoint?.let { point -> overlay.regionAt(point.x, point.y)?.textRegionId }
                            }
                        },
            ) {
                val transform = OcrOverlayTransform.contentFit(
                    sourceWidthPx = frame!!.width,
                    sourceHeightPx = frame.height,
                    viewportWidthPx = size.width,
                    viewportHeightPx = size.height,
                )
                drawImage(
                    image = sourceImage!!,
                    dstOffset = IntOffset(transform.offsetX.roundToInt(), transform.offsetY.roundToInt()),
                    dstSize = IntSize(transform.renderedWidthPx.roundToInt(), transform.renderedHeightPx.roundToInt()),
                )
                overlay.renderableRegions.forEach { region ->
                    val path = region.toCanvasPath(transform)
                    if (region.textRegionId == selectedRegionId) {
                        drawPath(path = path, color = selectedFillColor)
                    }
                    drawPath(path = path, color = outlineColor, style = Stroke(width = 3f))
                }
            }

            if (selectedRegion != null) {
                Column(
                    modifier = Modifier.testTag(OcrRegionOverlayTestTags.SELECTED),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(
                        stringResource(
                            R.string.page_viewer_ocr_region_selected_text,
                            selectedRegion.text,
                        ),
                    )
                    val confidence = selectedRegion.confidence
                    if (confidence == null) {
                        Text(stringResource(R.string.page_viewer_ocr_region_confidence_unavailable))
                    } else {
                        Text(
                            stringResource(
                                R.string.page_viewer_ocr_region_confidence,
                                (confidence.coerceIn(0f, 1f) * 100f).toInt(),
                            ),
                        )
                    }
                }
            } else {
                Text(stringResource(R.string.page_viewer_ocr_region_tap_hint))
            }
        }
    }
}

@Composable
private fun rememberOcrSourceImage(path: String?): State<ImageBitmap?> {
    val image = remember(path) { mutableStateOf<ImageBitmap?>(null) }
    LaunchedEffect(path) {
        image.value = null
        if (!path.isNullOrBlank()) {
            image.value = withContext(Dispatchers.IO) { BitmapFactory.decodeFile(path)?.asImageBitmap() }
        }
    }
    return image
}

@Composable
private fun OcrRegionOverlayState.disabledReason(
    frame: OcrRegionCoordinateFrame?,
    sourceImage: ImageBitmap?,
): String? =
    when {
        !enabled || frame == null -> stringResource(R.string.page_viewer_ocr_region_overlay_disabled)
        sourceImagePath.isNullOrBlank() -> stringResource(R.string.page_viewer_ocr_region_overlay_missing_image)
        sourceImage == null -> stringResource(R.string.page_viewer_ocr_region_overlay_image_unavailable)
        else -> null
    }

private fun OcrRegionOverlayRegion.toCanvasPath(transform: OcrOverlayTransform): Path =
    Path().apply {
        polygon.firstOrNull()?.let { first ->
            val start = transform.sourceToViewport(OcrOverlayPoint(first.x, first.y))
            moveTo(start.x, start.y)
            polygon.drop(1).forEach { point ->
                val mapped = transform.sourceToViewport(OcrOverlayPoint(point.x, point.y))
                lineTo(mapped.x, mapped.y)
            }
            close()
        }
    }
