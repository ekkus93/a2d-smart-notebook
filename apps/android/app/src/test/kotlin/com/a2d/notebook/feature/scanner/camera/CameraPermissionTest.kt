package com.a2d.notebook.feature.scanner.camera

import org.junit.Assert.assertEquals
import org.junit.Test

class CameraPermissionTest {
    @Test
    fun grantedPermissionAlwaysWins() {
        assertEquals(
            CameraPermissionStatus.Granted,
            classifyCameraPermission(
                granted = true,
                hasRequested = true,
                shouldShowRationale = true,
            ),
        )
    }

    @Test
    fun permissionStartsAsNotRequested() {
        assertEquals(
            CameraPermissionStatus.NotRequested,
            classifyCameraPermission(
                granted = false,
                hasRequested = false,
                shouldShowRationale = false,
            ),
        )
    }

    @Test
    fun deniedPermissionRemainsRetryableWhenRationaleIsAvailable() {
        assertEquals(
            CameraPermissionStatus.Denied,
            classifyCameraPermission(
                granted = false,
                hasRequested = true,
                shouldShowRationale = true,
            ),
        )
    }

    @Test
    fun deniedPermissionBecomesSettingsOnlyWhenTheSystemWillNotPromptAgain() {
        assertEquals(
            CameraPermissionStatus.PermanentlyDenied,
            classifyCameraPermission(
                granted = false,
                hasRequested = true,
                shouldShowRationale = false,
            ),
        )
    }
}