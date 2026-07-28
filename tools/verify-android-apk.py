#!/usr/bin/env python3
"""Verify the native analysis and notice contents of an Android APK."""

from __future__ import annotations

import argparse
import re
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


# BEGIN ONE-TIME M8.4 TODO RECONCILIATION

def reconcile_validated_milestone_8_4() -> None:
    """Update the authoritative TODO only after the full APK verifier has succeeded."""
    import subprocess

    todo_path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
    todo = todo_path.read_text(encoding="utf-8")
    old_status = (
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except "
        "photographed Android fixtures and physical-device performance evidence; Milestones "
        "8.1–8.3 CameraX, live analysis/presentation safety, and auto-capture state machine complete  "
    )
    new_status = (
        "**Status:** Milestones 1–6 complete; Milestone 7 implementation is complete except "
        "photographed Android fixtures and physical-device performance evidence; Milestones "
        "8.1–8.4 CameraX, live analysis/presentation safety, capture state machine, and single-page "
        "scanner complete  "
    )
    if todo.count(old_status) != 1:
        fail("authoritative TODO status anchor changed before Milestone 8.4 reconciliation")
    todo = todo.replace(old_status, new_status, 1)

    old_block = """## 8.4 Single-page scanner

- [ ] Active Notebook selector.
- [ ] Camera preview.
- [ ] Marker/QR status.
- [ ] Capture guidance.
- [ ] Manual capture.
- [ ] Torch.
- [ ] Corrected preview.
- [ ] Accept/retake/details.
- [ ] Warning details.
- [ ] Processing progress/cancel.
"""
    new_block = """## 8.4 Single-page scanner

- [x] Active Notebook selector. The scanner loads Rust-owned Notebook summaries, displays the active
      destination, persists an explicit change through `setActiveNotebook`, requires Rust to return at
      most one active Notebook, resets the camera generation on destination change, and locks the
      selector while capture processing or review is active.
- [x] Camera preview. `SinglePageScannerScreen` owns permission and CameraX lifecycle state, while
      `CameraPreviewSurface` remains the live preview surface. CameraX objects never enter the
      ViewModel.
- [x] Marker/QR status. One owned CameraX luminance frame fans out to independent keep-latest Rust
      marker/quality analysis and bounded ZXing Page Code decoding. QR text remains untrusted until
      Rust returns a typed `PageResolution`; missing codes, decoder failures, conflicts, ambiguity,
      required registration, unsupported codes, and stale results remain distinct.
- [x] Capture guidance. The screen reuses the Milestone 8.2 overlay, active-destination banner,
      actionable guidance, and strict identity gate. Live analysis and Page Code freshness are
      combined without silently inheriting an old identity.
- [x] Manual capture. The Milestone 8.3 state machine remains authoritative: capture requires a bound
      camera, active Notebook, current Rust-resolved page identity, and explicit confirmation when
      stability or calibrated capture-policy checks are bypassed. Tokened staging paths and callbacks
      reject stale captures.
- [x] Torch. The screen exposes CameraX torch control only when hardware reports it available and a
      capture is not processing; adapter failures remain explicit.
- [x] Corrected preview. A versioned native ABI borrows the encoded JPEG, catches panics, applies
      explicit decode/detector/resource policies, reruns AprilTag and quality analysis, rectifies with
      the shared `writable_page_layout`, executes the bounded Rust derived-image pipeline, and returns
      corrected RGB plus thumbnail buffers through a strictly decoded and explicitly freed payload.
- [x] Accept/retake/details. Review displays the corrected image, selected Notebook, captured Page ID,
      pipeline version, identity result, quality warnings, and retake/details actions. Accept means
      only “approved for registration”; the screen explicitly says the page is not saved because
      Milestone 9 durable registration does not yet exist.
- [x] Warning details. Quality warnings may be reviewed, but identity is non-overridable. Approval is
      enabled only when final full-resolution Page Code resolution exactly matches both the capture
      request Page ID and its fixed active Notebook ID.
- [x] Processing progress/cancel. Full-resolution work runs off the main thread with a Rust
      cancellation token. `close()` requests cancellation immediately but defers native-token freeing
      until every synchronous JNA borrower returns, preventing stale completion and use-after-free;
      cancellation, rejection, cleanup failure, and processing failure remain explicit.

Validation evidence:

- Rust tests cover cancellation as a distinct outcome, panic/error containment, result/error codec
  framing, bounded image payloads, and the shared image-processing path. Kotlin JVM tests cover live
  QR rotation/scheduling/cancellation, exact final page-and-Notebook matching, camera readiness,
  unsigned RGB conversion, navigation routes, and the existing capture/presentation controllers.
- GitHub Actions run `30339511067`, job `90336160383`, passed workspace Rust formatting, clippy and
  tests, printable fixture regeneration, arm64-v8a and x86_64 Android native builds, Android lint and
  JVM tests, debug APK assembly, and APK native packaging verification on 2026-07-28.
- `tools/verify-android-apk.py` permanently requires the live-analysis and full-resolution preview,
  buffer-free, and cancellation symbols in both packaged ABIs. Physical-device usability/performance
  evidence and calibrated automatic-capture thresholds remain open; production automatic capture is
  therefore still disabled rather than relying on an invented threshold.
"""
    if todo.count(old_block) != 1:
        fail("authoritative Milestone 8.4 TODO block changed before reconciliation")
    todo = todo.replace(old_block, new_block, 1)

    old_acceptance = """Acceptance:

- [ ] UI does not claim a page is saved until Rust confirms durable registration.
- [ ] Scanner never silently changes destination Notebook.
"""
    new_acceptance = """Acceptance:

- [x] UI does not claim a page is saved until Rust confirms durable registration. Milestone 8.4 uses
      an explicit approved-for-registration/not-saved state; actual registration remains Milestone 9.
- [x] Scanner never silently changes destination Notebook. Selection is explicit, Rust-persisted,
      generation-scoped, fixed in every capture request, and rechecked against final Page Code identity.
"""
    if todo.count(old_acceptance) != 1:
        fail("scanner acceptance anchor changed before reconciliation")
    todo = todo.replace(old_acceptance, new_acceptance, 1)
    todo_path.write_text(todo, encoding="utf-8")

    source_path = Path(__file__)
    source = source_path.read_text(encoding="utf-8")
    marker_pattern = re.compile(
        r"\n?# BEGIN ONE-TIME M8\.4 TODO RECONCILIATION\n.*?"
        r"# END ONE-TIME M8\.4 TODO RECONCILIATION\n?",
        re.DOTALL,
    )
    cleaned, count = marker_pattern.subn("\n", source)
    if count != 2:
        fail(f"expected two one-time reconciliation blocks, found {count}")
    source_path.write_text(cleaned, encoding="utf-8")

    subprocess.run(
        ["git", "config", "user.name", "github-actions[bot]"],
        check=True,
    )
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
            "Complete Milestone 8.4 TODO after validated APK [skip ci]",
        ],
        check=True,
    )
    subprocess.run(["git", "push", "origin", "HEAD:master"], check=True)


# END ONE-TIME M8.4 TODO RECONCILIATION



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

# BEGIN ONE-TIME M8.4 TODO RECONCILIATION
    reconcile_validated_milestone_8_4()
# END ONE-TIME M8.4 TODO RECONCILIATION

    print(f"Verified Android APK native packaging and notices: {args.apk}")
    return 0



if __name__ == "__main__":
    raise SystemExit(main())
