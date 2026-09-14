package com.a2d.notebook.feature.ocr

import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.OcrSearchDocumentKind as FfiOcrSearchDocumentKind
import uniffi.a2d_ffi.SearchOcrTextRequest as FfiSearchOcrTextRequest

/** Android-facing adapter for Rust-owned local OCR search. */
interface AndroidOcrSearchGateway {
    fun searchOcrText(request: AndroidOcrSearchRequest): AndroidOcrSearchResults
}

class FfiAndroidOcrSearchGateway(private val client: A2dClient) : AndroidOcrSearchGateway {
    override fun searchOcrText(request: AndroidOcrSearchRequest): AndroidOcrSearchResults {
        val results =
            client.searchOcrText(
                FfiSearchOcrTextRequest(
                    query = request.query,
                    limit = request.limit,
                ),
            )
        return AndroidOcrSearchResults(
            query = results.query,
            hits =
                results.hits.map { hit ->
                    AndroidOcrSearchHit(
                        pageId = hit.pageId,
                        scanId = hit.scanId,
                        ocrRunId = hit.ocrRunId,
                        textRegionId = hit.textRegionId,
                        documentKind = hit.documentKind.toAndroid(),
                        snippet = hit.snippet,
                    )
                },
        )
    }
}

/**
 * Converts a search submission into presentation state.
 *
 * Validation and query syntax remain Rust-owned. This controller only avoids a blank-query trip
 * across the FFI boundary and preserves any Rust error text for local presentation.
 */
class AndroidOcrSearchController(
    private val gateway: AndroidOcrSearchGateway,
    private val defaultLimit: UInt = 20u,
) {
    fun submit(query: String): OcrSearchPresentationState {
        val trimmedQuery = query.trim()
        if (trimmedQuery.isEmpty()) {
            return OcrSearchPresentationState.noQuery(query)
        }

        return try {
            val results =
                gateway.searchOcrText(
                    AndroidOcrSearchRequest(
                        query = trimmedQuery,
                        limit = defaultLimit,
                    ),
                )
            if (results.hits.isEmpty()) {
                OcrSearchPresentationState.noMatches(trimmedQuery)
            } else {
                OcrSearchPresentationState.results(
                    query = trimmedQuery,
                    hits = results.hits.map { it.toPresentationHit() },
                )
            }
        } catch (failure: Exception) {
            OcrSearchPresentationState.error(
                query = trimmedQuery,
                message = failure.message ?: failure::class.java.simpleName,
            )
        }
    }
}

data class AndroidOcrSearchRequest(
    val query: String,
    val limit: UInt,
)

data class AndroidOcrSearchResults(
    val query: String,
    val hits: List<AndroidOcrSearchHit>,
)

data class AndroidOcrSearchHit(
    val pageId: String,
    val scanId: String,
    val ocrRunId: String,
    val textRegionId: String?,
    val documentKind: AndroidOcrSearchDocumentKind,
    val snippet: String,
) {
    fun toPresentationHit(): OcrSearchHitState =
        OcrSearchHitState(
            pageId = pageId,
            scanId = scanId,
            ocrRunId = ocrRunId,
            textRegionId = textRegionId,
            documentKind = documentKind,
            snippet = snippet,
        )
}

enum class AndroidOcrSearchDocumentKind(val displayLabel: String) {
    FullText("OCR full text"),
    TextRegion("OCR text region"),
}

data class OcrSearchPresentationState(
    val query: String = "",
    val status: OcrSearchPresentationStatus = OcrSearchPresentationStatus.NoQuery,
    val hits: List<OcrSearchHitState> = emptyList(),
    val errorMessage: String? = null,
) {
    companion object {
        fun noQuery(query: String = ""): OcrSearchPresentationState =
            OcrSearchPresentationState(
                query = query,
                status = OcrSearchPresentationStatus.NoQuery,
            )

        fun noMatches(query: String): OcrSearchPresentationState =
            OcrSearchPresentationState(
                query = query,
                status = OcrSearchPresentationStatus.NoMatches,
            )

        fun results(query: String, hits: List<OcrSearchHitState>): OcrSearchPresentationState =
            OcrSearchPresentationState(
                query = query,
                status = OcrSearchPresentationStatus.Results,
                hits = hits,
            )

        fun error(query: String, message: String): OcrSearchPresentationState =
            OcrSearchPresentationState(
                query = query,
                status = OcrSearchPresentationStatus.Error,
                errorMessage = message,
            )
    }
}

data class OcrSearchHitState(
    val pageId: String,
    val scanId: String,
    val ocrRunId: String,
    val textRegionId: String?,
    val documentKind: AndroidOcrSearchDocumentKind,
    val snippet: String,
)

enum class OcrSearchPresentationStatus {
    NoQuery,
    NoMatches,
    Results,
    Error,
}

private fun FfiOcrSearchDocumentKind.toAndroid(): AndroidOcrSearchDocumentKind =
    when (this) {
        FfiOcrSearchDocumentKind.FULL_TEXT -> AndroidOcrSearchDocumentKind.FullText
        FfiOcrSearchDocumentKind.TEXT_REGION -> AndroidOcrSearchDocumentKind.TextRegion
    }
