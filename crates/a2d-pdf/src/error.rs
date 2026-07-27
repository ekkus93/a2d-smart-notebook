//! Typed errors for PDF generation (TODO 5.4/5.5), mapped to the categories that best describe
//! each failure class rather than one generic catch-all.

use a2d_domain::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

fn pdf_error(code: &'static str, category: ErrorCategory, message: impl Into<String>) -> A2dError {
    A2dError::new(
        code_wrap(code),
        category,
        ErrorSeverity::Error,
        "error.pdf.generation_failed",
        message.into(),
        false,
    )
}

fn code_wrap(code: &'static str) -> ErrorCode {
    ErrorCode::new(code)
}

/// The QR payload could not be encoded (e.g. it exceeds QR's maximum capacity) -- caller-supplied
/// input, not an infrastructure failure.
pub(crate) fn qr_encode_error(message: impl Into<String>) -> A2dError {
    pdf_error("PDF_QR_ENCODE_FAILED", ErrorCategory::Validation, message)
}

/// Writing the generated PDF bytes to disk failed.
pub(crate) fn io_error(message: impl Into<String>) -> A2dError {
    pdf_error("PDF_IO_FAILED", ErrorCategory::Storage, message)
}

/// The PDF this process just wrote could not be re-parsed -- the generated output itself is
/// suspect, which is an integrity concern, not an ordinary I/O failure (TODO 5.4 "write to a
/// temp path and verify before returning success").
pub(crate) fn verify_error(message: impl Into<String>) -> A2dError {
    pdf_error("PDF_VERIFY_FAILED", ErrorCategory::Integrity, message)
}
