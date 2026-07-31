//! Rust-core projection of the non-destructive canonical-library integrity checker.

pub use a2d_storage::{
    IntegrityCancellation, IntegrityCheckOptions, IntegrityCheckOutcome, IntegrityFinding,
    IntegrityFindingSeverity, IntegrityReport,
};

use a2d_domain::A2dError;

use super::A2dCore;

impl A2dCore {
    /// Checks the canonical database and library-owned files without repairing or deleting data.
    pub fn check_integrity(
        &self,
        options: IntegrityCheckOptions,
        cancellation: &IntegrityCancellation,
    ) -> Result<IntegrityCheckOutcome, A2dError> {
        self.lock_storage()?
            .check_integrity(&self.library_path, options, cancellation)
    }
}

#[cfg(test)]
mod tests {
    use a2d_domain::PageId;

    use super::*;
    use crate::OpenLibraryRequest;

    #[test]
    fn core_exposes_a_clean_read_only_report_for_a_fresh_library() {
        let root = std::env::temp_dir().join(format!(
            "a2d-core-integrity-{}",
            PageId::generate()
        ));
        let core = A2dCore::open(OpenLibraryRequest {
            library_path: root.to_string_lossy().into_owned(),
        })
        .unwrap();
        let outcome = core
            .check_integrity(
                IntegrityCheckOptions::default(),
                &IntegrityCancellation::active(),
            )
            .unwrap();
        let IntegrityCheckOutcome::Completed(report) = outcome else {
            panic!("fresh-library integrity check was unexpectedly cancelled")
        };
        assert!(report.is_clean(), "findings: {:?}", report.findings);
        drop(core);
        std::fs::remove_dir_all(root).ok();
    }
}
