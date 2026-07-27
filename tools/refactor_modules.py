from pathlib import Path

ROOT = Path('.')


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f'{label}: expected one match, found {count}')
    return text.replace(old, new, 1)


def refactor_entities() -> None:
    path = ROOT / 'crates/a2d-domain/src/entities.rs'
    text = path.read_text()
    text = replace_once(text, 'use std::collections::BTreeMap;\n\n', '', 'entities BTreeMap import')
    text = replace_once(
        text,
        '''use crate::id::{\n    AnnotationId, AssetId, AuditEventId, CollectionId, NotebookDesignId, NotebookId, OcrRunId,\n    PageId, PageSetId, PhysicalCopyId, ReviewItemId, ScanId, SkillId, SkillRunId, SmartPageId,\n    TextCorrectionId, TextRegionId,\n};''',
        '''use crate::id::{\n    AssetId, NotebookDesignId, NotebookId, PageId, PageSetId, PhysicalCopyId, ScanId,\n    SmartPageId,\n};''',
        'entities id imports',
    )
    start_marker = '/// INFERRED — spec §15.8 describes a Page Set'
    end_marker = '#[cfg(test)]\nmod tests {'
    start = text.index(start_marker)
    end = text.index(end_marker)
    moved = text[start:end].rstrip() + '\n'
    replacement = '''mod derived;\n\npub use derived::{\n    Annotation, AuditEvent, Collection, OcrRun, PageSet, ReviewItem, ReviewItemKind,\n    ReviewItemStatus, SkillDefinition, SkillRun, SkillRunStatus, TextCorrection, TextRegion,\n};\n\n'''
    text = text[:start] + replacement + text[end:]
    path.write_text(text)

    derived = ROOT / 'crates/a2d-domain/src/entities/derived.rs'
    derived.parent.mkdir(parents=True, exist_ok=True)
    derived.write_text(
        '''//! Derived knowledge, review, automation, and audit records.\n\nuse std::collections::BTreeMap;\n\nuse super::Provenance;\nuse crate::error::ErrorSeverity;\nuse crate::id::{\n    AnnotationId, AuditEventId, CollectionId, OcrRunId, PageId, PageSetId, ReviewItemId, ScanId,\n    SkillId, SkillRunId, TextCorrectionId, TextRegionId,\n};\n\n''' + moved
    )


def refactor_repository() -> None:
    path = ROOT / 'crates/a2d-storage/src/repository.rs'
    text = path.read_text()
    text = replace_once(
        text,
        '''use a2d_domain::{\n    A2dError, Asset, AssetId, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity,\n    LayoutId, Notebook, NotebookDesign, NotebookDesignId, NotebookId, OcrRun, OcrRunId, Page,\n    PageId, PageKind, PageSet, PageSetId, PageState, Scan, ScanId, TrimSizeMm, TrustState,\n};\nuse rusqlite::{Connection, OptionalExtension, params};''',
        '''use a2d_domain::{\n    A2dError, AssetId, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, Notebook,\n    NotebookDesign, NotebookDesignId, NotebookId, Page, PageId, PageKind, PageSet, PageSetId,\n    PageState, ScanId, TrimSizeMm, TrustState,\n};\nuse rusqlite::{params, Connection, OptionalExtension};''',
        'repository imports',
    )
    insertion = 'use crate::json_columns::{decode_json, encode_json};\n'
    text = replace_once(
        text,
        insertion,
        insertion + '\nmod capture;\n\npub use capture::{AssetRepository, AuditEventRepository, OcrRunRepository, ScanRepository};\n',
        'repository module insertion',
    )
    marker = '// ---------------------------------------------------------------------------------------------\n// Asset\n// ---------------------------------------------------------------------------------------------\n'
    start = text.index(marker)
    moved = text[start:]
    text = text[:start].rstrip() + '\n'
    path.write_text(text)

    capture = ROOT / 'crates/a2d-storage/src/repository/capture.rs'
    capture.parent.mkdir(parents=True, exist_ok=True)
    capture.write_text(
        '''//! Asset, scan, OCR, and audit repository implementations.\n\nuse a2d_domain::{\n    A2dError, Asset, AssetId, AuditEvent, AuditEventId, ErrorCategory, ErrorCode, ErrorSeverity,\n    OcrRun, OcrRunId, PageId, PhysicalCopyId, Provenance, Scan, ScanId,\n};\nuse rusqlite::{params, Connection, OptionalExtension};\n\nuse crate::json_columns::{decode_json, encode_json};\n\nuse super::{corrupt_enum_error, map_sql_error};\n\n''' + moved
    )


refactor_entities()
refactor_repository()
