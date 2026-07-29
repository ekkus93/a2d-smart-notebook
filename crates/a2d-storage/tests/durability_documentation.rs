//! Documentation drift guard for FIX-020 and FIX-021.
//!
//! This is intentionally a source-level contract test. It does not try to prove that hardware
//! honors synchronization requests; it prevents the normative durability document from drifting
//! away from the concrete filesystem ordering, platform primitives, and SQLite pragmas used by
//! `a2d-storage`.

const CONTRACT: &str = include_str!("../../../docs/decisions/V01_STORAGE_DURABILITY_CONTRACT.md");
const ASSET_IMPLEMENTATION: &str = include_str!("../src/assets.rs");
const PLATFORM_IMPLEMENTATION: &str = include_str!("../src/asset_platform.rs");
const STORAGE_IMPLEMENTATION: &str = include_str!("../src/lib.rs");

#[test]
fn durability_contract_defines_each_required_layer_separately() {
    for required_term in [
        "Userspace flush",
        "File-content synchronization",
        "Metadata synchronization",
        "Directory-entry synchronization",
        "SQLite transaction durability",
        "asset filesystem commit completed",
        "finalized but unregistered asset",
    ] {
        assert!(
            CONTRACT.contains(required_term),
            "durability contract is missing required term: {required_term}"
        );
    }

    assert!(CONTRACT.contains(
        "does **not** describe the combined file-plus-database operation as fully power-loss durable"
    ));
    assert!(CONTRACT.contains("WAL plus `synchronous=NORMAL`"));
    assert!(CONTRACT.contains("A successful `flush()` is never sufficient"));
}

#[test]
fn asset_implementation_retains_the_documented_filesystem_ordering_markers() {
    for required_source_marker in [
        "file.flush()",
        "file.sync_all()",
        "platform::finalize_no_replace(&tmp_path, &final_path)",
        "verify_finalized_metadata(&final_path, byte_length, immutable)",
        "platform::sync_directory(&destination_directory)",
        "std::fs::remove_file(&tmp_path)",
        "platform::sync_directory(&temp_directory)",
    ] {
        assert!(
            ASSET_IMPLEMENTATION.contains(required_source_marker),
            "asset implementation no longer contains documented ordering marker: {required_source_marker}"
        );
    }
}

#[test]
fn platform_adapter_retains_explicit_no_replace_and_unsupported_semantics() {
    assert!(PLATFORM_IMPLEMENTATION.contains("target_os = \"android\""));
    assert!(PLATFORM_IMPLEMENTATION.contains("target_os = \"linux\""));
    assert!(PLATFORM_IMPLEMENTATION.contains("std::fs::hard_link(temp_path, final_path)"));
    assert!(PLATFORM_IMPLEMENTATION.contains("ErrorKind::Unsupported"));
    assert!(!PLATFORM_IMPLEMENTATION.contains("std::fs::rename"));
}

#[test]
fn sqlite_configuration_matches_the_documented_transaction_contract() {
    assert!(STORAGE_IMPLEMENTATION.contains("pragma_update(None, \"journal_mode\", \"WAL\")"));
    assert!(STORAGE_IMPLEMENTATION.contains("pragma_update(None, \"synchronous\", \"NORMAL\")"));
    assert!(STORAGE_IMPLEMENTATION.contains("TransactionBehavior::Immediate"));
}
