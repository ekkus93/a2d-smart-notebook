//! Notebook Design manifests (TODO 4.4, spec §11.3/§15.1): a versioned, offline-resolvable
//! description of a Notebook Design's physical dimensions, layout IDs, marker family/roles,
//! logical page count, and content hash, loaded from a bundled JSON resource and turned into a
//! canonical [`NotebookDesign`].
//!
//! This module builds the *mechanism* only. No real physical Notebook Design exists yet — the one
//! manifest bundled here ([`bundled_placeholder_registry`]) is explicitly a development
//! placeholder, not an official design. Physical validation and an official reviewed manifest are
//! still required before release claims can be made.
//!
//! `trust_state` is deliberately **not** a field in the manifest JSON itself: a manifest must not
//! self-declare trust. Trust is assigned by the loader based on how the manifest was obtained.
//!
//! `manifest_hash` is the SHA-256 of the exact source bytes supplied to [`parse_manifest`]. It is
//! intentionally not a semantic/canonical-JSON hash: whitespace or key-order changes create a new
//! hash even when all parsed fields are equivalent.

use std::collections::{BTreeSet, HashMap};

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesign, NotebookDesignId,
    TrimSizeMm, TrustState,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::notebook::{setup_page_layout, writable_page_layout};
use crate::page_layout::PageLayout;

/// The newest manifest schema version this build understands.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;
/// Parsing is rejected before JSON allocation when the UTF-8 source exceeds this limit.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
/// Portable v0.1 bound for a Notebook Design's logical writable pages.
pub const MAX_LOGICAL_PAGE_COUNT: u32 = 1_000;

const MAX_DESIGN_NAME_BYTES: usize = 200;
const MAX_MARKER_FAMILY_BYTES: usize = 64;
const MAX_MARKER_ROLE_COUNT: usize = 16;
const MAX_MARKER_ROLE_BYTES: usize = 32;
const MIN_TRIM_DIMENSION_MM: u32 = 50;
const MAX_TRIM_DIMENSION_MM: u32 = 500;
const TRIM_MATCH_TOLERANCE_MM: f64 = 0.001;
const REQUIRED_MARKER_ROLES: [&str; 4] = ["BL", "BR", "TL", "TR"];
const SUPPORTED_MARKER_FAMILIES: [&str; 2] = ["apriltag", "apriltag-placeholder"];

/// The on-disk/bundled JSON shape. Unknown fields are rejected so a newer required field cannot be
/// silently ignored by an older build.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_trimmed_string(
    value: &str,
    field: &'static str,
    max_bytes: usize,
) -> Result<(), A2dError> {
    if value.trim().is_empty() {
        return Err(manifest_error(
            "MANIFEST_REQUIRED_STRING_EMPTY",
            format!("manifest field `{field}` must not be empty or whitespace-only"),
        )
        .with_detail("field", field));
    }
    if value != value.trim() {
        return Err(manifest_error(
            "MANIFEST_STRING_NOT_CANONICAL",
            format!("manifest field `{field}` must not contain leading or trailing whitespace"),
        )
        .with_detail("field", field));
    }
    if value.len() > max_bytes {
        return Err(manifest_error(
            "MANIFEST_STRING_TOO_LONG",
            format!("manifest field `{field}` exceeds its {max_bytes}-byte limit"),
        )
        .with_detail("field", field)
        .with_detail("actual_bytes", value.len().to_string())
        .with_detail("max_bytes", max_bytes.to_string()));
    }
    Ok(())
}

fn validate_marker_roles(marker_role_ids: &[String]) -> Result<(), A2dError> {
    if marker_role_ids.is_empty() {
        return Err(manifest_error(
            "MANIFEST_NO_MARKER_ROLES",
            "manifest must declare marker_role_ids",
        ));
    }
    if marker_role_ids.len() > MAX_MARKER_ROLE_COUNT {
        return Err(manifest_error(
            "MANIFEST_TOO_MANY_MARKER_ROLES",
            format!(
                "manifest declares {} marker roles, maximum is {MAX_MARKER_ROLE_COUNT}",
                marker_role_ids.len()
            ),
        )
        .with_detail("marker_role_count", marker_role_ids.len().to_string())
        .with_detail("max_marker_role_count", MAX_MARKER_ROLE_COUNT.to_string()));
    }

    for role in marker_role_ids {
        validate_trimmed_string(role, "marker_role_ids", MAX_MARKER_ROLE_BYTES)?;
        if !role.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        }) {
            return Err(manifest_error(
                "MANIFEST_MARKER_ROLE_INVALID",
                "marker roles must use uppercase ASCII letters, digits, underscore, or hyphen",
            )
            .with_detail("marker_role", role));
        }
    }

    let actual: BTreeSet<&str> = marker_role_ids.iter().map(String::as_str).collect();
    if actual.len() != marker_role_ids.len() {
        return Err(manifest_error(
            "MANIFEST_DUPLICATE_MARKER_ROLE",
            "manifest marker_role_ids must not contain duplicates",
        ));
    }
    let required: BTreeSet<&str> = REQUIRED_MARKER_ROLES.into_iter().collect();
    if actual != required {
        return Err(manifest_error(
            "MANIFEST_MARKER_ROLE_SET_UNSUPPORTED",
            "v0.1 Notebook Designs must declare exactly TL, TR, BL, and BR marker roles",
        )
        .with_detail("required_marker_roles", REQUIRED_MARKER_ROLES.join(","))
        .with_detail("actual_marker_roles", marker_role_ids.join(",")));
    }
    Ok(())
}

fn resolve_bundled_notebook_layout(layout_id: &LayoutId) -> Option<PageLayout> {
    let setup = setup_page_layout();
    if setup.id == *layout_id {
        return Some(setup);
    }
    let page = writable_page_layout();
    if page.id == *layout_id {
        return Some(page);
    }
    None
}

fn validate_layout_trim(
    layout_id: &LayoutId,
    field: &'static str,
    trim_width_mm: u32,
    trim_height_mm: u32,
) -> Result<(), A2dError> {
    let layout = resolve_bundled_notebook_layout(layout_id).ok_or_else(|| {
        manifest_error(
            "MANIFEST_LAYOUT_UNAVAILABLE",
            format!("manifest field `{field}` references an unavailable bundled layout"),
        )
        .with_detail("field", field)
        .with_detail("layout_id", layout_id.to_string())
    })?;
    let width_difference = (layout.physical_size.width_mm - f64::from(trim_width_mm)).abs();
    let height_difference = (layout.physical_size.height_mm - f64::from(trim_height_mm)).abs();
    if width_difference > TRIM_MATCH_TOLERANCE_MM || height_difference > TRIM_MATCH_TOLERANCE_MM {
        return Err(manifest_error(
            "MANIFEST_LAYOUT_TRIM_MISMATCH",
            "manifest trim dimensions do not agree with the referenced bundled layout",
        )
        .with_detail("field", field)
        .with_detail("layout_id", layout_id.to_string())
        .with_detail("manifest_trim_width_mm", trim_width_mm.to_string())
        .with_detail("manifest_trim_height_mm", trim_height_mm.to_string())
        .with_detail(
            "layout_trim_width_mm",
            layout.physical_size.width_mm.to_string(),
        )
        .with_detail(
            "layout_trim_height_mm",
            layout.physical_size.height_mm.to_string(),
        ));
    }
    Ok(())
}

/// Parses a Notebook Design manifest from exact UTF-8 JSON source bytes, computes the exact-source
/// SHA-256, and assigns `trust_state` from the caller-supplied loading context.
pub fn parse_manifest(json: &str, trust_state: TrustState) -> Result<NotebookDesign, A2dError> {
    if json.len() > MAX_MANIFEST_BYTES {
        return Err(manifest_error(
            "MANIFEST_TOO_LARGE",
            format!(
                "manifest source is {} bytes, maximum is {MAX_MANIFEST_BYTES}",
                json.len()
            ),
        )
        .with_detail("actual_bytes", json.len().to_string())
        .with_detail("max_bytes", MAX_MANIFEST_BYTES.to_string()));
    }

    let raw: RawManifest = serde_json::from_str(json)
        .map_err(|error| manifest_error("MANIFEST_INVALID_JSON", error.to_string()))?;

    if raw.schema_version == 0 || raw.schema_version > CURRENT_SCHEMA_VERSION {
        return Err(manifest_error(
            "MANIFEST_UNSUPPORTED_SCHEMA_VERSION",
            format!(
                "manifest declares schema_version {}, this build supports 1..={CURRENT_SCHEMA_VERSION}",
                raw.schema_version
            ),
        )
        .with_detail("schema_version", raw.schema_version.to_string()));
    }
    if raw.design_version == 0 {
        return Err(manifest_error(
            "MANIFEST_DESIGN_VERSION_INVALID",
            "design_version must be greater than zero",
        ));
    }
    validate_trimmed_string(&raw.name, "name", MAX_DESIGN_NAME_BYTES)?;
    validate_trimmed_string(&raw.marker_family, "marker_family", MAX_MARKER_FAMILY_BYTES)?;
    if !SUPPORTED_MARKER_FAMILIES.contains(&raw.marker_family.as_str()) {
        return Err(manifest_error(
            "MANIFEST_MARKER_FAMILY_UNSUPPORTED",
            "manifest marker_family is not supported by this build",
        )
        .with_detail("marker_family", &raw.marker_family));
    }
    if !(MIN_TRIM_DIMENSION_MM..=MAX_TRIM_DIMENSION_MM).contains(&raw.trim_width_mm)
        || !(MIN_TRIM_DIMENSION_MM..=MAX_TRIM_DIMENSION_MM).contains(&raw.trim_height_mm)
    {
        return Err(manifest_error(
            "MANIFEST_TRIM_DIMENSIONS_INVALID",
            format!(
                "trim dimensions must each be within {MIN_TRIM_DIMENSION_MM}..={MAX_TRIM_DIMENSION_MM} mm"
            ),
        )
        .with_detail("trim_width_mm", raw.trim_width_mm.to_string())
        .with_detail("trim_height_mm", raw.trim_height_mm.to_string()));
    }
    if raw.logical_page_count == 0 || raw.logical_page_count > MAX_LOGICAL_PAGE_COUNT {
        return Err(manifest_error(
            "MANIFEST_LOGICAL_PAGE_COUNT_INVALID",
            format!("logical_page_count must be within 1..={MAX_LOGICAL_PAGE_COUNT}"),
        )
        .with_detail("logical_page_count", raw.logical_page_count.to_string())
        .with_detail("max_logical_page_count", MAX_LOGICAL_PAGE_COUNT.to_string()));
    }
    validate_marker_roles(&raw.marker_role_ids)?;

    let id = NotebookDesignId::parse(&raw.id)?;
    let setup_layout_id = LayoutId::parse(&raw.setup_layout_id)?;
    let page_layout_id = LayoutId::parse(&raw.page_layout_id)?;
    if setup_layout_id == page_layout_id {
        return Err(manifest_error(
            "MANIFEST_LAYOUT_ROLES_CONFLICT",
            "setup_layout_id and page_layout_id must identify distinct layouts",
        )
        .with_detail("layout_id", setup_layout_id.to_string()));
    }
    validate_layout_trim(
        &setup_layout_id,
        "setup_layout_id",
        raw.trim_width_mm,
        raw.trim_height_mm,
    )?;
    validate_layout_trim(
        &page_layout_id,
        "page_layout_id",
        raw.trim_width_mm,
        raw.trim_height_mm,
    )?;

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
        hex_sha256(json.as_bytes()),
        trust_state,
    ))
}

/// An offline, in-memory lookup from `NotebookDesignId` to its resolved manifest. Every manifest
/// the registry can resolve was parsed and validated up front.
#[derive(Default)]
pub struct ManifestRegistry {
    by_id: HashMap<NotebookDesignId, NotebookDesign>,
}

impl ManifestRegistry {
    pub fn empty() -> Self {
        Self::default()
    }

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

/// The one manifest bundled with this build today: an explicit development placeholder, not an
/// official Notebook Design. It is loaded as trusted only because it ships inside the reviewed app
/// binary; that does not make its unvalidated physical dimensions a production design.
pub fn bundled_placeholder_registry() -> Result<ManifestRegistry, A2dError> {
    const PLACEHOLDER_JSON: &str = include_str!("../manifests/dev-placeholder.json");
    let mut registry = ManifestRegistry::empty();
    registry.insert(parse_manifest(PLACEHOLDER_JSON, TrustState::Trusted)?)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest_value(id: &str, schema_version: u32) -> serde_json::Value {
        serde_json::json!({
            "schema_version": schema_version,
            "id": id,
            "name": "Test Notebook",
            "design_version": 1,
            "trim_width_mm": 152,
            "trim_height_mm": 229,
            "logical_page_count": 100,
            "setup_layout_id": "DEV-SETUP-V1",
            "page_layout_id": "DEV-PAGE-V1",
            "marker_family": "apriltag-placeholder",
            "marker_role_ids": ["TL", "TR", "BL", "BR"]
        })
    }

    fn manifest_json(id: &str, schema_version: u32) -> String {
        serde_json::to_string_pretty(&manifest_value(id, schema_version)).unwrap()
    }

    fn parse_value(value: &serde_json::Value) -> Result<NotebookDesign, A2dError> {
        parse_manifest(&serde_json::to_string(value).unwrap(), TrustState::Trusted)
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
        assert_eq!(design.manifest_hash.len(), 64);
    }

    #[test]
    fn manifest_hash_identifies_exact_source_bytes() {
        let id = NotebookDesignId::generate();
        let json = manifest_json(&id.to_string(), 1);
        let same = parse_manifest(&json, TrustState::Trusted).unwrap();
        let same_again = parse_manifest(&json, TrustState::Trusted).unwrap();
        let whitespace_changed = parse_manifest(&format!("\n{json}"), TrustState::Trusted).unwrap();
        assert_eq!(same.manifest_hash, same_again.manifest_hash);
        assert_ne!(same.manifest_hash, whitespace_changed.manifest_hash);
    }

    #[test]
    fn rejects_oversized_source_before_json_parsing() {
        let error =
            parse_manifest(&" ".repeat(MAX_MANIFEST_BYTES + 1), TrustState::Trusted).unwrap_err();
        assert_eq!(error.code.to_string(), "MANIFEST_TOO_LARGE");
    }

    #[test]
    fn rejects_unknown_fields_and_unsupported_versions() {
        let id = NotebookDesignId::generate();
        let mut unknown = manifest_value(&id.to_string(), 1);
        unknown["future_required_field"] = serde_json::json!(true);
        assert_eq!(
            parse_value(&unknown).unwrap_err().code.to_string(),
            "MANIFEST_INVALID_JSON"
        );

        for version in [0, CURRENT_SCHEMA_VERSION + 1] {
            let error = parse_manifest(
                &manifest_json(&NotebookDesignId::generate().to_string(), version),
                TrustState::Trusted,
            )
            .unwrap_err();
            assert_eq!(
                error.code.to_string(),
                "MANIFEST_UNSUPPORTED_SCHEMA_VERSION"
            );
        }
    }

    #[test]
    fn rejects_invalid_names_versions_dimensions_and_page_counts() {
        let id = NotebookDesignId::generate();
        let mut value = manifest_value(&id.to_string(), 1);
        value["name"] = serde_json::json!("   ");
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_REQUIRED_STRING_EMPTY"
        );

        value = manifest_value(&id.to_string(), 1);
        value["design_version"] = serde_json::json!(0);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_DESIGN_VERSION_INVALID"
        );

        value = manifest_value(&id.to_string(), 1);
        value["trim_width_mm"] = serde_json::json!(0);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_TRIM_DIMENSIONS_INVALID"
        );

        value = manifest_value(&id.to_string(), 1);
        value["logical_page_count"] = serde_json::json!(MAX_LOGICAL_PAGE_COUNT + 1);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_LOGICAL_PAGE_COUNT_INVALID"
        );
    }

    #[test]
    fn rejects_unsupported_marker_family_and_invalid_role_sets() {
        let id = NotebookDesignId::generate();
        let mut value = manifest_value(&id.to_string(), 1);
        value["marker_family"] = serde_json::json!("aruco");
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_MARKER_FAMILY_UNSUPPORTED"
        );

        value = manifest_value(&id.to_string(), 1);
        value["marker_role_ids"] = serde_json::json!(["TL", "TR", "BL", "BL"]);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_DUPLICATE_MARKER_ROLE"
        );

        value = manifest_value(&id.to_string(), 1);
        value["marker_role_ids"] = serde_json::json!(["TL", "TR", "BL", "CENTER"]);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_MARKER_ROLE_SET_UNSUPPORTED"
        );
    }

    #[test]
    fn rejects_missing_conflicting_or_trim_mismatched_layouts() {
        let id = NotebookDesignId::generate();
        let mut value = manifest_value(&id.to_string(), 1);
        value["page_layout_id"] = serde_json::json!("UNKNOWN-V1");
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_LAYOUT_UNAVAILABLE"
        );

        value = manifest_value(&id.to_string(), 1);
        value["page_layout_id"] = serde_json::json!("DEV-SETUP-V1");
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_LAYOUT_ROLES_CONFLICT"
        );

        value = manifest_value(&id.to_string(), 1);
        value["trim_width_mm"] = serde_json::json!(153);
        assert_eq!(
            parse_value(&value).unwrap_err().code.to_string(),
            "MANIFEST_LAYOUT_TRIM_MISMATCH"
        );
    }

    #[test]
    fn rejects_an_invalid_design_id() {
        let error = parse_manifest(&manifest_json("TOOSHORT", 1), TrustState::Trusted).unwrap_err();
        assert!(error.code.to_string().contains("INVALID_LENGTH"));
    }

    #[test]
    fn registry_resolves_an_inserted_design_and_rejects_duplicate_ids() {
        let id = NotebookDesignId::generate();
        let first =
            parse_manifest(&manifest_json(&id.to_string(), 1), TrustState::Trusted).unwrap();
        let mut second_value = manifest_value(&id.to_string(), 1);
        second_value["name"] = serde_json::json!("Different Name");
        let second = parse_value(&second_value).unwrap();

        let mut registry = ManifestRegistry::empty();
        registry.insert(first).unwrap();
        assert_eq!(
            registry.resolve(&id).map(|design| design.id().clone()),
            Some(id)
        );
        assert!(registry.resolve(&NotebookDesignId::generate()).is_none());
        assert_eq!(
            registry.insert(second).unwrap_err().code.to_string(),
            "MANIFEST_DUPLICATE_DESIGN_ID"
        );
    }

    #[test]
    fn bundled_placeholder_is_valid_but_explicitly_nonproduction() {
        let registry = bundled_placeholder_registry().unwrap();
        assert_eq!(registry.len(), 1);
        let id = NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX").unwrap();
        let design = registry.resolve(&id).unwrap();
        assert!(design.name.contains("Development Placeholder"));
        assert_eq!(design.marker_family, "apriltag-placeholder");
    }
}
