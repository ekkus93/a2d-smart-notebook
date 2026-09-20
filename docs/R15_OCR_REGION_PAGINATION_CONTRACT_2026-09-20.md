# R15 OCR region pagination contract

This note records the implementation contract for R15 in `M11_OCR_POST_CLOSEOUT_REVIEW_2_REMEDIATION_TODO_2026-09-19.md`. It is intentionally specific enough to drive the Rust, UniFFI, Android, and test changes without allowing the old 1,000-row readback ceiling to become an implicit product limit.

## Inventory

The current Rust write path in `crates/a2d-core/src/ocr_regions.rs` accepts one `record_ocr_text_regions` batch of at most **20,000** regions (`MAX_OCR_TEXT_REGION_BATCH_SIZE`). The transactional queue finalizer likewise accepts a provider result containing up to 20,000 regions. This is the effective supported write maximum and therefore the authoritative supported per-run region maximum for R15.

The current readback path `load_latest_ocr_output` accepts `region_limit` up to **1,000** (`MAX_OCR_READBACK_REGION_LIMIT`). It returns the authoritative `text_region_count`, but only the first `region_limit` rows and has no offset/cursor. Before PR #135, Page Viewer could therefore treat a detected run with more than 1,000 durable regions as a complete overlay. PR #135 made that state fail closed; R15 replaces that temporary sentinel with complete bounded pagination.

Android currently defaults `FfiAndroidOcrReadback` to a 1,000-region request. The adapter now rejects a detected run when `text_region_count != text_regions.size`, which prevents silent truncation but cannot hydrate a larger run until pagination exists.

## Authoritative limits

Rust owns these limits. Foreign callers must not define an independent product maximum.

- `MAX_OCR_REGIONS_PER_RUN = 20_000`: maximum durable OCR regions for one OCR run.
- `MAX_OCR_REGION_PAGE_SIZE = 1_000`: maximum number of region DTOs returned by one readback page.
- A write that would make the durable region count exceed 20,000 is rejected explicitly, including when multiple write calls cumulatively cross the limit.
- The transactional finalizer rejects provider output above 20,000 before durable mutation.
- Page size is a transport/memory bound, not a completeness bound.

## Rust readback page

Readback must expose a bounded page with these semantics:

- `total_count`: authoritative durable region count for the selected OCR run.
- `returned_count`: number of region DTOs in this response.
- `offset`: zero-based durable-row offset used for this response.
- `next_offset`: next offset when more rows remain; absent when complete.
- `has_more`: true exactly when another page is required.
- `complete`: true exactly when this response reaches the authoritative end of the region sequence.
- `regions`: stable durable region IDs and region data in deterministic repository order.

The request carries `offset` and `page_size`. `page_size == 0` is permitted for metadata-only readback. `page_size > 1,000` is rejected. `offset > total_count` is rejected rather than silently normalized. Arithmetic must be checked before converting between integer widths.

A detected OCR run with more than 20,000 durable rows is an integrity error: readback must not claim completeness for data outside the supported contract.

## Android hydration

Page Viewer hydration must iterate pages until `complete == true`. It must validate every page before accepting it:

1. OCR run identity remains unchanged across pages.
2. `total_count` remains unchanged across pages.
3. Returned IDs are unique across the aggregate.
4. `returned_count` matches the actual region list size.
5. `next_offset` advances monotonically when `has_more` is true.
6. The final aggregate size equals `total_count`.

A later-page exception or invariant violation is a real hydration error. The partially accumulated list is never published as a complete overlay. Stable `text_region_id` values are preserved so selection can survive replacement of a partial/loading model by the completed aggregate where the UI controller supports selection.

Recreation may restart pagination from offset zero. Deduplication by stable region ID makes overlapping/replayed pages harmless; a conflicting duplicate ID is an error rather than last-write-wins data loss.

## Qualification matrix

R15 tests must cover:

- 51+ regions (ordinary multi-page behavior with a deliberately smaller test page size).
- 1,001+ regions (regression for the former readback ceiling).
- exactly 20,000 regions.
- explicit rejection of a 20,001st region, including cumulative writes.
- metadata consistency (`total_count`, `returned_count`, `offset`, `next_offset`, `has_more`, `complete`).
- full multi-page Android hydration and stable IDs.
- recreation/restart during pagination without duplicates.
- injected later-page failure yielding explicit error/partial state, never a complete overlay.
- search/overlay metadata coherence after full hydration.

## Migration rule

The legacy `load_latest_ocr_output(scan_id, region_limit)` shape may remain temporarily for compatibility while production Android moves to the paginated contract, but production Page Viewer must not depend on a single bounded preview call for completeness. Once the paginated production path and sentinels are merged, compatibility use must remain fail-closed for truncation until removed.