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
mod milestone9;
pub use milestone9::*;
mod scan_policy;
pub use scan_policy::*;

#[derive(uniffi::Record)]
pub struct OpenLibraryRequest {
    pub library_path: String,
}

/// One nonsecret structured diagnostic detail attached by the Rust producer of an [`A2dError`].
///
/// A vector is used at the foreign boundary because UniFFI map support would not preserve the
/// deterministic `BTreeMap` order relied on by tests, logs, and stable presentation. Producers
/// remain responsible for the domain rule that detail values never contain secrets or raw note
/// content.
#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct A2dFfiErrorDetail {
    pub key: String,
    pub value: String,
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
    pub details: Vec<A2dFfiErrorDetail>,
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
        let details = err
            .details
            .iter()
            .map(|(key, value)| A2dFfiErrorDetail {
                key: key.clone(),
                value: value.clone(),
            })
            .collect();
        A2dFfiError::Failed(Box::new(A2dFfiErrorDetails {
            code: err.code.to_string(),
            category: format!("{:?}", err.category),
            severity: format!("{:?}", err.severity),
            user_message_key: err.user_message_key.clone(),
            developer_message: err.developer_message.clone(),
            retryable: err.retryable,
            correlation_id: err.correlation_id.clone(),
            details,
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
}

/// Intentional panic endpoint for dedicated FFI containment tests only. The normal Android and
/// future iOS libraries are built without `ffi-test-panic`, so production users cannot call or
/// discover this defect-injection API.
#[cfg(feature = "ffi-test-panic")]
#[uniffi::export]
impl A2dClient {
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
    fn parse_page_id_maps_domain_errors_and_details_to_ffi_errors() {
        let client = open_test_client();
        let err = client
            .parse_page_id("not-a-valid-id".to_string())
            .unwrap_err();
        let A2dFfiError::Failed(details) = err;
        assert!(details.code.contains("PAGE_ID"));
        assert!(!details.retryable);
        assert_eq!(
            details.details,
            vec![A2dFfiErrorDetail {
                key: "input".to_string(),
                value: "not-a-valid-id".to_string(),
            }]
        );
    }

    #[test]
    fn error_mapping_preserves_empty_details() {
        let mapped = A2dFfiError::from(A2dError::internal_unknown("no details"));
        let A2dFfiError::Failed(details) = mapped;
        assert!(details.details.is_empty());
    }

    #[test]
    fn error_mapping_preserves_multiple_details_in_btree_order() {
        let error = a2d_domain::A2dError::new(
            a2d_domain::ErrorCode::new("TEST_DETAILS"),
            a2d_domain::ErrorCategory::Integrity,
            a2d_domain::ErrorSeverity::Critical,
            "error.test_details",
            "test details",
            false,
        )
        .with_detail("zeta", "last")
        .with_detail("alpha", "first")
        .with_detail("middle", "second");
        let mapped = A2dFfiError::from(error);
        let A2dFfiError::Failed(details) = mapped;
        assert_eq!(
            details
                .details
                .iter()
                .map(|detail| detail.key.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "middle", "zeta"]
        );
        assert_eq!(details.details[0].value, "first");
        assert_eq!(details.details[1].value, "second");
        assert_eq!(details.details[2].value, "last");
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

    #[cfg(feature = "ffi-test-panic")]
    #[test]
    #[should_panic(expected = "intentional panic")]
    fn trigger_panic_for_testing_panics_at_the_rust_level() {
        let client = open_test_client();
        client.trigger_panic_for_testing();
    }
}
