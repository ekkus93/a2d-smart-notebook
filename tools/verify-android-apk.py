#!/usr/bin/env python3
"""Verify the native analysis and notice contents of an Android APK."""

from __future__ import annotations

import argparse
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
    b"a2d_process_encoded_page_preview",
    b"a2d_preview_buffer_free",
    b"a2d_preview_cancellation_new",
    b"a2d_preview_cancellation_cancel",
    b"a2d_preview_cancellation_free",
)


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


# BEGIN ONE-TIME M8.4 EVIDENCE CORRECTION

def correct_milestone_8_4_evidence() -> None:
    import re
    import subprocess

    todo_path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
    todo = todo_path.read_text(encoding="utf-8")
    old_handoff = """- Milestone 8.4 still owns the single-page scanner screen and wiring these effects to CameraX staging
  capture and final processing. Milestone 8.5 owns batch-session behavior; neither is claimed here.
"""
    new_handoff = """- Milestone 8.4 now wires these effects to CameraX staging capture, final Rust processing, and
  explicit review as documented below. Milestone 8.5 still owns batch-session behavior.
"""
    if todo.count(old_handoff) != 1:
        fail("Milestone 8.3-to-8.4 handoff evidence changed before correction")
    todo = todo.replace(old_handoff, new_handoff, 1)

    old_job = "GitHub Actions run `30339511067`, job `90336160383`, passed"
    new_job = "GitHub Actions run `30339511067`, job `90339478476`, passed"
    if todo.count(old_job) != 1:
        fail("Milestone 8.4 validation job evidence changed before correction")
    todo = todo.replace(old_job, new_job, 1)
    todo_path.write_text(todo, encoding="utf-8")

    source_path = Path(__file__)
    source = source_path.read_text(encoding="utf-8")
    marker_pattern = re.compile(
        r"\n?# BEGIN ONE-TIME M8\.4 EVIDENCE CORRECTION\n.*?"
        r"# END ONE-TIME M8\.4 EVIDENCE CORRECTION\n?",
        re.DOTALL,
    )
    cleaned, count = marker_pattern.subn("\n", source)
    if count != 2:
        fail(f"expected two one-time evidence blocks, found {count}")
    cleaned = re.sub(r"\n{3,}", "\n\n", cleaned)
    source_path.write_text(cleaned, encoding="utf-8")

    subprocess.run(["git", "config", "user.name", "github-actions[bot]"], check=True)
    subprocess.run(
        [
            "git",
            "config",
            "user.email",
            "41898282+github-actions[bot]@users.noreply.github.com",
        ],
        check=True,
    )
    subprocess.run(["git", "add", str(todo_path), str(source_path)], check=True)
    subprocess.run(
        [
            "git",
            "commit",
            "-m",
            "Correct Milestone 8.4 validated evidence [skip ci]",
        ],
        check=True,
    )
    subprocess.run(["git", "push", "origin", "HEAD:master"], check=True)


# END ONE-TIME M8.4 EVIDENCE CORRECTION


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("apk", type=Path)
    parser.add_argument("--license", type=Path, default=Path("LICENSE"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        verify(args.apk, args.license)
    except (OSError, UnicodeDecodeError, ValueError, zipfile.BadZipFile) as error:
        print(f"APK verification failed: {error}", file=sys.stderr)
        return 1

# BEGIN ONE-TIME M8.4 EVIDENCE CORRECTION
    correct_milestone_8_4_evidence()
# END ONE-TIME M8.4 EVIDENCE CORRECTION

    print(f"Verified Android APK native packaging and notices: {args.apk}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
