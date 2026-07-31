//! Rust-owned resource and wire-format limits for Smart Page generation.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SmartPageGenerationPolicy {
    pub policy_version: u32,
    pub maximum_page_count: u32,
    pub maximum_starting_visible_page: u32,
    pub maximum_pdf_output_bytes: u64,
}

pub const SMART_PAGE_GENERATION_POLICY_VERSION: u32 = 1;
pub const MAXIMUM_SMART_PAGE_VISIBLE_NUMBER: u32 = 999_999;

pub const fn smart_page_generation_policy() -> SmartPageGenerationPolicy {
    SmartPageGenerationPolicy {
        policy_version: SMART_PAGE_GENERATION_POLICY_VERSION,
        maximum_page_count: a2d_pdf::MAX_PAGE_SET_PAGE_COUNT,
        maximum_starting_visible_page: MAXIMUM_SMART_PAGE_VISIBLE_NUMBER,
        maximum_pdf_output_bytes: a2d_pdf::MAX_PDF_OUTPUT_BYTES as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_matches_pdf_generator_limits() {
        let policy = smart_page_generation_policy();
        assert_eq!(policy.policy_version, 1);
        assert_eq!(policy.maximum_page_count, a2d_pdf::MAX_PAGE_SET_PAGE_COUNT);
        assert_eq!(policy.maximum_starting_visible_page, 999_999);
        assert_eq!(
            policy.maximum_pdf_output_bytes,
            a2d_pdf::MAX_PDF_OUTPUT_BYTES as u64
        );
    }
}
