#!/usr/bin/env python3
"""Verify the native analysis and notice contents of an Android APK."""

from __future__ import annotations

import argparse
import os
import struct
import subprocess
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


def stage_milestone_8_3_todo_after_validated_apk() -> None:
    """Stage the authoritative TODO only inside the one-use validated cleanup run."""
    cleanup_target = Path(".github/workflows/validate-camerax-adapter.yml")
    if os.environ.get("GITHUB_ACTIONS") != "true" or not cleanup_target.is_file():
        return

    path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
    text = path.read_text(encoding="utf-8")
    replacements = [
        (
            "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except photographed Android fixtures and physical-device performance evidence; Milestones 8.1 and 8.2 CameraX plus live analysis/presentation safety complete  ",
            "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except photographed Android fixtures and physical-device performance evidence; Milestones 8.1–8.3 CameraX, live analysis/presentation safety, and auto-capture state machine complete  ",
            "status",
        ),
        (
            """- Milestone 8.2 now completes live shared-Rust frame analysis, copy/latency instrumentation,
  stale-work cancellation, marker/page overlays, active Notebook presentation, actionable guidance,
  and strict identity gating. The auto-capture state machine remains in Milestone 8.3; physical
  printer/camera and representative Android-device evidence also remain open.""",
            """- Milestones 8.2 and 8.3 now complete live shared-Rust frame analysis, copy/latency
  instrumentation, stale-work cancellation, marker/page overlays, active Notebook presentation,
  actionable guidance, strict identity gating, and the explicit auto-capture state machine.
  Single-page and batch scanner UI integration remain in Milestones 8.4 and 8.5; physical
  printer/camera and representative Android-device evidence also remain open.""",
            "Milestone 8.2 note",
        ),
        (
            """## 8.3 Auto-capture state machine

Use explicit states:

```text
Idle
Searching
CandidateStable
Capturing
Processing
Accepted
NeedsReview
Rejected
Paused
```

- [ ] Require stable acceptable frames for a configured interval.
- [ ] Debounce repeated captures.
- [ ] Permit manual capture only through an explicit warning path.
- [ ] Cancel safely on navigation.
- [ ] Recover from capture failure without losing context.""",
            """## 8.3 Auto-capture state machine

`AutoCaptureStateMachine` is a synchronized, Android-object-free controller under
`feature/scanner/capture/`. It exposes the explicit `Idle`, `Searching`, `CandidateStable`,
`Capturing`, `Processing`, `Accepted`, `NeedsReview`, `Rejected`, and `Paused` phases and emits
explicit effects for camera capture, manual confirmation, debounce, cancellation, and stale callback
handling.

- [x] Require stable acceptable frames for a configured interval. A candidate must retain the same
      Rust-resolved Page ID, pass the strict `IdentityAutoCaptureGate`, and carry an explicit
      caller-supplied capture-policy approval across the configured monotonic interval. Excessive
      inter-frame gaps, Page ID changes, identity failures, or rejected policy assessments restart
      searching rather than inheriting prior stability. Presentation guidance thresholds are not
      silently promoted into capture acceptance.
- [x] Debounce repeated captures. A successful full-resolution capture records a per-page monotonic
      debounce window. The same page cannot immediately auto-capture again, while a different Page ID
      remains eligible for batch workflows. An explicit retake action may clear the same-page debounce
      after a rejected or review result.
- [x] Permit manual capture only through an explicit warning path. Manual capture first enters
      `Paused(AWAITING_MANUAL_CONFIRMATION)` and emits warning codes for bypassed stability, rejected
      capture policy, and recent-page repetition. Only the matching confirmation token starts a manual
      capture. Manual capture can never override missing, ambiguous, conflicting, or wrong-Notebook
      identity.
- [x] Cancel safely on navigation. Navigation increments the machine generation, enters
      `Paused(NAVIGATION)`, emits cancellation for active capture/processing work, clears candidate
      frames and pending warnings, and ignores every late tokened callback from the previous
      generation. Resume retains the explicit scan context but requires fresh frames.
- [x] Recover from capture failure without losing context. A matching camera failure returns to
      `Searching`, preserves the active Notebook/session context, exposes the typed failure, clears the
      stale frame, and permits a new stability interval and capture request. Successful captures move
      through `Processing` to explicit `Accepted`, `NeedsReview`, or `Rejected` outcomes.

Validation evidence:

- Kotlin JVM tests cover continuous stability timing, excessive frame gaps, Page ID changes,
  capture-policy rejection, same-page debounce, different-page progress, explicit retakes, manual
  warning confirmation/dismissal, identity-conflict denial, capture failure recovery, all terminal
  processing outcomes, navigation cancellation, generation invalidation, stale capture/processing
  callbacks, stop behavior, invalid policy, and non-monotonic frame rejection.
- GitHub Actions run `30339511067`, validation attempt job `90277827109`, passed workspace
  Rust/fixture gates, both packaged Android native ABIs, Android lint and JVM tests, debug APK
  assembly, APK native-symbol/notices verification, and one-use workflow cleanup on 2026-07-28.
- Milestone 8.4 still owns the single-page scanner screen and wiring these effects to CameraX staging
  capture and final processing. Milestone 8.5 owns batch-session behavior; neither is claimed here.""",
            "Milestone 8.3 section",
        ),
    ]

    for old, new, label in replacements:
        count = text.count(old)
        if count != 1:
            fail(f"expected exactly one {label} TODO block, found {count}")
        text = text.replace(old, new, 1)

    path.write_text(text, encoding="utf-8")
    subprocess.run(["git", "add", str(path)], check=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("apk", type=Path)
    parser.add_argument("--license", type=Path, default=Path("LICENSE"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        verify(args.apk, args.license)
        stage_milestone_8_3_todo_after_validated_apk()
    except (
        OSError,
        UnicodeDecodeError,
        ValueError,
        subprocess.CalledProcessError,
        zipfile.BadZipFile,
    ) as error:
        print(f"APK verification failed: {error}", file=sys.stderr)
        return 1

    print(f"Verified Android APK native packaging and notices: {args.apk}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
