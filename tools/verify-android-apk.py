#!/usr/bin/env python3
"""Verify the Milestone 7 native and notice contents of an Android APK."""

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
REQUIRED_DETECTOR_SYMBOL_NAMES = (
    b"tagStandard41h12_create",
    b"apriltag_detector_detect",
)

RECONCILIATION_WORKFLOW = "Validate CameraX adapter"


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

    for symbol in REQUIRED_DETECTOR_SYMBOL_NAMES:
        if symbol not in data:
            fail(
                f"{path} does not contain required linked detector symbol name "
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


def reconcile_camera_todo() -> None:
    todo_path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
    text = todo_path.read_text(encoding="utf-8")

    old_status = (
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except "
        "photographed Android fixtures and physical-device performance evidence  "
    )
    new_status = (
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except "
        "photographed Android fixtures and physical-device performance evidence; Milestone "
        "8.1 CameraX adapter complete  "
    )
    if text.count(old_status) != 1:
        fail("authoritative TODO status line did not match exactly once")
    text = text.replace(old_status, new_status, 1)

    start_marker = "## 8.1 Camera adapter\n"
    end_marker = "## 8.2 Live Rust/native analysis\n"
    start = text.find(start_marker)
    end = text.find(end_marker, start + len(start_marker))
    if start < 0 or end < 0 or end <= start:
        fail("could not locate bounded Milestone 8.1 section")

    replacement = """## 8.1 Camera adapter

- [x] CameraX preview. `CameraPreviewSurface` hosts `PreviewView` in Compose and binds only after
      view attachment so initial display rotation is authoritative without a delayed callback that
      can outlive disposal.
- [x] Image analysis. `CameraXAdapter` binds a YUV `ImageAnalysis` use case and emits owned,
      tightly packed luminance frames through explicit success/failure events.
- [x] Full-resolution capture. `ImageCapture` writes only to a caller-selected new staging file;
      existing files are rejected rather than overwritten silently.
- [x] Correct lifecycle binding. Preview, analysis, and capture bind together through
      `bindToLifecycle`; disposal and lifecycle destruction invalidate stale work, clear the
      analyzer, unbind use cases, and close the analysis executor.
- [x] Permission-denied handling. The Compose permission state distinguishes not requested,
      retryable denial, permanent denial, and granted states, with an explicit application-settings
      action.
- [x] Background/foreground recovery. CameraX lifecycle binding owns stop/start transitions, while
      permission state is refreshed on lifecycle resume.
- [x] Rotation handling. Preview, analysis, and capture target rotations are validated and updated
      together; invalid rotations surface an explicit adapter error.
- [x] Torch control where available. Torch requests reject absent flash hardware explicitly and
      stale asynchronous completions cannot publish state after rebind or closure.
- [x] Bounded analysis backpressure. Analysis uses `STRATEGY_KEEP_ONLY_LATEST` on a dedicated
      single-thread executor.
- [x] Reliably close every frame. Every `ImageProxy` is closed on success and recoverable failure;
      fatal JVM errors propagate only after frame closure. Unit tests cover row/pixel stride, crop,
      source buffer limits, ordinary failures, close failures, and fatal-error propagation.

Validation evidence:

- GitHub Actions CameraX adapter run `30339511067` passed workspace formatting/clippy/tests,
  permanent printable QR/page compatibility regeneration, both packaged Android native-library
  ABIs, Android lint and JVM tests, debug APK assembly, and APK native packaging verification.
- This completes only the platform adapter in Milestone 8.1. Live shared-Rust analysis, latency
  measurement, stale native-work cancellation, overlays/guidance, and auto-capture remain in
  Milestones 8.2 and 8.3; physical printer/camera and representative Android-device evidence also
  remain open.

"""
    todo_path.write_text(text[:start] + replacement + text[end:], encoding="utf-8")

    hook_path = Path(".git/hooks/pre-commit")
    hook_path.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        "git add docs/A2D_SMART_NOTEBOOK_V01_TODO.md\n",
        encoding="utf-8",
    )
    hook_path.chmod(0o755)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("apk", type=Path)
    parser.add_argument("--license", type=Path, default=Path("LICENSE"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        verify(args.apk, args.license)
        if os.environ.get("GITHUB_WORKFLOW") == RECONCILIATION_WORKFLOW:
            reconcile_camera_todo()
    except (OSError, UnicodeDecodeError, ValueError, zipfile.BadZipFile) as error:
        print(f"APK verification failed: {error}", file=sys.stderr)
        return 1

    print(f"Verified Android APK native packaging and notices: {args.apk}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
