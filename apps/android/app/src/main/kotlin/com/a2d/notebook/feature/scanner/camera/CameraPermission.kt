package com.a2d.notebook.feature.scanner.camera

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.ContextWrapper
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner

enum class CameraPermissionStatus {
    Granted,
    NotRequested,
    Denied,
    PermanentlyDenied,
}

data class CameraPermissionState(
    val status: CameraPermissionStatus,
    val requestPermission: () -> Unit,
    val openApplicationSettings: () -> Unit,
)

internal fun classifyCameraPermission(
    granted: Boolean,
    hasRequested: Boolean,
    shouldShowRationale: Boolean,
): CameraPermissionStatus = when {
    granted -> CameraPermissionStatus.Granted
    !hasRequested -> CameraPermissionStatus.NotRequested
    shouldShowRationale -> CameraPermissionStatus.Denied
    else -> CameraPermissionStatus.PermanentlyDenied
}

@Composable
fun rememberCameraPermissionState(): CameraPermissionState {
    val context = LocalContext.current
    val lifecycleOwner = LocalLifecycleOwner.current
    val activity = remember(context) { context.findActivity() }
    var hasRequested by rememberSaveable { mutableStateOf(false) }
    var granted by remember {
        mutableStateOf(context.hasCameraPermission())
    }

    val launcher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission(),
    ) { result ->
        hasRequested = true
        granted = result
    }

    DisposableEffect(context, lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) {
                granted = context.hasCameraPermission()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    val shouldShowRationale = activity?.let {
        ActivityCompat.shouldShowRequestPermissionRationale(it, Manifest.permission.CAMERA)
    } ?: false
    val status = classifyCameraPermission(granted, hasRequested, shouldShowRationale)

    return CameraPermissionState(
        status = status,
        requestPermission = {
            hasRequested = true
            launcher.launch(Manifest.permission.CAMERA)
        },
        openApplicationSettings = {
            context.startActivity(
                Intent(
                    Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                    Uri.fromParts("package", context.packageName, null),
                ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            )
        },
    )
}

private fun Context.hasCameraPermission(): Boolean = ContextCompat.checkSelfPermission(
    this,
    Manifest.permission.CAMERA,
) == PackageManager.PERMISSION_GRANTED

private tailrec fun Context.findActivity(): Activity? = when (this) {
    is Activity -> this
    is ContextWrapper -> baseContext.findActivity()
    else -> null
}