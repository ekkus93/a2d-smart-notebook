package com.a2d.notebook.feature.scanner.presentation

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import com.a2d.notebook.R
import com.a2d.notebook.feature.scanner.camera.CameraPreviewSurface
import com.a2d.notebook.feature.scanner.camera.CameraXAdapter
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import kotlin.math.max

object LiveScannerTestTags {
    const val ACTIVE_NOTEBOOK = "scanner_active_notebook"
    const val GUIDANCE = "scanner_guidance"
    const val IDENTITY_GATE = "scanner_identity_gate"
    const val OVERLAY = "scanner_overlay"
}

/** Actual CameraX preview plus reusable Milestone 8.2B presentation chrome. */
@Composable
fun LiveScannerPreview(
    adapter: CameraXAdapter,
    state: LiveScannerPresentationState,
    modifier: Modifier = Modifier,
) {
    LiveScannerChrome(
        state = state,
        modifier = modifier,
        preview = {
            CameraPreviewSurface(
                adapter = adapter,
                modifier = Modifier.fillMaxSize(),
            )
        },
    )
}

/**
 * Reusable scanner chrome that can be instrumented with a fake preview and later embedded by the
 * Milestone 8.4 single-page and batch scanner screens.
 */
@Composable
fun LiveScannerChrome(
    state: LiveScannerPresentationState,
    modifier: Modifier = Modifier,
    preview: @Composable BoxScope.() -> Unit,
) {
    Box(modifier = modifier.background(Color.Black)) {
        preview()
        state.overlay?.let { overlay ->
            LivePageMarkerOverlay(
                model = overlay,
                modifier = Modifier.fillMaxSize().testTag(LiveScannerTestTags.OVERLAY),
            )
        }
        ActiveNotebookBanner(
            state = state,
            modifier =
                Modifier
                    .align(Alignment.TopCenter)
                    .fillMaxWidth()
                    .padding(12.dp),
        )
        GuidanceBanner(
            guidance = state.guidance,
            identityGate = state.identityGate,
            modifier =
                Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .padding(12.dp),
        )
    }
}

@Composable
private fun ActiveNotebookBanner(
    state: LiveScannerPresentationState,
    modifier: Modifier,
) {
    val notebook = state.activeNotebook
    Surface(
        modifier = modifier.testTag(LiveScannerTestTags.ACTIVE_NOTEBOOK),
        shape = RoundedCornerShape(14.dp),
        color = MaterialTheme.colorScheme.surface.copy(alpha = 0.94f),
        tonalElevation = 6.dp,
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            Text(
                text = stringResource(R.string.scanner_destination_label),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (notebook == null) {
                Text(
                    text = stringResource(R.string.scanner_no_active_notebook),
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.error,
                )
            } else {
                Text(
                    text = notebook.displayName,
                    style = MaterialTheme.typography.titleLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = stringResource(R.string.scanner_destination_design, notebook.designId),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun GuidanceBanner(
    guidance: ScannerGuidance,
    identityGate: IdentityAutoCaptureGate,
    modifier: Modifier,
) {
    val containerColor =
        when (guidance.severity) {
            ScannerGuidanceSeverity.INFO -> MaterialTheme.colorScheme.surface.copy(alpha = 0.94f)
            ScannerGuidanceSeverity.WARNING ->
                MaterialTheme.colorScheme.tertiaryContainer.copy(alpha = 0.96f)
            ScannerGuidanceSeverity.BLOCKING ->
                MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.96f)
            ScannerGuidanceSeverity.POSITIVE ->
                MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.96f)
        }
    val contentColor =
        when (guidance.severity) {
            ScannerGuidanceSeverity.INFO -> MaterialTheme.colorScheme.onSurface
            ScannerGuidanceSeverity.WARNING -> MaterialTheme.colorScheme.onTertiaryContainer
            ScannerGuidanceSeverity.BLOCKING -> MaterialTheme.colorScheme.onErrorContainer
            ScannerGuidanceSeverity.POSITIVE -> MaterialTheme.colorScheme.onPrimaryContainer
        }

    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(14.dp),
        color = containerColor,
        tonalElevation = 6.dp,
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = guidanceText(guidance),
                style = MaterialTheme.typography.titleMedium,
                color = contentColor,
                modifier = Modifier.testTag(LiveScannerTestTags.GUIDANCE),
            )
            Row(
                modifier = Modifier.testTag(LiveScannerTestTags.IDENTITY_GATE),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text =
                        if (identityGate.allowed) {
                            stringResource(R.string.scanner_identity_verified)
                        } else {
                            stringResource(R.string.scanner_auto_capture_blocked)
                        },
                    style = MaterialTheme.typography.labelMedium,
                    color = contentColor,
                )
            }
        }
    }
}

@Composable
private fun guidanceText(guidance: ScannerGuidance): String =
    when (guidance.code) {
        ScannerGuidanceCode.SELECT_NOTEBOOK -> stringResource(R.string.scanner_guidance_select_notebook)
        ScannerGuidanceCode.FIND_PAGE_CODE -> stringResource(R.string.scanner_guidance_find_page_code)
        ScannerGuidanceCode.WRONG_NOTEBOOK -> stringResource(R.string.scanner_guidance_wrong_notebook)
        ScannerGuidanceCode.SELECT_MATCHING_NOTEBOOK ->
            stringResource(R.string.scanner_guidance_select_matching_notebook)
        ScannerGuidanceCode.REGISTER_NOTEBOOK ->
            stringResource(R.string.scanner_guidance_register_notebook)
        ScannerGuidanceCode.SMART_PAGE_OUTSIDE_NOTEBOOK_MODE ->
            stringResource(R.string.scanner_guidance_smart_page_mode)
        ScannerGuidanceCode.UNSUPPORTED_PAGE_CODE ->
            stringResource(R.string.scanner_guidance_unsupported_page)
        ScannerGuidanceCode.ANALYSIS_FAILED -> stringResource(R.string.scanner_guidance_analysis_failed)
        ScannerGuidanceCode.FIND_PAGE -> stringResource(R.string.scanner_guidance_find_page)
        ScannerGuidanceCode.SHOW_ALL_CORNERS ->
            stringResource(R.string.scanner_guidance_show_all_corners)
        ScannerGuidanceCode.USE_SUPPORTED_PAGE ->
            stringResource(R.string.scanner_guidance_supported_page)
        ScannerGuidanceCode.MOVE_CLOSER -> stringResource(R.string.scanner_guidance_move_closer)
        ScannerGuidanceCode.MOVE_FARTHER -> stringResource(R.string.scanner_guidance_move_farther)
        ScannerGuidanceCode.HOLD_STEADY -> stringResource(R.string.scanner_guidance_hold_steady)
        ScannerGuidanceCode.ADD_LIGHT -> stringResource(R.string.scanner_guidance_add_light)
        ScannerGuidanceCode.REDUCE_GLARE -> stringResource(R.string.scanner_guidance_reduce_glare)
        ScannerGuidanceCode.PAGE_ALIGNED -> stringResource(R.string.scanner_guidance_page_aligned)
    }

@Composable
private fun LivePageMarkerOverlay(
    model: ScannerOverlayModel,
    modifier: Modifier,
) {
    val pageColor =
        if (model.conflict) {
            MaterialTheme.colorScheme.error
        } else {
            MaterialTheme.colorScheme.primary
        }
    val markerColor =
        if (model.conflict) {
            MaterialTheme.colorScheme.error
        } else {
            MaterialTheme.colorScheme.tertiary
        }

    Canvas(modifier = modifier) {
        val mapper =
            PreviewCoordinateMapper(
                frameWidth = model.frameWidth,
                frameHeight = model.frameHeight,
                rotationDegrees = model.sourceRotationDegrees,
                previewSize = size,
            )
        if (model.pageBoundary.size == 4) {
            drawPath(
                path = model.pageBoundary.toPath(mapper),
                color = pageColor,
                style = Stroke(width = 5.dp.toPx()),
            )
        }
        model.markers.forEach { marker ->
            drawPath(
                path = marker.corners.toPath(mapper),
                color = markerColor,
                style = Stroke(width = 3.dp.toPx()),
            )
            marker.corners.forEach { point ->
                drawCircle(
                    color = markerColor,
                    radius = 4.dp.toPx(),
                    center = mapper.map(point),
                )
            }
        }
    }
}

private fun List<AnalyzedPagePoint>.toPath(mapper: PreviewCoordinateMapper): Path =
    Path().also { path ->
        if (isEmpty()) return@also
        val first = mapper.map(first())
        path.moveTo(first.x, first.y)
        drop(1).forEach { point ->
            val mapped = mapper.map(point)
            path.lineTo(mapped.x, mapped.y)
        }
        path.close()
    }

/** FILL_CENTER mapping shared by page and marker overlays and unit-tested for all rotations. */
internal class PreviewCoordinateMapper(
    frameWidth: Long,
    frameHeight: Long,
    private val rotationDegrees: Int,
    previewSize: Size,
) {
    private val sourceWidth = frameWidth.toDouble()
    private val sourceHeight = frameHeight.toDouble()
    private val rotatedWidth: Double
    private val rotatedHeight: Double
    private val scale: Double
    private val offsetX: Double
    private val offsetY: Double

    init {
        require(frameWidth > 0L && frameHeight > 0L)
        require(rotationDegrees in setOf(0, 90, 180, 270))
        require(previewSize.width > 0f && previewSize.height > 0f)
        val swapsAxes = rotationDegrees == 90 || rotationDegrees == 270
        rotatedWidth = if (swapsAxes) sourceHeight else sourceWidth
        rotatedHeight = if (swapsAxes) sourceWidth else sourceHeight
        scale =
            max(
                previewSize.width.toDouble() / rotatedWidth,
                previewSize.height.toDouble() / rotatedHeight,
            )
        offsetX = (previewSize.width.toDouble() - rotatedWidth * scale) / 2.0
        offsetY = (previewSize.height.toDouble() - rotatedHeight * scale) / 2.0
    }

    fun map(point: AnalyzedPagePoint): Offset {
        require(point.x.isFinite() && point.y.isFinite())
        val rotated = rotate(point)
        return Offset(
            x = (offsetX + rotated.x * scale).toFloat(),
            y = (offsetY + rotated.y * scale).toFloat(),
        )
    }

    private fun rotate(point: AnalyzedPagePoint): AnalyzedPagePoint =
        when (rotationDegrees) {
            0 -> point
            90 -> AnalyzedPagePoint(x = sourceHeight - point.y, y = point.x)
            180 -> AnalyzedPagePoint(x = sourceWidth - point.x, y = sourceHeight - point.y)
            270 -> AnalyzedPagePoint(x = point.y, y = sourceWidth - point.x)
            else -> error("validated rotation became invalid")
        }
}
