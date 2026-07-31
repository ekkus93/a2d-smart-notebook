//! Source/documentation drift guard for the FIX-024 path-resolution contract.

#[test]
fn path_resolution_implementation_and_documented_limitations_stay_aligned() {
    let source = include_str!("../src/assets.rs");
    let contract = include_str!("../../../docs/decisions/V01_ASSET_PATH_RESOLUTION_CONTRACT.md");

    for required_source_fragment in [
        "std::fs::symlink_metadata(&candidate)",
        "metadata.file_type().is_symlink()",
        "!metadata.is_file()",
        "candidate.canonicalize()",
        "!canonical_candidate.starts_with(&canonical_root)",
        "Ok(canonical_candidate)",
    ] {
        assert!(
            source.contains(required_source_fragment),
            "asset path resolution drifted from the documented validation sequence: {required_source_fragment}",
        );
    }

    for required_contract_fragment in [
        "returns the canonical absolute candidate that it validated",
        "Path-based reopen and bounded TOCTOU limitation",
        "does not claim to eliminate",
        "malicious same-UID process",
        "validated-handle API",
    ] {
        assert!(
            contract.contains(required_contract_fragment),
            "asset path resolution documentation lost a required limitation or future-hardening statement: {required_contract_fragment}",
        );
    }
}
