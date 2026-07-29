package com.a2d.notebook.rustbridge

import kotlinx.coroutines.CancellationException

/**
 * Executes one suspending platform/Rust bridge operation without converting coroutine cancellation
 * into an ordinary application failure.
 *
 * Kotlin's standard [runCatching] catches every [Throwable], including [CancellationException] and
 * fatal JVM errors. ViewModels must not display lifecycle cancellation as a user-visible failure,
 * and they must not hide fatal errors. This helper catches only ordinary [Exception] values after
 * rethrowing cancellation.
 */
internal suspend fun <T> catchingOperationFailure(
    block: suspend () -> T,
): Result<T> = try {
    Result.success(block())
} catch (cancellation: CancellationException) {
    throw cancellation
} catch (failure: Exception) {
    Result.failure(failure)
}
