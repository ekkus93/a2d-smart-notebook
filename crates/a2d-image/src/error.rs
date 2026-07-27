use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

pub(crate) fn validation_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.image.invalid_input",
        message,
        false,
    )
}

pub(crate) fn processing_error(
    code: &'static str,
    message: impl Into<String>,
    retryable: bool,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::ImageProcessing,
        ErrorSeverity::Error,
        "error.image.processing",
        message,
        retryable,
    )
}
