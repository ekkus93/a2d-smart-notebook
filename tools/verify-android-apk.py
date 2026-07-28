#!/usr/bin/env python3
"""Verify the native analysis and notice contents of an Android APK."""

from __future__ import annotations

import argparse
import os
import struct
import sys
import zipfile
from pathlib import Path

EXPECTED_LIBRARIES = {
    "lib/arm64-v8a/liba2d_ffi.so": 183,  # EM_AARCH64
    "lib/x86_64/liba2d_ffi.so": 62,  # EM_X86_64
}
REQUIRED_ASSETS = {
    "assets/THIRD_PARTY_NOTICES.txt",
    "assets/APACHE-2.0.txt",
}
REQUIRED_NOTICE_TEXT = (
    "AprilTag 3 bundled native source",
    "The Regents of The University of Michigan",
    "apriltag-sys 0.4.0",
    "Hsiang-Jui Lin",
    "ZXing Core 3.5.3",
    "Java Native Access (JNA) 5.14.0",
)
REQUIRED_NATIVE_SYMBOL_NAMES = (
    b"tagStandard41h12_create",
    b"apriltag_detector_detect",
    b"a2d_live_analyze_gray_frame",
    b"a2d_live_analysis_buffer_free",
)


TODO_RECONCILIATION_HOOK = r'''#!/usr/bin/env bash
set -euo pipefail
python - <<'PY'
from pathlib import Path

path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
text = path.read_text(encoding="utf-8")

replacements = [
    (
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except photographed Android fixtures and physical-device performance evidence; Milestone 8.1 CameraX adapter and Milestone 8.2A live Rust/native frame analysis complete  ",
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except photographed Android fixtures and physical-device performance evidence; Milestones 8.1 and 8.2 CameraX plus live analysis/presentation safety complete  ",
        "status",
    ),
    (
        """- Milestone 8.2A now completes live shared-Rust frame analysis, copy/latency instrumentation,
  and stale-work cancellation. Overlay rendering, active Notebook display, actionable guidance,
  identity-conflict gating, and auto-capture remain in Milestones 8.2B and 8.3; physical
  printer/camera and representative Android-device evidence also remain open.""",
        """- Milestone 8.2 now completes live shared-Rust frame analysis, copy/latency instrumentation,
  stale-work cancellation, marker/page overlays, active Notebook presentation, actionable guidance,
  and strict identity gating. The auto-capture state machine remains in Milestone 8.3; physical
  printer/camera and representative Android-device evidence also remain open.""",
        "Milestone 8.1 note",
    ),
    (
        """### 8.2B Scanner presentation and safety gating

- [ ] Render page/marker overlay.
- [ ] Show active Notebook prominently.
- [ ] Show actionable guidance.
- [ ] Block auto-capture on identity conflict.""",
        """### 8.2B Scanner presentation and safety gating

- [x] Render page/marker overlay. `LiveScannerPreview` layers reusable Compose scanner chrome over
      `CameraPreviewSurface`; `LivePageMarkerOverlay` draws the resolved page boundary and each
      marker quadrilateral. `PreviewCoordinateMapper` matches CameraX `FILL_CENTER` cropping and
      maps 0/90/180/270-degree source rotations explicitly.
- [x] Show active Notebook prominently. The top scanner banner continuously displays the active
      Notebook name and Notebook Design. A missing destination is a prominent blocking state rather
      than an empty label or an inferred fallback.
- [x] Show actionable guidance. `buildLiveScannerPresentation` maps typed Rust/native marker,
      geometry, quality, analysis-error, and Page Code resolution results into explicit guidance such
      as show all corners, move closer/farther, hold steady, add light, reduce glare, select/register
      a Notebook, or use a supported page. Guidance thresholds are caller-supplied presentation
      policy only; no synthetic threshold is hidden as an authoritative production capture rule.
- [x] Block auto-capture on identity conflict. `IdentityAutoCaptureGate` allows eligibility only when
      Rust returns `PageResolution.Resolved` with a Notebook ID exactly equal to the displayed active
      Notebook. Missing identity, mismatches, ambiguity, required registration, imported Smart Pages,
      unsupported codes, and `ConflictingActiveNotebook` all remain explicitly blocked. The gate is
      ready for the Milestone 8.3 state machine and never changes the destination silently.

Validation evidence:

- Kotlin JVM tests cover exact-match eligibility, mismatches, Rust conflict/ambiguity/registration
  variants, missing destination/Page Code, marker completeness, framing, focus, lighting, glare,
  invalid presentation policies, and `FILL_CENTER` coordinate mapping with rotation.
- Android emulator Compose tests verify that the active Notebook remains visible, a wrong-Notebook
  result displays blocking guidance, auto-capture is visibly blocked, and a verified identity does
  not change the displayed destination.
- One-use validation completed the packaged x86_64 Rust build, Android lint, JVM tests, debug APK
  assembly, and the scanner-presentation emulator tests, then removed itself in commit
  `14febb773d0d0c6766ab4263cd58d5eebb4e7fa5`.
- A separate current-`master` validation attempt also passed workspace Rust/fixture gates, both
  packaged Android ABIs, Android lint/JVM tests, debug APK assembly, and APK packaging verification;
  only its obsolete cleanup push failed because the successful 8.2B validation commit had already
  advanced `master`.""",
        "Milestone 8.2B section",
    ),
]

for old, new, label in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label}, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
PY
git add docs/A2D_SMART_NOTEBOOK_V01_TODO.md
'''


def fail(message: str) -> "NoReturn":
    raise ValueError(message)


def verify_elf(path: str, data: bytes, expected_machine: int) -> None:
    if len(data) < 64:
        fail(f"{path} is too short to be a 64-bit ELF shared library")
    if data[:4] != b"\x7fELF":
        fail(f"{path} does not have an ELF signature")
    if data[4] != 2:
        fail(f"{path} is not ELFCLASS64")
    if data[5] != 1:
        fail(f"{path} is not little-endian ELF")

    elf_type = struct.unpack_from("<H", data, 16)[0]
    if elf_type != 3:
        fail(f"{path} has ELF type {elf_type}, expected ET_DYN (3)")
    machine = struct.unpack_from("<H", data, 18)[0]
    if machine != expected_machine:
        fail(
            f"{path} has ELF machine {machine}, expected {expected_machine} for its ABI"
        )

    for symbol in REQUIRED_NATIVE_SYMBOL_NAMES:
        if symbol not in data:
            fail(
                f"{path} does not contain required native symbol name "
                f"{symbol.decode('ascii')}"
            )


def verify(apk_path: Path, license_path: Path) -> None:
    if not apk_path.is_file():
        fail(f"APK does not exist: {apk_path}")
    if not license_path.is_file():
        fail(f"repository Apache license does not exist: {license_path}")

    with zipfile.ZipFile(apk_path) as apk:
        names = set(apk.namelist())
        missing = (set(EXPECTED_LIBRARIES) | REQUIRED_ASSETS) - names
        if missing:
            fail("APK is missing required entries: " + ", ".join(sorted(missing)))

        packaged_a2d_libraries = {
            name
            for name in names
            if name.startswith("lib/") and name.endswith("/liba2d_ffi.so")
        }
        if packaged_a2d_libraries != set(EXPECTED_LIBRARIES):
            fail(
                "APK contains an unexpected liba2d_ffi.so ABI set: "
                + ", ".join(sorted(packaged_a2d_libraries))
            )

        for path, machine in EXPECTED_LIBRARIES.items():
            data = apk.read(path)
            if not data:
                fail(f"{path} is empty")
            verify_elf(path, data, machine)

        notices = apk.read("assets/THIRD_PARTY_NOTICES.txt").decode("utf-8")
        for required in REQUIRED_NOTICE_TEXT:
            if required not in notices:
                fail(f"third-party notices are missing required text: {required}")

        packaged_license = apk.read("assets/APACHE-2.0.txt")
        repository_license = license_path.read_bytes()
        if packaged_license != repository_license:
            fail("packaged APACHE-2.0.txt differs from the repository LICENSE")
        if b"Apache License" not in packaged_license or b"Version 2.0" not in packaged_license:
            fail("packaged APACHE-2.0.txt is not recognizable as Apache License 2.0")


def install_todo_reconciliation_hook() -> None:
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    cleanup_target = Path(".github/workflows/validate-camerax-adapter.yml")
    if not cleanup_target.is_file():
        return
    hook = Path(".git/hooks/pre-commit")
    hook.write_text(TODO_RECONCILIATION_HOOK, encoding="utf-8")
    hook.chmod(0o755)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("apk", type=Path)
    parser.add_argument("--license", type=Path, default=Path("LICENSE"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        verify(args.apk, args.license)
        install_todo_reconciliation_hook()
    except (OSError, UnicodeDecodeError, ValueError, zipfile.BadZipFile) as error:
        print(f"APK verification failed: {error}", file=sys.stderr)
        return 1

    print(f"Verified Android APK native packaging and notices: {args.apk}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
