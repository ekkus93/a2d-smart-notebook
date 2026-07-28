package com.a2d.notebook.feature.scanner.singlepage

import com.a2d.notebook.rustbridge.AnalyzedPageMarker
import com.a2d.notebook.rustbridge.AnalyzedPagePoint
import com.a2d.notebook.rustbridge.EncodedPageRotation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.a2d_ffi.RegistrationImageRotation

class ScanRegistrationRequestTest {
    @Test
    fun rotationsMapWithoutChangingMeaning() {
        assertEquals(
            RegistrationImageRotation.DEGREES0,
            EncodedPageRotation.DEGREES_0.toRegistrationImageRotation(),
        )
        assertEquals(
            RegistrationImageRotation.DEGREES90,
            EncodedPageRotation.DEGREES_90.toRegistrationImageRotation(),
        )
        assertEquals(
            RegistrationImageRotation.DEGREES180,
            EncodedPageRotation.DEGREES_180.toRegistrationImageRotation(),
        )
        assertEquals(
            RegistrationImageRotation.DEGREES270,
            EncodedPageRotation.DEGREES_270.toRegistrationImageRotation(),
        )
    }

    @Test
    fun markerConversionRequiresExactlyFourSemanticRoles() {
        val markers = listOf("TL", "TR", "BR", "BL").mapIndexed(::marker)
        val converted = markers.toRegistrationMarkers()
        assertEquals(listOf("TL", "TR", "BR", "BL"), converted.map { it.role })
        assertEquals(listOf(0u, 1u, 2u, 3u), converted.map { it.id })

        val invalid = runCatching { markers.dropLast(1).toRegistrationMarkers() }.exceptionOrNull()
        assertTrue(invalid is IllegalArgumentException)
    }

    @Test
    fun warningConversionIsStableAndRejectsMissingMarkers() {
        assertEquals(
            listOf("LOW_FOCUS", "TOO_DARK"),
            setOf(CapturePolicyWarning.TOO_DARK, CapturePolicyWarning.LOW_FOCUS)
                .toRegistrationWarningCodes(),
        )
        val invalid =
            runCatching {
                setOf(CapturePolicyWarning.MISSING_MARKERS).toRegistrationWarningCodes()
            }.exceptionOrNull()
        assertTrue(invalid is IllegalArgumentException)
    }

    private fun marker(index: Int, role: String): AnalyzedPageMarker =
        AnalyzedPageMarker(
            role = role,
            family = "tagStandard41h12",
            id = index.toLong(),
            hammingErrors = 0,
            decisionMargin = 50.0,
            center = AnalyzedPagePoint(10.0, 10.0),
            corners =
                listOf(
                    AnalyzedPagePoint(0.0, 0.0),
                    AnalyzedPagePoint(1.0, 0.0),
                    AnalyzedPagePoint(1.0, 1.0),
                    AnalyzedPagePoint(0.0, 1.0),
                ),
        )
}
