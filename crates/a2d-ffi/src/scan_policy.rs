//! UniFFI projection of the Rust-owned stored-page scan layout policy.

use a2d_domain::PageId;
use a2d_layout::MarkerRole;

use super::{A2dClient, A2dFfiError};

#[derive(Clone, Debug, uniffi::Record)]
pub struct StoredScanLayoutPolicy {
    pub layout_id: String,
    pub physical_width_mm: f64,
    pub physical_height_mm: f64,
    pub marker_family: String,
    pub declared_marker_family: Option<String>,
    pub top_left_tag_id: u32,
    pub top_right_tag_id: u32,
    pub bottom_right_tag_id: u32,
    pub bottom_left_tag_id: u32,
    pub corrected_width: u32,
    pub corrected_height: u32,
    pub layout_version: String,
    pub processing_policy_version: u32,
}

impl From<a2d_core::StoredScanLayout> for StoredScanLayoutPolicy {
    fn from(value: a2d_core::StoredScanLayout) -> Self {
        let marker_id = |role| {
            value
                .marker_roles
                .iter()
                .find(|marker| marker.role == role)
                .expect("resolved scan layout guarantees all four marker roles")
                .marker_id
        };
        Self {
            layout_id: value.layout_id.to_string(),
            physical_width_mm: value.physical_width_mm,
            physical_height_mm: value.physical_height_mm,
            marker_family: value.marker_family,
            declared_marker_family: value.declared_marker_family,
            top_left_tag_id: marker_id(MarkerRole::TopLeft),
            top_right_tag_id: marker_id(MarkerRole::TopRight),
            bottom_right_tag_id: marker_id(MarkerRole::BottomRight),
            bottom_left_tag_id: marker_id(MarkerRole::BottomLeft),
            corrected_width: value.corrected_width,
            corrected_height: value.corrected_height,
            layout_version: value.layout_version,
            processing_policy_version: value.processing_policy_version,
        }
    }
}

#[uniffi::export]
impl A2dClient {
    /// Resolves the exact physical geometry, marker assignment, and corrected output dimensions
    /// that Rust durable registration will use for this stored page.
    pub fn resolve_stored_scan_layout_policy(
        &self,
        page_id: String,
    ) -> Result<StoredScanLayoutPolicy, A2dFfiError> {
        let page_id = PageId::parse(&page_id)?;
        self.core
            .resolve_stored_scan_layout(&page_id)
            .map(Into::into)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        A2dClient, OpenLibraryRequest, SmartPageContentStyle, SmartPageGenerationRequest,
        SmartPagePaperSize,
    };

    #[test]
    fn generated_a4_smart_page_projects_the_registration_policy() {
        let dir = std::env::temp_dir().join(format!(
            "a2d-ffi-scan-policy-{}",
            a2d_domain::PageId::generate()
        ));
        let client = A2dClient::open(OpenLibraryRequest {
            library_path: dir.to_string_lossy().into_owned(),
        })
        .unwrap();
        let generated = client
            .generate_smart_pages(SmartPageGenerationRequest {
                paper_size: SmartPagePaperSize::A4,
                style: SmartPageContentStyle::Blank,
                page_count: 1,
                starting_visible_page: 1,
            })
            .unwrap();

        let policy = client
            .resolve_stored_scan_layout_policy(generated.page_ids[0].clone())
            .unwrap();
        assert_eq!(policy.layout_id, "SP-A4-BLANK-V1");
        assert_eq!(policy.marker_family, "tagStandard41h12");
        assert_eq!(
            (
                policy.top_left_tag_id,
                policy.top_right_tag_id,
                policy.bottom_right_tag_id,
                policy.bottom_left_tag_id,
            ),
            (0, 1, 2, 3)
        );
        assert_eq!((policy.corrected_width, policy.corrected_height), (900, 1_273));
        assert_eq!(policy.processing_policy_version, 1);

        drop(client);
        std::fs::remove_dir_all(dir).ok();
    }
}
