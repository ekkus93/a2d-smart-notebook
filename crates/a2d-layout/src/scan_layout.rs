//! Rust-owned resolution of the physical layout and portable processing parameters used by scan
//! preview and durable registration.
//!
//! The printable v1 compatibility surface uses official `tagStandard41h12` markers with stable
//! IDs 0..=3 assigned to semantic corners. Notebook Design manifests currently describe semantic
//! roles but do not yet carry numeric marker IDs or a concrete detector-family token, so this
//! module maps the reviewed v1 printable contract explicitly and keeps the manifest declaration as
//! provenance. Unknown layouts and contradictory stored records fail closed; no development layout
//! is selected as a fallback.

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesign, Page, PageKind,
};

use crate::notebook::writable_page_layout;
use crate::page_layout::{MarkerRole, PageLayout};
use crate::smart_page::{ALL_PAPER_SIZES, ALL_STYLES, smart_page_layout};

/// Portable scan-processing policy version. Presentation-only guidance thresholds are not part of
/// this record.
pub const SCAN_PROCESSING_POLICY_VERSION: u32 = 1;
/// The reviewed printable marker family used by `a2d-pdf` and detected by `a2d-image` in v1.
pub const V1_MARKER_FAMILY: &str = "tagStandard41h12";
/// Portrait corrected images use a stable width and derive height from physical page geometry.
pub const V1_CORRECTED_WIDTH_PX: u32 = 900;
const MAX_CORRECTED_DIMENSION_PX: u32 = 4_096;
const PHYSICAL_DIMENSION_TOLERANCE_MM: f64 = 0.001;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedMarkerRole {
    pub role: MarkerRole,
    pub marker_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedScanLayout {
    pub layout_id: LayoutId,
    pub physical_width_mm: f64,
    pub physical_height_mm: f64,
    pub marker_family: String,
    /// Original manifest declaration when a Notebook Design supplied one. This is provenance, not
    /// detector configuration, because the current manifest schema does not name a concrete tag
    /// family such as `tagStandard41h12`.
    pub declared_marker_family: Option<String>,
    pub marker_roles: Vec<ResolvedMarkerRole>,
    pub corrected_width: u32,
    pub corrected_height: u32,
    pub layout_version: String,
    pub processing_policy_version: u32,
    pub page_layout: PageLayout,
}

impl ResolvedScanLayout {
    pub fn marker_id_layout(&self) -> [(u32, MarkerRole); 4] {
        [
            (marker_id_for_role(MarkerRole::TopLeft), MarkerRole::TopLeft),
            (
                marker_id_for_role(MarkerRole::TopRight),
                MarkerRole::TopRight,
            ),
            (
                marker_id_for_role(MarkerRole::BottomRight),
                MarkerRole::BottomRight,
            ),
            (
                marker_id_for_role(MarkerRole::BottomLeft),
                MarkerRole::BottomLeft,
            ),
        ]
    }
}

/// Stable marker IDs assigned by the v1 PDF renderer. This function is deliberately public so
/// preview, registration, and future platform projections can consume one Rust-owned mapping.
pub const fn marker_id_for_role(role: MarkerRole) -> u32 {
    match role {
        MarkerRole::TopLeft => 0,
        MarkerRole::TopRight => 1,
        MarkerRole::BottomRight => 2,
        MarkerRole::BottomLeft => 3,
    }
}

/// Resolves scan geometry from the canonical stored page record. Notebook Pages require their
/// stored Notebook Design; Smart Pages resolve through the bundled canonical Smart Page registry.
pub fn resolve_scan_layout_for_page(
    page: &Page,
    notebook_design: Option<&NotebookDesign>,
) -> Result<ResolvedScanLayout, A2dError> {
    match &page.kind {
        PageKind::NotebookPage { design_id, .. } => {
            let design = notebook_design.ok_or_else(|| {
                resolution_error(
                    "SCAN_LAYOUT_NOTEBOOK_DESIGN_REQUIRED",
                    ErrorCategory::UnsupportedFormat,
                    "resolving a Notebook Page scan requires its stored Notebook Design",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("design_id", design_id.to_string())
            })?;
            if design.id() != design_id {
                return Err(resolution_error(
                    "SCAN_LAYOUT_NOTEBOOK_DESIGN_CONFLICT",
                    ErrorCategory::Integrity,
                    "the supplied Notebook Design does not own the stored Notebook Page",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("page_design_id", design_id.to_string())
                .with_detail("supplied_design_id", design.id().to_string()));
            }
            if page.layout_id != design.page_layout_id {
                return Err(resolution_error(
                    "SCAN_LAYOUT_PAGE_DESIGN_LAYOUT_CONFLICT",
                    ErrorCategory::Integrity,
                    "the stored page layout does not agree with its Notebook Design",
                )
                .with_detail("page_id", page.id().to_string())
                .with_detail("page_layout_id", page.layout_id.to_string())
                .with_detail("design_layout_id", design.page_layout_id.to_string()));
            }
            validate_manifest_marker_roles(design)?;
            let layout = resolve_notebook_page_layout(&page.layout_id)?;
            validate_design_trim(design, &layout)?;
            resolved(
                layout,
                Some(design.marker_family.clone()),
                format!(
                    "notebook-design:{}:v{}:{}",
                    design.id(), design.design_version, design.manifest_hash
                ),
            )
        }
        PageKind::SmartPage { .. } => {
            if notebook_design.is_some() {
                return Err(resolution_error(
                    "SCAN_LAYOUT_UNEXPECTED_NOTEBOOK_DESIGN",
                    ErrorCategory::Integrity,
                    "a Smart Page scan must not be resolved through a Notebook Design",
                )
                .with_detail("page_id", page.id().to_string()));
            }
            let layout = resolve_smart_page_layout(&page.layout_id)?;
            let layout_version = layout.id.to_string();
            resolved(layout, None, layout_version)
        }
    }
}

/// Resolves any bundled canonical scan layout by ID. This does not use a development Notebook Page
/// as a fallback: unknown IDs return a typed unsupported-format error.
pub fn resolve_bundled_scan_layout(layout_id: &LayoutId) -> Result<ResolvedScanLayout, A2dError> {
    if let Ok(layout) = resolve_notebook_page_layout(layout_id) {
        return resolved(layout, None, format!("bundled:{layout_id}"));
    }
    let layout = resolve_smart_page_layout(layout_id)?;
    resolved(layout, None, format!("bundled:{layout_id}"))
}

fn resolve_notebook_page_layout(layout_id: &LayoutId) -> Result<PageLayout, A2dError> {
    let layout = writable_page_layout();
    if layout.id == *layout_id {
        return Ok(layout);
    }
    Err(unavailable_layout_error(layout_id))
}

fn resolve_smart_page_layout(layout_id: &LayoutId) -> Result<PageLayout, A2dError> {
    for paper in ALL_PAPER_SIZES {
        for style in ALL_STYLES {
            let layout = smart_page_layout(paper, style);
            if layout.id == *layout_id {
                return Ok(layout);
            }
        }
    }
    Err(unavailable_layout_error(layout_id))
}

fn resolved(
    layout: PageLayout,
    declared_marker_family: Option<String>,
    layout_version: String,
) -> Result<ResolvedScanLayout, A2dError> {
    layout.validate().map_err(|error| {
        resolution_error(
            "SCAN_LAYOUT_CANONICAL_LAYOUT_INVALID",
            ErrorCategory::Integrity,
            "a bundled canonical layout failed validation",
        )
        .with_detail("layout_id", layout.id.to_string())
        .with_detail("cause_code", error.code.to_string())
    })?;
    let corrected_height = corrected_height_for(&layout)?;
    let marker_roles = MarkerRole::ALL
        .into_iter()
        .map(|role| ResolvedMarkerRole {
            role,
            marker_id: marker_id_for_role(role),
        })
        .collect();
    Ok(ResolvedScanLayout {
        layout_id: layout.id.clone(),
        physical_width_mm: layout.physical_size.width_mm,
        physical_height_mm: layout.physical_size.height_mm,
        marker_family: V1_MARKER_FAMILY.to_string(),
        declared_marker_family,
        marker_roles,
        corrected_width: V1_CORRECTED_WIDTH_PX,
        corrected_height,
        layout_version,
        processing_policy_version: SCAN_PROCESSING_POLICY_VERSION,
        page_layout: layout,
    })
}

fn corrected_height_for(layout: &PageLayout) -> Result<u32, A2dError> {
    let width_mm = layout.physical_size.width_mm;
    let height_mm = layout.physical_size.height_mm;
    if !width_mm.is_finite() || !height_mm.is_finite() || width_mm <= 0.0 || height_mm <= 0.0 {
        return Err(resolution_error(
            "SCAN_LAYOUT_PHYSICAL_SIZE_INVALID",
            ErrorCategory::Integrity,
            "scan layout physical dimensions must be finite and positive",
        )
        .with_detail("layout_id", layout.id.to_string())
        .with_detail("physical_width_mm", width_mm.to_string())
        .with_detail("physical_height_mm", height_mm.to_string()));
    }
    let height = (f64::from(V1_CORRECTED_WIDTH_PX) * height_mm / width_mm).round();
    if !height.is_finite() || height < 1.0 || height > f64::from(MAX_CORRECTED_DIMENSION_PX) {
        return Err(resolution_error(
            "SCAN_LAYOUT_CORRECTED_SIZE_UNSUPPORTED",
            ErrorCategory::UnsupportedFormat,
            "scan layout requires corrected dimensions outside the supported v1 bounds",
        )
        .with_detail("layout_id", layout.id.to_string())
        .with_detail("computed_height", height.to_string())
        .with_detail(
            "max_corrected_dimension",
            MAX_CORRECTED_DIMENSION_PX.to_string(),
        ));
    }
    Ok(height as u32)
}

fn validate_manifest_marker_roles(design: &NotebookDesign) -> Result<(), A2dError> {
    let mut actual = design.marker_role_ids.clone();
    actual.sort();
    let expected = ["BL", "BR", "TL", "TR"];
    if actual.iter().map(String::as_str).ne(expected) {
        return Err(resolution_error(
            "SCAN_LAYOUT_MARKER_ROLE_SET_UNSUPPORTED",
            ErrorCategory::Integrity,
            "the stored Notebook Design marker roles do not match the v1 printable contract",
        )
        .with_detail("design_id", design.id().to_string())
        .with_detail("actual_marker_roles", actual.join(","))
        .with_detail("expected_marker_roles", expected.join(",")));
    }
    Ok(())
}

fn validate_design_trim(design: &NotebookDesign, layout: &PageLayout) -> Result<(), A2dError> {
    let width_difference =
        (layout.physical_size.width_mm - f64::from(design.trim_size.width)).abs();
    let height_difference =
        (layout.physical_size.height_mm - f64::from(design.trim_size.height)).abs();
    if width_difference > PHYSICAL_DIMENSION_TOLERANCE_MM
        || height_difference > PHYSICAL_DIMENSION_TOLERANCE_MM
    {
        return Err(resolution_error(
            "SCAN_LAYOUT_NOTEBOOK_TRIM_CONFLICT",
            ErrorCategory::Integrity,
            "the stored Notebook Design trim dimensions do not match its page layout",
        )
        .with_detail("design_id", design.id().to_string())
        .with_detail("layout_id", layout.id.to_string())
        .with_detail("design_width_mm", design.trim_size.width.to_string())
        .with_detail("design_height_mm", design.trim_size.height.to_string())
        .with_detail("layout_width_mm", layout.physical_size.width_mm.to_string())
        .with_detail(
            "layout_height_mm",
            layout.physical_size.height_mm.to_string(),
        ));
    }
    Ok(())
}

fn unavailable_layout_error(layout_id: &LayoutId) -> A2dError {
    resolution_error(
        "SCAN_LAYOUT_UNAVAILABLE",
        ErrorCategory::UnsupportedFormat,
        "this build cannot resolve the stored page layout for scanning",
    )
    .with_detail("layout_id", layout_id.to_string())
}

fn resolution_error(
    code: &'static str,
    category: ErrorCategory,
    message: impl Into<String>,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        category,
        if category == ErrorCategory::Integrity {
            ErrorSeverity::Critical
        } else {
            ErrorSeverity::Error
        },
        "error.layout.scan_resolution",
        message.into(),
        false,
    )
}

#[cfg(test)]
mod tests {
    use a2d_domain::{
        NotebookDesignId, NotebookId, PageId, PageState, SmartPageId, TrimSizeMm, TrustState,
    };

    use super::*;

    fn notebook_design() -> NotebookDesign {
        NotebookDesign::new(
            NotebookDesignId::parse("6DE28E53DBKPXCWWNHPC8T7QJX").unwrap(),
            1,
            "Development Placeholder".to_string(),
            1,
            TrimSizeMm {
                width: 152,
                height: 229,
            },
            100,
            LayoutId::parse("DEV-SETUP-V1").unwrap(),
            LayoutId::parse("DEV-PAGE-V1").unwrap(),
            "apriltag-placeholder".to_string(),
            vec![
                "TL".to_string(),
                "TR".to_string(),
                "BL".to_string(),
                "BR".to_string(),
            ],
            "manifest-hash".to_string(),
            TrustState::Trusted,
        )
    }

    fn notebook_page(design: &NotebookDesign) -> Page {
        Page::new(
            PageId::generate(),
            PageKind::NotebookPage {
                notebook_id: NotebookId::generate(),
                design_id: design.id().clone(),
                logical_page_number: 1,
            },
            design.page_layout_id.clone(),
            None,
            PageState::Unscanned,
            1,
        )
    }

    fn smart_page(layout_id: LayoutId) -> Page {
        Page::new(
            PageId::generate(),
            PageKind::SmartPage {
                smart_page_id: SmartPageId::generate(),
                page_set_id: None,
                visible_page_number: Some(1),
            },
            layout_id,
            None,
            PageState::GeneratedNotScanned,
            1,
        )
    }

    #[test]
    fn resolves_notebook_page_from_its_design_without_a_fallback() {
        let design = notebook_design();
        let page = notebook_page(&design);
        let resolved = resolve_scan_layout_for_page(&page, Some(&design)).unwrap();
        assert_eq!(resolved.layout_id.to_string(), "DEV-PAGE-V1");
        assert_eq!(resolved.corrected_width, 900);
        assert_eq!(resolved.corrected_height, 1_356);
        assert_eq!(resolved.marker_family, V1_MARKER_FAMILY);
        assert_eq!(
            resolved.declared_marker_family.as_deref(),
            Some("apriltag-placeholder")
        );
        assert_eq!(
            resolved.marker_id_layout(),
            [
                (0, MarkerRole::TopLeft),
                (1, MarkerRole::TopRight),
                (2, MarkerRole::BottomRight),
                (3, MarkerRole::BottomLeft),
            ]
        );
    }

    #[test]
    fn every_smart_page_layout_resolves_with_its_physical_aspect_ratio() {
        for paper in ALL_PAPER_SIZES {
            for style in ALL_STYLES {
                let layout = smart_page_layout(paper, style);
                let page = smart_page(layout.id.clone());
                let resolved = resolve_scan_layout_for_page(&page, None).unwrap();
                assert_eq!(resolved.layout_id, layout.id);
                let physical_ratio = layout.physical_size.height_mm / layout.physical_size.width_mm;
                let pixel_ratio =
                    f64::from(resolved.corrected_height) / f64::from(resolved.corrected_width);
                assert!(
                    (physical_ratio - pixel_ratio).abs() < 0.001,
                    "{} corrected aspect ratio drifted: physical={physical_ratio}, pixels={pixel_ratio}",
                    resolved.layout_id
                );
            }
        }
    }

    #[test]
    fn rejects_unknown_layout_instead_of_using_the_development_page() {
        let page = smart_page(LayoutId::parse("UNKNOWN-V1").unwrap());
        let error = resolve_scan_layout_for_page(&page, None).unwrap_err();
        assert_eq!(error.code.to_string(), "SCAN_LAYOUT_UNAVAILABLE");
    }

    #[test]
    fn rejects_notebook_page_layout_that_disagrees_with_its_design() {
        let design = notebook_design();
        let mut page = notebook_page(&design);
        page.layout_id = LayoutId::parse("SP-A4-BLANK-V1").unwrap();
        let error = resolve_scan_layout_for_page(&page, Some(&design)).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "SCAN_LAYOUT_PAGE_DESIGN_LAYOUT_CONFLICT"
        );
    }

    #[test]
    fn rejects_notebook_page_without_its_design() {
        let design = notebook_design();
        let page = notebook_page(&design);
        let error = resolve_scan_layout_for_page(&page, None).unwrap_err();
        assert_eq!(
            error.code.to_string(),
            "SCAN_LAYOUT_NOTEBOOK_DESIGN_REQUIRED"
        );
    }

    #[test]
    fn marker_ids_are_stable_and_unique() {
        let ids = MarkerRole::ALL.map(marker_id_for_role);
        assert_eq!(ids, [0, 1, 3, 2]);
        let unique = ids.into_iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), 4);
    }
}
