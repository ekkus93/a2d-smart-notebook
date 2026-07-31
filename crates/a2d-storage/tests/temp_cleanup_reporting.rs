//! FIX-023 regression coverage for temporary asset cleanup failure reporting.
//!
//! The cleanup helper is intentionally private because production callers must use the complete
//! asset commit protocol. This source-level drift test protects the externally observable error
//! contract without exposing a test-only cleanup primitive through the portable storage API.

#[test]
fn production_temp_cleanup_failures_remain_explicit_secondary_error_details() {
    let source = include_str!("../src/assets.rs");

    assert!(
        source.contains("fn with_cleanup_result(error: A2dError, tmp_path: &Path) -> A2dError"),
        "the centralized cleanup-result helper must remain present",
    );
    assert!(
        source.contains("match std::fs::remove_file(tmp_path)"),
        "temporary file removal must be handled explicitly",
    );
    assert!(
        source.contains(".with_detail(\"temp_path\", tmp_path.to_string_lossy())"),
        "cleanup failures must identify the retained temporary path",
    );
    assert!(
        source.contains(".with_detail(\"temp_cleanup_completed\", \"false\")"),
        "cleanup failure must remain distinguishable from successful cleanup",
    );
    assert!(
        source.contains(".with_detail(\"temp_cleanup_error\", cleanup_error.to_string())"),
        "the cleanup error must be attached as structured secondary evidence",
    );
    assert!(
        !source.contains("remove_file(tmp_path).ok()")
            && !source.contains("remove_file(&tmp_path).ok()"),
        "production cleanup errors must never be discarded with Result::ok",
    );
}

#[test]
fn primary_commit_error_is_preserved_while_cleanup_evidence_is_attached() {
    let source = include_str!("../src/assets.rs");
    let helper_start = source
        .find("fn with_cleanup_result(error: A2dError, tmp_path: &Path) -> A2dError")
        .expect("cleanup helper must exist");
    let helper = &source[helper_start..];

    assert!(
        helper.contains("Ok(()) => error")
            && helper.contains("Err(cleanup_error) if cleanup_error.kind() == io::ErrorKind::NotFound => error")
            && helper.contains("Err(cleanup_error) => error"),
        "all cleanup outcomes must augment and return the original primary A2dError",
    );
    assert!(
        !helper.contains("A2dError::new("),
        "cleanup reporting must not replace the primary failure with a new cleanup error",
    );
}
