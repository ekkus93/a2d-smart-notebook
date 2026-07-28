package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.EncodedPageRotation
import uniffi.a2d_ffi.RegisterScanRequest
import uniffi.a2d_ffi.RegistrationImageFormat
import uniffi.a2d_ffi.RegistrationImageRotation
import uniffi.a2d_ffi.RegistrationMarker
import uniffi.a2d_ffi.ScanCaptureSource

internal fun SinglePageReviewArtifact.toRegisterScanRequest(): RegisterScanRequest {
    require(approvalAllowed) { "blocked review artifacts cannot be registered" }
    val payload = requireNotNull(pageCodePayload?.takeIf(String::isNotBlank)) {
        "validated Page Code payload is required for registration"
    }
    return RegisterScanRequest(
        stagingPath = stagingPath,
        pageCodePayload = payload,
        expectedPageId = captureRequest.pageId,
        activeNotebookId = captureRequest.activeNotebookId,
        captureSource = ScanCaptureSource.CAMERA,
        imageFormat = RegistrationImageFormat.JPEG,
        imageRotation = imageRotation.toRegistrationImageRotation(),
        capturedAtMs = capturedAtMs,
        observedMarkers = analysis.markers.toRegistrationMarkers(),
        previewWarnings = warnings.toRegistrationWarningCodes(),
        userApproved = true,
    )
}

internal fun EncodedPageRotation.toRegistrationImageRotation(): RegistrationImageRotation =
    when (this) {
        EncodedPageRotation.DEGREES_0 -> RegistrationImageRotation.DEGREES0
        EncodedPageRotation.DEGREES_90 -> RegistrationImageRotation.DEGREES90
        EncodedPageRotation.DEGREES_180 -> RegistrationImageRotation.DEGREES180
        EncodedPageRotation.DEGREES_270 -> RegistrationImageRotation.DEGREES270
    }

internal fun List<AnalyzedPageMarker>.toRegistrationMarkers(): List<RegistrationMarker> {
    val markers =
        map { marker ->
            require(marker.id in 0L..UInt.MAX_VALUE.toLong()) {
                "marker ID must fit an unsigned 32-bit integer"
            }
            RegistrationMarker(role = marker.role, id = marker.id.toUInt())
        }
    val roles = markers.map { it.role.trim().uppercase() }
    require(roles.size == 4 && roles.toSet() == setOf("TL", "TR", "BR", "BL")) {
        "registration requires exactly one TL, TR, BR, and BL marker"
    }
    return markers
}

internal fun Set<CapturePolicyWarning>.toRegistrationWarningCodes(): List<String> {
    require(CapturePolicyWarning.MISSING_MARKERS !in this) {
        "a capture with missing markers cannot be approved for registration"
    }
    return map { it.name }.sorted()
}
