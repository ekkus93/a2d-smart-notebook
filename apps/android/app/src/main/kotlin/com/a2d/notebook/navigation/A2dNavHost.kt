package com.a2d.notebook.navigation

import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.navArgument
import com.a2d.notebook.feature.home.HomeScreen
import com.a2d.notebook.feature.library.ImportLibraryScreen
import com.a2d.notebook.feature.library.LibraryHubScreen
import com.a2d.notebook.feature.library.PageBrowserScreen
import com.a2d.notebook.feature.library.PageViewerScreen
import com.a2d.notebook.feature.library.PageViewerState
import com.a2d.notebook.feature.library.TrashScreen
import com.a2d.notebook.feature.notebook.NotebookDetailScreen
import com.a2d.notebook.feature.notebook.NotebookLibraryScreen
import com.a2d.notebook.feature.notebook.NotebookSetupScreen
import com.a2d.notebook.feature.notebook.PageCodeScreen
import com.a2d.notebook.feature.ocr.AndroidOcrCorrectionController
import com.a2d.notebook.feature.ocr.AndroidOcrSearchController
import com.a2d.notebook.feature.ocr.FfiAndroidOcrCorrectionGateway
import com.a2d.notebook.feature.ocr.FfiAndroidOcrReadback
import com.a2d.notebook.feature.ocr.FfiAndroidOcrSearchGateway
import com.a2d.notebook.feature.ocr.OcrCorrectionPresentationState
import com.a2d.notebook.feature.ocr.OcrPresentationState
import com.a2d.notebook.feature.ocr.OcrPresentationStatus
import com.a2d.notebook.feature.ocr.OcrRegionOverlayState
import com.a2d.notebook.feature.ocr.OcrSearchScreen
import com.a2d.notebook.feature.review.NeedsReviewScreen
import com.a2d.notebook.feature.scanner.singlepage.PolicyAwareBatchScannerRoute
import com.a2d.notebook.feature.scanner.singlepage.SinglePageScannerScreen
import com.a2d.notebook.feature.smartpage.SmartPageLibraryScreen
import com.a2d.notebook.feature.smartpage.SmartPagesScreen
import com.a2d.notebook.feature.version.VersionHistoryScreen
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.a2d_ffi.A2dClient
import uniffi.a2d_ffi.EnqueueOcrJobRequest
import uniffi.a2d_ffi.OcrInputKind
import uniffi.a2d_ffi.OcrQueueJob
import uniffi.a2d_ffi.OcrQueueJobStatus

object A2dDestinations {
    const val HOME = "home"
    const val LIBRARY = "library"
    const val PAGES = "library/pages"
    const val OCR_SEARCH = "library/ocr-search"
    const val PAGE_VIEWER_PATTERN = "library/pages/{pageId}?scanId={scanId}"
    const val NEEDS_REVIEW = "library/needs-review"
    const val SMART_PAGE_LIBRARY = "library/smart-pages"
    const val IMPORTS = "library/imports"
    const val TRASH = "library/trash"
    const val SINGLE_PAGE_SCANNER = "scanner/single"
    const val BATCH_SCANNER = "scanner/batch"
    const val NOTEBOOKS = "notebooks"
    const val ADD_NOTEBOOK = "notebooks/add"
    const val NOTEBOOK_DETAIL_PATTERN = "notebooks/detail/{notebookId}"
    const val SMART_PAGES = "smart-pages"
    const val PAGE_CODE_PATTERN = "page-code/{notebookId}"
    const val VERSION_HISTORY_PATTERN = "versions/{pageId}"

    fun pageViewer(pageId: String, scanId: String? = null): String =
        if (scanId == null) {
            "library/pages/$pageId"
        } else {
            "library/pages/$pageId?scanId=${Uri.encode(scanId)}"
        }

    fun pageCode(notebookId: String) = "page-code/$notebookId"
    fun notebookDetail(notebookId: String) = "notebooks/detail/$notebookId"
    fun versionHistory(pageId: String) = "versions/$pageId"
}

@Composable
fun A2dNavHost(
    navController: NavHostController,
    client: A2dClient? = null,
) {
    val ocrSearchController =
        remember(client) {
            client?.let { AndroidOcrSearchController(FfiAndroidOcrSearchGateway(it)) }
        }
    val ocrReadback = remember(client) { client?.let(::FfiAndroidOcrReadback) }
    val ocrCorrectionController =
        remember(client) {
            client?.let { AndroidOcrCorrectionController(FfiAndroidOcrCorrectionGateway(it)) }
        }

    NavHost(navController = navController, startDestination = A2dDestinations.HOME) {
        composable(A2dDestinations.HOME) {
            HomeScreen(
                onScanPage = { navController.navigate(A2dDestinations.SINGLE_PAGE_SCANNER) },
                onBatchScan = { navController.navigate(A2dDestinations.BATCH_SCANNER) },
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
                onOpenLibrary = { navController.navigate(A2dDestinations.LIBRARY) },
            )
        }
        composable(A2dDestinations.LIBRARY) {
            LibraryHubScreen(
                onBack = { navController.navigateUp() },
                onOpenNotebooks = { navController.navigate(A2dDestinations.NOTEBOOKS) },
                onOpenSmartPages = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenPages = { navController.navigate(A2dDestinations.PAGES) },
                onOpenOcrSearch = { navController.navigate(A2dDestinations.OCR_SEARCH) },
                onOpenPageSets = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenCollections = { navController.navigate(A2dDestinations.SMART_PAGE_LIBRARY) },
                onOpenImports = { navController.navigate(A2dDestinations.IMPORTS) },
                onOpenNeedsReview = { navController.navigate(A2dDestinations.NEEDS_REVIEW) },
                onOpenTrash = { navController.navigate(A2dDestinations.TRASH) },
            )
        }
        composable(A2dDestinations.PAGES) {
            PageBrowserScreen(
                onBack = { navController.navigateUp() },
                onOpenPage = { pageId -> navController.navigate(A2dDestinations.pageViewer(pageId)) },
                onOpenVersions = { pageId -> navController.navigate(A2dDestinations.versionHistory(pageId)) },
            )
        }
        composable(A2dDestinations.OCR_SEARCH) {
            OcrSearchScreen(
                onBack = { navController.navigateUp() },
                onOpenPage = { pageId -> navController.navigate(A2dDestinations.pageViewer(pageId)) },
                onOpenOcrHit = { hit ->
                    navController.navigate(A2dDestinations.pageViewer(hit.pageId, hit.scanId))
                },
                searchController = ocrSearchController,
            )
        }
        composable(
            route = A2dDestinations.PAGE_VIEWER_PATTERN,
            arguments =
                listOf(
                    navArgument("pageId") { type = NavType.StringType },
                    navArgument("scanId") {
                        type = NavType.StringType
                        nullable = true
                        defaultValue = null
                    },
                ),
        ) { entry ->
            val pageId = requireNotNull(entry.arguments?.getString("pageId"))
            val scanId = entry.arguments?.getString("scanId")
            val scope = rememberCoroutineScope()
            var viewerState by
                remember(pageId, scanId, client) {
                    mutableStateOf(
                        PageViewerState(
                            pageId = pageId,
                            preferredScanId = scanId,
                            viewerApiConnected = client != null,
                        ),
                    )
                }
            LaunchedEffect(pageId, scanId, client, ocrReadback, ocrCorrectionController) {
                if (client != null && ocrReadback != null) {
                    viewerState = hydratePageViewerMetadata(viewerState, pageId, scanId, client)
                    viewerState.preferredScanId?.let { selectedScanId ->
                        viewerState = hydrateOcrForViewer(viewerState, selectedScanId, client, ocrReadback)
                        viewerState = hydrateOcrCorrectionReviewForViewer(viewerState, selectedScanId, ocrCorrectionController)
                    }
                }
            }
            PageViewerScreen(
                pageId = pageId,
                onBack = { navController.navigateUp() },
                onOpenVersions = { id -> navController.navigate(A2dDestinations.versionHistory(id)) },
                onOpenNeedsReview = { navController.navigate(A2dDestinations.NEEDS_REVIEW) },
                state = viewerState,
                onStartOcr = { selectedScanId ->
                    scope.launch {
                        viewerState = enqueueOcrJobForViewer(viewerState, selectedScanId, client)
                    }
                },
                onRetryOcr = { selectedScanId ->
                    scope.launch {
                        viewerState = enqueueOcrJobForViewer(viewerState, selectedScanId, client)
                    }
                },
                onCancelOcr = {
                    scope.launch {
                        viewerState = cancelOcrJobForViewer(viewerState, client)
                    }
                },
                onSubmitOcrCorrection = { selectedScanId, correctedText ->
                    scope.launch {
                        viewerState = submitOcrCorrectionForViewer(
                            current = viewerState,
                            scanId = selectedScanId,
                            correctedText = correctedText,
                            controller = ocrCorrectionController,
                        )
                    }
                },
            )
        }
        composable(A2dDestinations.NEEDS_REVIEW) {
            NeedsReviewScreen(
                onBack = { navController.navigateUp() },
                onOpenVersions = { pageId -> navController.navigate(A2dDestinations.versionHistory(pageId)) },
            )
        }
        composable(A2dDestinations.SMART_PAGE_LIBRARY) {
            SmartPageLibraryScreen(
                onBack = { navController.navigateUp() },
                onCreateSmartPages = { navController.navigate(A2dDestinations.SMART_PAGES) },
            )
        }
        composable(A2dDestinations.IMPORTS) {
            ImportLibraryScreen(
                onBack = { navController.navigateUp() },
                onOpenPage = { pageId -> navController.navigate(A2dDestinations.pageViewer(pageId)) },
                onMoveToReview = { navController.navigate(A2dDestinations.NEEDS_REVIEW) },
            )
        }
        composable(A2dDestinations.TRASH) {
            TrashScreen(
                onBack = { navController.navigateUp() },
                onOpenPage = { pageId -> navController.navigate(A2dDestinations.pageViewer(pageId)) },
            )
        }
        composable(A2dDestinations.SINGLE_PAGE_SCANNER) {
            SinglePageScannerScreen(
                onBack = { navController.navigateUp() },
                onOpenVersions = { pageId -> navController.navigate(A2dDestinations.versionHistory(pageId)) },
            )
        }
        composable(A2dDestinations.BATCH_SCANNER) {
            PolicyAwareBatchScannerRoute(onBack = { navController.navigateUp() })
        }
        composable(A2dDestinations.NOTEBOOKS) {
            NotebookLibraryScreen(
                onBack = { navController.navigateUp() },
                onAddNotebook = { navController.navigate(A2dDestinations.ADD_NOTEBOOK) },
                onOpenNotebook = { notebookId -> navController.navigate(A2dDestinations.notebookDetail(notebookId)) },
            )
        }
        composable(
            route = A2dDestinations.NOTEBOOK_DETAIL_PATTERN,
            arguments = listOf(navArgument("notebookId") { type = NavType.StringType }),
        ) { entry ->
            NotebookDetailScreen(
                notebookId = requireNotNull(entry.arguments?.getString("notebookId")),
                onBack = { navController.navigateUp() },
                onScanPage = { navController.navigate(A2dDestinations.SINGLE_PAGE_SCANNER) },
                onBatchScan = { navController.navigate(A2dDestinations.BATCH_SCANNER) },
            )
        }
        composable(A2dDestinations.ADD_NOTEBOOK) {
            NotebookSetupScreen(
                onBack = { navController.navigateUp() },
                onResolveFirstPage = { notebookId -> navController.navigate(A2dDestinations.pageCode(notebookId)) },
            )
        }
        composable(A2dDestinations.SMART_PAGES) {
            SmartPagesScreen(onBack = { navController.navigateUp() })
        }
        composable(
            route = A2dDestinations.PAGE_CODE_PATTERN,
            arguments = listOf(navArgument("notebookId") { type = NavType.StringType }),
        ) { entry ->
            PageCodeScreen(
                notebookId = requireNotNull(entry.arguments?.getString("notebookId")),
                onBack = { navController.navigateUp() },
            )
        }
        composable(
            route = A2dDestinations.VERSION_HISTORY_PATTERN,
            arguments = listOf(navArgument("pageId") { type = NavType.StringType }),
        ) { entry ->
            VersionHistoryScreen(
                pageId = requireNotNull(entry.arguments?.getString("pageId")),
                onBack = { navController.navigateUp() },
            )
        }
    }
}

private suspend fun hydrateOcrCorrectionReviewForViewer(
    current: PageViewerState,
    scanId: String,
    controller: AndroidOcrCorrectionController?,
): PageViewerState {
    if (controller == null) {
        return current
    }
    return try {
        val correctionState = withContext(Dispatchers.IO) { controller.loadReview(scanId) }
        current.copy(
            ocrCorrectionState = correctionState,
            viewerApiConnected = true,
        )
    } catch (failure: Exception) {
        current.copy(
            ocrCorrectionState = OcrCorrectionPresentationState.error(scanId, failure.message ?: failure::class.java.simpleName),
            viewerApiConnected = true,
        )
    }
}

private suspend fun submitOcrCorrectionForViewer(
    current: PageViewerState,
    scanId: String,
    correctedText: String,
    controller: AndroidOcrCorrectionController?,
): PageViewerState {
    if (controller == null) {
        return current.copy(
            ocrCorrectionState = OcrCorrectionPresentationState.error(
                scanId,
                "No open local library is available for OCR corrections",
            ),
            viewerApiConnected = false,
        )
    }
    return try {
        val saved = withContext(Dispatchers.IO) {
            controller.submitCorrection(
                scanId = scanId,
                textRegionId = null,
                correctedText = correctedText,
                previousText = null,
            )
        }
        val correctionState =
            if (saved.errorMessage != null) {
                saved
            } else {
                withContext(Dispatchers.IO) { controller.loadReview(scanId) }
            }
        current.copy(
            ocrCorrectionState = correctionState,
            viewerApiConnected = true,
        )
    } catch (failure: Exception) {
        current.copy(
            ocrCorrectionState = OcrCorrectionPresentationState.error(scanId, failure.message ?: failure::class.java.simpleName),
            viewerApiConnected = true,
        )
    }
}

private suspend fun hydrateOcrForViewer(
    current: PageViewerState,
    scanId: String,
    client: A2dClient,
    ocrReadback: FfiAndroidOcrReadback,
): PageViewerState =
    try {
        val activeJob =
            withContext(Dispatchers.IO) {
                client.findActiveOcrJobForScan(
                    EnqueueOcrJobRequest(
                        scanId = scanId,
                        inputKind = OcrInputKind.ORIGINAL,
                        widthPx = DEFAULT_OCR_INPUT_WIDTH_PX,
                        heightPx = DEFAULT_OCR_INPUT_HEIGHT_PX,
                    ),
                )
            }
        if (activeJob != null) {
            current.copy(
                preferredScanId = scanId,
                activeOcrJobId = activeJob.jobId,
                ocrState = activeJob.toViewerOcrPresentationState(),
                viewerApiConnected = true,
            )
        } else {
            hydrateOcrReadback(current, scanId, ocrReadback)
        }
    } catch (failure: Exception) {
        current.copy(
            ocrState = failure.toOcrPresentationState(),
            activeOcrJobId = null,
            viewerApiConnected = true,
        )
    }

private suspend fun hydrateOcrReadback(
    current: PageViewerState,
    scanId: String,
    ocrReadback: FfiAndroidOcrReadback,
): PageViewerState =
    try {
        val output =
            withContext(Dispatchers.IO) {
                ocrReadback.loadLatestOcrOutput(
                    scanId = scanId,
                    regionLimit = VIEWER_OCR_REGION_LIMIT,
                )
            }
        val presentation = output.toPresentationState()
        current.copy(
            ocrState = presentation,
            ocrRegionOverlay = OcrRegionOverlayState.fromPersisted(output),
            hasRecognizedText = presentation.status == OcrPresentationStatus.Detected,
            activeOcrJobId = null,
            viewerApiConnected = true,
        )
    } catch (failure: Exception) {
        current.copy(
            ocrState = failure.toOcrPresentationState(),
            activeOcrJobId = null,
            viewerApiConnected = true,
        )
    }

private suspend fun enqueueOcrJobForViewer(
    current: PageViewerState,
    scanId: String,
    client: A2dClient?,
): PageViewerState {
    if (client == null) {
        return current.copy(
            ocrState =
                OcrPresentationState(
                    status = OcrPresentationStatus.Failed,
                    message = "No open local library is available for OCR",
                ),
            viewerApiConnected = false,
        )
    }
    return try {
        val job =
            withContext(Dispatchers.IO) {
                client.enqueueOcrJob(
                    EnqueueOcrJobRequest(
                        scanId = scanId,
                        inputKind = OcrInputKind.ORIGINAL,
                        widthPx = DEFAULT_OCR_INPUT_WIDTH_PX,
                        heightPx = DEFAULT_OCR_INPUT_HEIGHT_PX,
                    ),
                )
            }
        current.copy(
            preferredScanId = scanId,
            activeOcrJobId = job.jobId,
            ocrState = job.toViewerOcrPresentationState(),
            viewerApiConnected = true,
        )
    } catch (failure: Exception) {
        current.copy(
            ocrState = failure.toOcrPresentationState(),
            viewerApiConnected = true,
        )
    }
}

private suspend fun cancelOcrJobForViewer(
    current: PageViewerState,
    client: A2dClient?,
): PageViewerState {
    if (client == null) {
        return current.copy(
            ocrState =
                OcrPresentationState(
                    status = OcrPresentationStatus.Failed,
                    message = "No open local library is available for OCR cancellation",
                ),
            viewerApiConnected = false,
        )
    }
    val jobId = current.activeOcrJobId
        ?: return current.copy(
            ocrState =
                current.ocrState.copy(
                    status = OcrPresentationStatus.Failed,
                    message = "No active durable OCR job is available to cancel",
                    cancelAvailable = false,
                ),
            viewerApiConnected = true,
        )
    return try {
        val job = withContext(Dispatchers.IO) { client.requestOcrJobCancellation(jobId) }
        current.copy(
            activeOcrJobId = job.jobId,
            ocrState = job.toViewerOcrPresentationState(),
            viewerApiConnected = true,
        )
    } catch (failure: Exception) {
        current.copy(
            ocrState = failure.toOcrPresentationState(),
            viewerApiConnected = true,
        )
    }
}

private fun OcrQueueJob.toViewerOcrPresentationState(): OcrPresentationState =
    when (status) {
        OcrQueueJobStatus.QUEUED ->
            OcrPresentationState(
                status = OcrPresentationStatus.Preparing,
                message = "OCR queued as durable job $jobId",
                retryAvailable = false,
                cancelAvailable = true,
            )

        OcrQueueJobStatus.RUNNING ->
            OcrPresentationState(
                status = OcrPresentationStatus.Recognizing,
                message =
                    if (cancellationRequested) {
                        "OCR cancellation requested for durable job $jobId (attempt $attemptCount)"
                    } else {
                        "OCR running as durable job $jobId (attempt $attemptCount)"
                    },
                retryAvailable = false,
                cancelAvailable = !cancellationRequested,
            )

        OcrQueueJobStatus.RECOGNIZED ->
            OcrPresentationState(
                status = OcrPresentationStatus.Recording,
                runId = lastOcrRunId,
                providerLabel = provider,
                modelName = modelName,
                message = "OCR job $jobId completed; persisted OCR readback will refresh on reopen",
                retryAvailable = false,
                cancelAvailable = false,
            )

        OcrQueueJobStatus.UNAVAILABLE ->
            OcrPresentationState(
                status = OcrPresentationStatus.Unavailable,
                runId = lastOcrRunId,
                providerLabel = provider,
                modelName = modelName,
                unavailableReason = lastErrorCode,
                message = lastErrorMessage,
                retryAvailable = retryable,
                cancelAvailable = false,
            )

        OcrQueueJobStatus.CANCELLED ->
            OcrPresentationState(
                status = OcrPresentationStatus.Cancelled,
                runId = lastOcrRunId,
                providerLabel = provider,
                modelName = modelName,
                unavailableReason = lastErrorCode ?: "cancelled",
                message = lastErrorMessage ?: "OCR job was cancelled durably",
                retryAvailable = false,
                cancelAvailable = false,
            )
    }

private fun Exception.toOcrPresentationState(): OcrPresentationState =
    OcrPresentationState(
        status = OcrPresentationStatus.Failed,
        message = message ?: javaClass.simpleName,
        retryAvailable = false,
        cancelAvailable = false,
    )

private const val DEFAULT_OCR_INPUT_WIDTH_PX: UInt = 1_800u
private const val DEFAULT_OCR_INPUT_HEIGHT_PX: UInt = 2_200u
private const val VIEWER_OCR_REGION_LIMIT: UInt = 1_000u
