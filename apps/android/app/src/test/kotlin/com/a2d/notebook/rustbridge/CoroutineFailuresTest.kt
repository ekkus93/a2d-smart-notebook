package com.a2d.notebook.rustbridge

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class CoroutineFailuresTest {
    @Test
    fun success_is_returned_unchanged() {
        val result = runBlocking {
            catchingOperationFailure { "completed" }
        }

        assertEquals("completed", result.getOrThrow())
    }

    @Test
    fun ordinary_exceptions_are_returned_as_failures() {
        val expected = IllegalStateException("bridge failed")
        val result = runBlocking {
            catchingOperationFailure<String> { throw expected }
        }

        assertTrue(result.isFailure)
        assertSame(expected, result.exceptionOrNull())
    }

    @Test
    fun cancellation_is_rethrown_instead_of_becoming_an_application_failure() {
        val expected = CancellationException("lifecycle destroyed")

        val actual = assertThrows(CancellationException::class.java) {
            runBlocking {
                catchingOperationFailure<Unit> { throw expected }
            }
        }

        assertSame(expected, actual)
    }

    @Test
    fun fatal_errors_are_not_hidden_in_a_result() {
        val expected = AssertionError("fatal")

        val actual = assertThrows(AssertionError::class.java) {
            runBlocking {
                catchingOperationFailure<Unit> { throw expected }
            }
        }

        assertSame(expected, actual)
    }
}
