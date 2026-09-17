package com.a2d.notebook.feature.ocr

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R

object OcrRegionOverlayTestTags {
    const val CARD = "ocr_region_overlay_card"
    const val CANVAS = "ocr_region_overlay_canvas"
    const val DISABLED = "ocr_region_overlay_disabled"
    const val PARTIAL = "ocr_region_overlay_partial"
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
    val selectedRegion =
        overlay.renderableRegions.firstOrNull { it.textRegionId == selectedRegionId }

    Card(modifier.fillMaxWidth().testTag(OcrRegionOverlayTestTags.CARD)) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.page_viewer_ocr_region_overlay_title),
                style = MaterialTheme.typography.titleMedium,
            )
            if (!overlay.enabled || frame == null) {
                Text(
                    text = stringResource(R.string.page_viewer_ocr_region_overlay_disabled),
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
            if (overlay.isPartial) {
                Text(
                    text = "Showing ${overlay.regions.size} of ${overlay.totalRegionCount} persisted OCR regions; this overlay is partial.",
                    modifier = Modifier.testTag(OcrRegionOverlayTestTags.PARTIAL),
                )
            }
            val outlineColor = MaterialTheme.colorScheme.primary
            val selectedFillColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.18f)
            Canvas(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .aspectRatio(frame.aspectRatio.coerceIn(0.5f, 2.5f))
                        .testTag(OcrRegionOverlayTestTags.CANVAS)
                        .pointerInput(frame, overlay.renderableRegions) {
                            detectTapGestures { tap ->
                                val sourceX = tap.x * frame.width / size.width
                                val sourceY = tap.y * frame.height / size.height
                                selectedRegionId = overlay.regionAt(sourceX, sourceY)?.textRegionId
                            }
                        },
            ) {
                val scaleX = size.width / frame.width
                val scaleY = size.height / frame.height
                overlay.renderableRegions.forEach { region ->
                    val path = region.toCanvasPath(scaleX = scaleX, scaleY = scaleY)
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

private fun OcrRegionOverlayRegion.toCanvasPath(
    scaleX: Float,
    scaleY: Float,
): Path =
    Path().apply {
        polygon.firstOrNull()?.let { first ->
            moveTo(first.x * scaleX, first.y * scaleY)
            polygon.drop(1).forEach { point -> lineTo(point.x * scaleX, point.y * scaleY) }
            close()
        }
    }
