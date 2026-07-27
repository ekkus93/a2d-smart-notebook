#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_exact(path: str, old: str, new: str, expected: int = 1) -> None:
    target = ROOT / path
    text = target.read_text()
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{path}: expected {expected} matches, found {count}: {old!r}")
    target.write_text(text.replace(old, new))


replace_exact(
    "crates/a2d-core/src/milestone6.rs",
    "use a2d_layout::{\n    ALL_PAPER_SIZES, ALL_STYLES, PaperSize, SmartPageStyle, bundled_placeholder_registry,\n    smart_page_layout,\n};\n",
    "use a2d_layout::smart_page::{ALL_PAPER_SIZES, ALL_STYLES};\nuse a2d_layout::{\n    PaperSize, SmartPageStyle, bundled_placeholder_registry, smart_page_layout,\n};\n",
)
replace_exact(
    "crates/a2d-core/src/milestone6.rs",
    '''    pub fn rename_notebook(
        &self,
        notebook_id: &NotebookId,
        display_name: String,
    ) -> Result<NotebookSummary, A2dError> {
        let mut storage = self.lock_storage()?;
''',
    '''    pub fn rename_notebook(
        &self,
        notebook_id: &NotebookId,
        display_name: String,
    ) -> Result<NotebookSummary, A2dError> {
        let storage = self.lock_storage()?;
''',
)
replace_exact(
    "crates/a2d-core/src/milestone6.rs",
    '''    pub fn archive_notebook(
        &self,
        notebook_id: &NotebookId,
    ) -> Result<NotebookSummary, A2dError> {
        let mut storage = self.lock_storage()?;
''',
    '''    pub fn archive_notebook(
        &self,
        notebook_id: &NotebookId,
    ) -> Result<NotebookSummary, A2dError> {
        let storage = self.lock_storage()?;
''',
)
replace_exact(
    "crates/a2d-ffi/src/milestone6.rs",
    "use std::sync::Arc;\n\n",
    "",
)
replace_exact(
    "crates/a2d-storage/src/lib.rs",
    "        assert_eq!(upgraded.schema_version().unwrap(), 2);\n",
    "        assert_eq!(\n            upgraded.schema_version().unwrap(),\n            MIGRATIONS.last().unwrap().version\n        );\n",
)
replace_exact(
    "crates/a2d-storage/src/lib.rs",
    '''        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], (1, "initial".to_string(), ORIGINAL_V1_APPLIED_AT));
        assert_eq!(rows[1].0, 2);
        assert_eq!(rows[1].1, "page_generated_pdf_asset");
''',
    '''        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], (1, "initial".to_string(), ORIGINAL_V1_APPLIED_AT));
        assert_eq!(rows[1].0, 2);
        assert_eq!(rows[1].1, "page_generated_pdf_asset");
        assert_eq!(rows[2].0, 3);
        assert_eq!(rows[2].1, "milestone6_notebook_workflows");
''',
)

print("Milestone 6 pass-2 corrections applied")
