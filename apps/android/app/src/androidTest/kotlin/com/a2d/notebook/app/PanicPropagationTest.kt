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
 * Closes the Milestone 2 gap left open in TODO 2.4: "Panics MUST be treated as defects and MUST
 * NOT cross FFI as success" (spec section 27) was previously verified only at the Rust level
 * (`a2d-ffi`'s own `#[should_panic]` test) -- proving Rust panics, which nobody doubted, not
 * that the real generated Kotlin/JNA boundary handles it correctly, which is the actual risk.
 *
 * Checked the generated bindings (`uniffi/a2d_ffi/a2d_ffi.kt`, `uniffiCheckCallStatus`) before
 * writing this: a caught Rust panic sets `UniffiRustCallStatus.code = CALL_UNEXPECTED_ERROR`,
 * which the Kotlin wrapper turns into a thrown `InternalException` carrying the panic message --
 * not a silent return and not a process abort. This test proves that's what actually happens on
 * a real device, not just what the generated code appears to do on inspection.
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
            } catch (e: InternalException) {
                e
            }

        // If we get here at all, the process didn't abort -- that's already most of the proof.
        // The rest: the call must not have looked like a silent success (an exception was
        // actually thrown), and the exception must carry the panic's own message, not a generic
        // "something went wrong" -- proving the message genuinely crossed the boundary rather
        // than being dropped.
        if (thrown == null) {
            fail(
                "expected triggerPanicForTesting() to throw InternalException; it returned " +
                    "normally instead, which would mean a Rust panic silently looked like success"
            )
        }
        assertTrue(
            "expected the panic message to cross the FFI boundary, got: ${thrown!!.message}",
            thrown.message?.contains("intentional panic from trigger_panic_for_testing") == true,
        )
    }
}
