#!/usr/bin/env python3
from pathlib import Path

path = Path("crates/a2d-image/src/detector.rs")
text = path.read_text(encoding="utf-8")

replacements = [
    (
        "impl DetectorConfig {\n    fn validate(self) -> Result<Self, A2dError> {\n",
        "trait NativeBoolean {\n"
        "    fn from_bool(value: bool) -> Self;\n"
        "}\n\n"
        "impl NativeBoolean for bool {\n"
        "    fn from_bool(value: bool) -> Self {\n"
        "        value\n"
        "    }\n"
        "}\n\n"
        "impl NativeBoolean for i32 {\n"
        "    fn from_bool(value: bool) -> Self {\n"
        "        i32::from(value)\n"
        "    }\n"
        "}\n\n"
        "impl DetectorConfig {\n"
        "    fn validate(self) -> Result<Self, A2dError> {\n",
    ),
    (
        "native.refine_edges = if config.refine_edges { 1 } else { 0 };",
        "native.refine_edges = NativeBoolean::from_bool(config.refine_edges);",
    ),
    (
        "native.debug = 0;",
        "native.debug = NativeBoolean::from_bool(false);",
    ),
    (
        "    #[test]\n"
        "    fn rejects_invalid_native_detector_configuration_before_allocation() {\n",
        "    #[test]\n"
        "    fn native_boolean_mapping_is_portable_across_generated_binding_types() {\n"
        "        assert!(<bool as NativeBoolean>::from_bool(true));\n"
        "        assert!(!<bool as NativeBoolean>::from_bool(false));\n"
        "        assert_eq!(<i32 as NativeBoolean>::from_bool(true), 1);\n"
        "        assert_eq!(<i32 as NativeBoolean>::from_bool(false), 0);\n"
        "    }\n\n"
        "    #[test]\n"
        "    fn rejects_invalid_native_detector_configuration_before_allocation() {\n",
    ),
]

for old, new in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one occurrence, found {count}: {old!r}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
print(f"updated {path}")
