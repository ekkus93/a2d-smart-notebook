//! Fallible portable time primitives for canonical timestamps.
//!
//! Production code must never invent Unix timestamp zero when the platform clock is unavailable,
//! before the Unix epoch, or outside the stored `i64` millisecond representation. Callers receive a
//! typed [`A2dError`] and can roll back or preserve staged recovery data explicitly.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

/// A portable clock interface for workflows that need an authoritative canonical timestamp.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> Result<i64, A2dError>;
}

/// The operating system wall clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> Result<i64, A2dError> {
        unix_millis(SystemTime::now())
    }
}

/// Returns the current Unix timestamp in milliseconds or a typed platform/time error.
pub fn system_now_ms() -> Result<i64, A2dError> {
    SystemClock.now_ms()
}

/// Converts a supplied wall-clock value to the canonical Unix-millisecond representation. This is
/// public so deterministic tests and future injected platform clocks can use the exact same checked
/// conversion as [`SystemClock`].
pub fn unix_millis(time: SystemTime) -> Result<i64, A2dError> {
    let duration = time.duration_since(UNIX_EPOCH).map_err(|error| {
        time_error(
            "TIME_SOURCE_BEFORE_UNIX_EPOCH",
            "system clock is before the Unix epoch",
        )
        .with_detail("before_epoch_ms", error.duration().as_millis().to_string())
    })?;
    duration_millis(duration)
}

fn duration_millis(duration: Duration) -> Result<i64, A2dError> {
    i64::try_from(duration.as_millis()).map_err(|_| {
        time_error(
            "TIME_VALUE_OVERFLOW",
            "Unix timestamp milliseconds exceed the canonical i64 representation",
        )
        .with_detail("milliseconds", duration.as_millis().to_string())
    })
}

fn time_error(code: &'static str, message: &'static str) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::PlatformAdapter,
        ErrorSeverity::Critical,
        "error.time.source_invalid",
        message,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_positive_time_convert_exactly() {
        assert_eq!(unix_millis(UNIX_EPOCH).unwrap(), 0);
        assert_eq!(
            unix_millis(UNIX_EPOCH + Duration::from_millis(1_234)).unwrap(),
            1_234,
        );
    }

    #[test]
    fn pre_epoch_time_fails_without_inventing_zero() {
        let error = unix_millis(UNIX_EPOCH - Duration::from_millis(1)).unwrap_err();
        assert_eq!(error.code.to_string(), "TIME_SOURCE_BEFORE_UNIX_EPOCH");
        assert_eq!(
            error.details.get("before_epoch_ms").map(String::as_str),
            Some("1"),
        );
    }

    #[test]
    fn millisecond_overflow_is_explicit() {
        let overflowing_seconds = (i64::MAX as u64 / 1_000) + 1;
        let error = duration_millis(Duration::from_secs(overflowing_seconds)).unwrap_err();
        assert_eq!(error.code.to_string(), "TIME_VALUE_OVERFLOW");
        assert!(error.details.contains_key("milliseconds"));
    }

    #[test]
    fn injected_clock_can_fail_without_a_fallback_value() {
        struct FailingClock;

        impl Clock for FailingClock {
            fn now_ms(&self) -> Result<i64, A2dError> {
                Err(time_error("TIME_TEST_FAILURE", "injected clock failure"))
            }
        }

        let error = FailingClock.now_ms().unwrap_err();
        assert_eq!(error.code.to_string(), "TIME_TEST_FAILURE");
    }
}
