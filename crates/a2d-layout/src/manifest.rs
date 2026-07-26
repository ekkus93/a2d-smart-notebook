//! Notebook Design manifests (TODO 4.4, spec §11.3/§15.1): a versioned, offline-resolvable
//! description of a Notebook Design's physical dimensions, layout IDs, marker family/roles,
//! logical page count, and content hash, loaded from a bundled JSON resource and turned into a
//! canonical [`NotebookDesign`].
//!
//! This module builds the *mechanism* only. No real physical Notebook Design exists yet — trim
//! size, the marker family, and the real page layouts are all Milestone 5 decisions — so the one
//! manifest bundled here ([`bundled_placeholder_registry`]) is explicitly a development
//! placeholder, not an official design. Milestone 5 replaces it with the first real manifest;
//! this registry mechanism does not need to change when that happens.
//!
//! `trust_state` is deliberately **not** a field in the manifest JSON itself: a manifest
//! shouldn't be able to self-declare its own trust, since a tampered or hostile manifest could
//! just claim `"Trusted"`. Trust is assigned by the loader based on how the manifest was
//! obtained (bundled with a reviewed app build vs. a future signed/imported design), leaving room
//! for spec §14.4's future signed-manifest extension without changing this shape.

use std::collections::HashMap;

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesign, NotebookDesignId,
    TrimSizeMm, TrustState,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The newest manifest schema version this build understands. A manifest declaring a higher
/// `schema_version` is rejected outright (TODO 4.4 "reject unsupported required versions") —
/// never partially interpreted against an older, possibly-incompatible grammar.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The on-disk/bundled JSON shape. Kept separate from [`NotebookDesign`] itself so the wire
/// format (what a manifest author writes) can evolve independently of the domain entity's own
/// field layout.
#[derive(Deserialize)]
struct RawManifest {
    schema_version: u32,
    id: String,
    name: String,
    design_version: u32,
    trim_width_mm: u32,
    trim_height_mm: u32,
    logical_page_count: u32,
    setup_layout_id: String,
    page_layout_id: String,
    marker_family: String,
    marker_role_ids: Vec<String>,
}

fn manifest_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::UnsupportedFormat,
        ErrorSeverity::Error,
        "error.layout.manifest_invalid",
        message.into(),
        false,
    )
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Parses a Notebook Design manifest from its bundled JSON text, computing its content hash and
/// assigning `trust_state` from the caller-supplied loading context (see module docs for why
/// trust isn't embedded in the manifest itself).
pub fn parse_manifest(json: &str, trust_state: TrustState) -> Result<NotebookDesign, A2dError> {
    let raw: RawManifest = serde_json::from_str(json)
        .map_err(|e| manifest_error("MANIFEST_INVALID_JSON", format!("{e}")))?;

    if raw.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(manifest_error(
            "MANIFEST_UNSUPPORTED_SCHEMA_VERSION",
            format!(
                "manifest declares schema_version {}, this build only understands up to {}",
                raw.schema_version, CURRENT_SCHEMA_VERSION
            ),
        )
        .with_detail("schema_version", raw.schema_version.to_string()));
    }
    if raw.schema_version == 0 {
        return Err(manifest_error(
            "MANIFEST_UNSUPPORTED_SCHEMA_VERSION",
            "schema_version 0 is not a valid manifest version",
        ));
    }

    let id = NotebookDesignId::parse(&raw.id)?;
    let setup_layout_id = LayoutId::parse(&raw.setup_layout_id)?;
    let page_layout_id = LayoutId::parse(&raw.page_layout_id)?;
    if raw.marker_role_ids.is_empty() {
        return Err(manifest_error(
            "MANIFEST_NO_MARKER_ROLES",
            "manifest must declare at least one marker_role_id",
        ));
    }
    if raw.logical_page_count == 0 {
        return Err(manifest_error(
            "MANIFEST_ZERO_LOGICAL_PAGE_COUNT",
            "manifest must declare a positive logical_page_count",
        ));
    }

    let manifest_hash = hex_sha256(json.as_bytes());

    Ok(NotebookDesign::new(
        id,
        raw.schema_version,
        raw.name,
        raw.design_version,
        TrimSizeMm {
            width: raw.trim_width_mm,
            height: raw.trim_height_mm,
        },
        raw.logical_page_count,
        setup_layout_id,
        page_layout_id,
        raw.marker_family,
        raw.marker_role_ids,
        manifest_hash,
        trust_state,
    ))
}

/// An offline, in-memory lookup from `NotebookDesignId` to its resolved manifest (spec §7.2 "App
/// resolves the Notebook Design", §14.4 "resolved fully offline"). No network access, no lazy
/// loading — every manifest the registry can resolve was parsed and validated up front.
#[derive(Default)]
pub struct ManifestRegistry {
    by_id: HashMap<NotebookDesignId, NotebookDesign>,
}

impl ManifestRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Inserts a parsed design. Rejects a second manifest claiming an `id` already present —
    /// two bundled manifests sharing an id is a build-time content defect, not a runtime
    /// collision, but it still must fail loudly rather than silently letting the second
    /// manifest shadow the first.
    pub fn insert(&mut self, design: NotebookDesign) -> Result<(), A2dError> {
        if self.by_id.contains_key(design.id()) {
            return Err(manifest_error(
                "MANIFEST_DUPLICATE_DESIGN_ID",
                format!(
                    "more than one bundled manifest declares design id {}",
                    design.id()
                ),
            ));
        }
        self.by_id.insert(design.id().clone(), design);
        Ok(())
    }

    pub fn resolve(&self, id: &NotebookDesignId) -> Option<&NotebookDesign> {
        self.by_id.get(id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

/// The one manifest bundled with this build today: an explicit development placeholder (see
/// module docs), not an official Notebook Design. Loaded as `Trusted` because it ships inside
/// this reviewed build's binary, the same basis v0.1 has for trusting any bundled resource —
/// distinct from a future signed/imported manifest, which would derive trust from signature
/// verification instead.
pub fn bundled_placeholder_registry() -> Result<ManifestRegistry, A2dError> {
    const PLACEHOLDER_JSON: &str = include_str!("../manifests/dev-placeholder.json");
    let mut registry = ManifestRegistry::empty();
    registry.insert(parse_manifest(PLACEHOLDER_JSON, TrustState::Trusted)?)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_json(id: &str, schema_version: u32) -> String {
        format!(
            r#"{{
                "schema_version": {schema_version},
                "id": "{id}",
                "name": "Test Notebook",
                "design_version": 1,
                "trim_width_mm": 210,
                "trim_height_mm": 297,
                "logical_page_count": 100,
                "setup_layout_id": "SETUP-V1",
                "page_layout_id": "PAGE-V1",
                "marker_family": "apriltag",
                "marker_role_ids": ["TL", "TR", "BL", "BR"]
            }}"#
        )
    }

    #[test]
    fn parses_a_well_formed_manifest_into_a_notebook_design() {
        let id = NotebookDesignId::generate();
        let json = manifest_json(&id.to_string(), 1);
        let design = parse_manifest(&json, TrustState::Trusted).unwrap();
        assert_eq!(design.id(), &id);
        assert_eq!(design.name, "Test Notebook");
        assert_eq!(design.logical_page_count, 100);
        assert_eq!(design.marker_role_ids, vec!["TL", "TR", "BL", "BR"]);
        assert_eq!(design.trust_state, TrustState::Trusted);
        assert_eq!(
            design.manifest_hash.len(),
            64,
            "expected a hex sha256 digest"
        );
    }

    #[test]
    fn the_same_manifest_text_always_hashes_the_same() {
        let id = NotebookDesignId::generate();
        let json = manifest_json(&id.to_string(), 1);
        let a = parse_manifest(&json, TrustState::Trusted).unwrap();
        let b = parse_manifest(&json, TrustState::Trusted).unwrap();
        assert_eq!(a.manifest_hash, b.manifest_hash);
    }

    #[test]
    fn different_manifest_text_hashes_differently() {
        let a = parse_manifest(
            &manifest_json(&NotebookDesignId::generate().to_string(), 1),
            TrustState::Trusted,
        )
        .unwrap();
        let b = parse_manifest(
            &manifest_json(&NotebookDesignId::generate().to_string(), 1),
            TrustState::Trusted,
        )
        .unwrap();
        assert_ne!(a.manifest_hash, b.manifest_hash);
    }

    #[test]
    fn rejects_an_unsupported_future_schema_version() {
        let json = manifest_json(
            &NotebookDesignId::generate().to_string(),
            CURRENT_SCHEMA_VERSION + 1,
        );
        let err = parse_manifest(&json, TrustState::Trusted).unwrap_err();
        assert!(err.code.to_string().contains("UNSUPPORTED_SCHEMA_VERSION"));
    }

    #[test]
    fn rejects_schema_version_zero() {
        let json = manifest_json(&NotebookDesignId::generate().to_string(), 0);
        let err = parse_manifest(&json, TrustState::Trusted).unwrap_err();
        assert!(err.code.to_string().contains("UNSUPPORTED_SCHEMA_VERSION"));
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_manifest("not json", TrustState::Trusted).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_JSON"));
    }

    #[test]
    fn rejects_an_invalid_design_id() {
        let json = manifest_json("TOOSHORT", 1);
        let err = parse_manifest(&json, TrustState::Trusted).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_LENGTH"));
    }

    #[test]
    fn rejects_an_empty_marker_role_list() {
        let id = NotebookDesignId::generate();
        let json = format!(
            r#"{{
                "schema_version": 1,
                "id": "{id}",
                "name": "Test Notebook",
                "design_version": 1,
                "trim_width_mm": 210,
                "trim_height_mm": 297,
                "logical_page_count": 100,
                "setup_layout_id": "SETUP-V1",
                "page_layout_id": "PAGE-V1",
                "marker_family": "apriltag",
                "marker_role_ids": []
            }}"#
        );
        let err = parse_manifest(&json, TrustState::Trusted).unwrap_err();
        assert!(err.code.to_string().contains("NO_MARKER_ROLES"));
    }

    #[test]
    fn registry_resolves_an_inserted_design_and_returns_none_for_an_unknown_id() {
        let id = NotebookDesignId::generate();
        let design =
            parse_manifest(&manifest_json(&id.to_string(), 1), TrustState::Trusted).unwrap();
        let mut registry = ManifestRegistry::empty();
        registry.insert(design).unwrap();

        assert_eq!(registry.resolve(&id).map(|d| d.id().clone()), Some(id));
        assert!(registry.resolve(&NotebookDesignId::generate()).is_none());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn registry_rejects_a_second_manifest_reusing_an_existing_design_id() {
        let id = NotebookDesignId::generate();
        let first =
            parse_manifest(&manifest_json(&id.to_string(), 1), TrustState::Trusted).unwrap();
        let mut second_json_value: serde_json::Value =
            serde_json::from_str(&manifest_json(&id.to_string(), 1)).unwrap();
        second_json_value["name"] = serde_json::Value::String("Different Name".to_string());
        let second = parse_manifest(&second_json_value.to_string(), TrustState::Trusted).unwrap();

        let mut registry = ManifestRegistry::empty();
        registry.insert(first).unwrap();
        let err = registry.insert(second).unwrap_err();
        assert!(err.code.to_string().contains("DUPLICATE_DESIGN_ID"));
    }

    #[test]
    fn bundled_placeholder_registry_resolves_offline_without_touching_the_network() {
        let registry = bundled_placeholder_registry().unwrap();
        assert_eq!(registry.len(), 1);
    }
}
