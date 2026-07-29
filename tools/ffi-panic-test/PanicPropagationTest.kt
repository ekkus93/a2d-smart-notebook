package com.a2d.notebook.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.a2d.notebook.rustbridge.A2dBridge
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.a2d_ffi.InternalException

/**
 * CI-only defect-injection test. The emulator job copies this file into androidTest only after it
 * builds `a2d-ffi` and regenerates Kotlin with the explicit `ffi-test-panic` Cargo feature.
 * Production Android libraries and committed bindings omit the intentional panic method.
 */
@RunWith(AndroidJUnit4::class)
class PanicPropagationTest {
    @Test
    fun aRustPanicSurfacesAsAKotlinExceptionRatherThanASilentSuccessOrACrash() {
        val client =
            A2dBridge.client(InstrumentationRegistry.getInstrumentation().targetContext)

        val thrown =
            try {
                client.triggerPanicForTesting()
                null
            } catch (error: InternalException) {
                error
            }

        if (thrown == null) {
            fail(
                "expected triggerPanicForTesting() to throw InternalException; it returned " +
                    "normally instead, which would mean a Rust panic silently looked like success",
            )
        }
        assertTrue(
            "expected the panic message to cross the FFI boundary, got: ${thrown!!.message}",
            thrown.message?.contains("intentional panic from trigger_panic_for_testing") == true,
        )
    }
}
