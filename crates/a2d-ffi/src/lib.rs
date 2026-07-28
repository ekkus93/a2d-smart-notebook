//! Thin UniFFI boundary. Carries file paths and owned buffers only; no SQL or business rules.
//!
//! `#[uniffi::export]` was chosen over UDL (TODO 2.4's open decision): it keeps the interface
//! definition next to the Rust code it describes instead of duplicated in a separate `.udl`
//! file, and is UniFFI's current recommended default. Every exported operation here maps
//! directly to an already-real shared Rust operation — nothing here fakes a use case that
//! doesn't exist yet.

use std::fmt;
use std::sync::Arc;

use a2d_domain::A2dError;

uniffi::setup_scaffolding!();

mod milestone6;
pub use milestone6::*;
mod milestone7;
pub use milestone7::*;
mod live_analysis;
pub use live_analysis::*;
mod preview_processing;
pub use preview_processing::*;

#[derive(uniffi::Record)]
pub struct OpenLibraryRequest {
    pub library_path: String,
}

/// The FFI-safe projection of [`A2dError`]'s fields. Boxed inside [`A2dFfiError`] for the same
/// reason `A2dError` itself boxes its fields (clippy's `result_large_err`) — kept as a separate
/// `Record` rather than flattening it into the enum variant so the reason is visible at the
/// type level, not just via a `Box<...>` in the variant.
///
/// Enum fields are stringified (`{:?}`) rather than re-exposing `ErrorCategory`/`ErrorSeverity`
/// as UniFFI enums, to avoid keeping two copies of the same taxonomy in sync across the boundary;
/// revisit if Kotlin/Swift callers need to branch on category/severity programmatically rather
/// than just display them.
#[derive(Debug, uniffi::Record)]
pub struct A2dFfiErrorDetails {
    pub code: String,
    pub category: String,
    pub severity: String,
    pub user_message_key: String,
    pub developer_message: String,
    pub retryable: bool,
    pub correlation_id: String,
}

#[derive(Debug, uniffi::Error)]
pub enum A2dFfiError {
    Failed(Box<A2dFfiErrorDetails>),
}

impl fmt::Display for A2dFfiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            A2dFfiError::Failed(details) => write!(
                f,
                "{} [{}]: {}",
                details.code, details.correlation_id, details.developer_message
            ),
        }
    }
}

impl std::error::Error for A2dFfiError {}

impl From<A2dError> for A2dFfiError {
    fn from(err: A2dError) -> Self {
        A2dFfiError::Failed(Box::new(A2dFfiErrorDetails {
            code: err.code.to_string(),
            category: format!("{:?}", err.category),
            severity: format!("{:?}", err.severity),
            user_message_key: err.user_message_key.clone(),
            developer_message: err.developer_message.clone(),
            retryable: err.retryable,
            correlation_id: err.correlation_id.clone(),
        }))
    }
}

#[derive(Debug, uniffi::Object)]
pub struct A2dClient {
    core: Arc<a2d_core::A2dCore>,
}

#[uniffi::export]
impl A2dClient {
    #[uniffi::constructor]
    pub fn open(request: OpenLibraryRequest) -> Result<Arc<Self>, A2dFfiError> {
        let core = a2d_core::A2dCore::open(a2d_core::OpenLibraryRequest {
            library_path: request.library_path,
        })?;
        Ok(Arc::new(Self { core }))
    }

    pub fn library_path(&self) -> String {
        self.core.library_path()
    }

    pub fn generate_page_id(&self) -> String {
        self.core.generate_page_id()
    }

    pub fn parse_page_id(&self, candidate: String) -> Result<String, A2dFfiError> {
        self.core.parse_page_id(&candidate).map_err(Into::into)
    }

    /// Real (freshly random), not hardcoded, example v1 QR payloads (ADR 0001) for each code
    /// type — for the ADR's own required Android decoder spike to render and decode.
    pub fn generate_example_notebook_setup_qr_payload(&self) -> Result<String, A2dFfiError> {
        self.core
            .generate_example_notebook_setup_qr_payload()
            .map_err(Into::into)
    }

    pub fn generate_example_notebook_page_qr_payload(&self) -> Result<String, A2dFfiError> {
        self.core
            .generate_example_notebook_page_qr_payload()
            .map_err(Into::into)
    }

    pub fn generate_example_smart_page_qr_payload(&self) -> Result<String, A2dFfiError> {
        self.core
            .generate_example_smart_page_qr_payload()
            .map_err(Into::into)
    }

    /// Exists only so a test can demonstrate a Rust panic doesn't silently look like a
    /// successful FFI call (spec §27: "Panics MUST be treated as defects and MUST NOT cross FFI
    /// as success"). UniFFI's generated scaffolding catches unwinds at the `extern "C"`
    /// boundary; fully proving that requires calling through real generated Kotlin/Swift
    /// bindings, which don't exist yet (no Android/iOS project — Milestone 1.2/15). The test
    /// here only proves the Rust-level behavior this relies on.
    pub fn trigger_panic_for_testing(&self) {
        panic!("intentional panic from trigger_panic_for_testing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_client() -> Arc<A2dClient> {
        let dir =
            std::env::temp_dir().join(format!("a2d-ffi-test-{}", a2d_domain::PageId::generate()));
        A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .expect("open must succeed for a fresh directory")
    }

    #[test]
    fn open_generate_and_parse_round_trip_through_the_ffi_types() {
        let client = open_test_client();
        let generated = client.generate_page_id();
        let parsed = client
            .parse_page_id(generated.clone())
            .expect("must parse its own output");
        assert_eq!(generated, parsed);
    }

    #[test]
    fn parse_page_id_maps_domain_errors_to_ffi_errors() {
        let client = open_test_client();
        let err = client
            .parse_page_id("not-a-valid-id".to_string())
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;
        assert!(details.code.contains("PAGE_ID"));
        assert!(!details.retryable);
    }

    #[test]
    fn example_qr_payload_methods_cross_the_ffi_wrapper_and_round_trip_as_typed_codes() {
        let client = open_test_client();

        let setup_a = client.generate_example_notebook_setup_qr_payload().unwrap();
        let setup_b = client.generate_example_notebook_setup_qr_payload().unwrap();
        assert_ne!(
            setup_a, setup_b,
            "each wrapper call must preserve fresh IDs"
        );
        assert!(matches!(
            a2d_identity::qr::parse(&setup_a, |_| true).unwrap(),
            a2d_identity::PageCode::NotebookSetup { .. }
        ));

        let notebook_page = client.generate_example_notebook_page_qr_payload().unwrap();
        match a2d_identity::qr::parse(&notebook_page, |_| true).unwrap() {
            a2d_identity::PageCode::NotebookPage {
                logical_page_number,
                layout_id,
                ..
            } => {
                assert_eq!(logical_page_number, 12);
                assert_eq!(layout_id.as_str(), "USLETTER-LINED");
            }
            other => panic!("expected NotebookPage through FFI wrapper, got {other:?}"),
        }

        let smart_page = client.generate_example_smart_page_qr_payload().unwrap();
        match a2d_identity::qr::parse(&smart_page, |_| true).unwrap() {
            a2d_identity::PageCode::SmartPage {
                layout_id,
                visible_page_number,
                page_set_id,
                ..
            } => {
                assert_eq!(layout_id.as_str(), "USLETTER-LINED");
                assert_eq!(visible_page_number, Some(3));
                assert_eq!(page_set_id, None);
            }
            other => panic!("expected SmartPage through FFI wrapper, got {other:?}"),
        }
    }

    #[test]
    fn open_rejects_a_path_that_is_a_file_with_a_mapped_error() {
        let file = std::env::temp_dir().join(format!(
            "a2d-ffi-test-file-{}",
            a2d_domain::PageId::generate()
        ));
        std::fs::write(&file, b"not a directory").expect("test setup must be able to write");
        let err = A2dClient::open(OpenLibraryRequest {
            library_path: file.to_string_lossy().into_owned(),
        })
        .unwrap_err();
        let A2dFfiError::Failed(details) = err;
        assert_eq!(details.category, "Storage");
        std::fs::remove_file(&file).ok();
    }

    #[test]
    #[should_panic(expected = "intentional panic")]
    fn trigger_panic_for_testing_panics_at_the_rust_level() {
        let client = open_test_client();
        client.trigger_panic_for_testing();
    }
}
