from pathlib import Path

path = Path("docs/A2D_SMART_NOTEBOOK_V01_TODO.md")
text = path.read_text()


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one match, found {count}: {old[:160]!r}")
    text = text.replace(old, new)


replace_once(
    '''- [x] Accept/retake/details. Review displays the corrected image, selected Notebook, captured Page ID,
      pipeline version, identity result, quality warnings, and retake/details actions. Accept means
      only “approved for registration”; the screen explicitly says the page is not saved because
      Milestone 9 durable registration does not yet exist.
''',
    '''- [x] Accept/save/retake/details. Review displays the corrected image, selected Notebook, captured
      Page ID, pipeline version, identity result, quality warnings, and retake/details actions. Save
      Scan sends the retained full-resolution Page Code payload, final marker roles/IDs, EXIF rotation,
      capture timestamp, warnings, fixed Page ID, and fixed Notebook ID to the typed Rust registration
      API. The UI says “Scan saved” only after Rust returns a `RegisteredScan`; failed registration
      preserves the staging file and remains explicitly retryable.
''',
)

replace_once(
    '''- [x] Processing progress/cancel. Full-resolution work runs off the main thread with a Rust
      cancellation token. `close()` requests cancellation immediately but defers native-token freeing
      until every synchronous JNA borrower returns, preventing stale completion and use-after-free;
      cancellation, rejection, cleanup failure, and processing failure remain explicit.
''',
    '''- [x] Processing progress/cancel. Full-resolution work runs off the main thread with a Rust
      cancellation token. `close()` requests cancellation immediately but defers native-token freeing
      until every synchronous JNA borrower returns, preventing stale completion and use-after-free;
      cancellation, rejection, cleanup failure, and processing failure remain explicit.
- [x] Durable registration progress. Navigation, Notebook switching, retake, and duplicate save
      requests are blocked while synchronous Rust registration is active. ViewModel cleanup never
      deletes the staging capture while registration is in progress, and registration success is
      always surfaced rather than discarded by a stale presentation-generation check.
''',
)

replace_once(
    '''- `tools/verify-android-apk.py` permanently requires the live-analysis and full-resolution preview,
  buffer-free, and cancellation symbols in both packaged ABIs. Physical-device usability/performance
  evidence and calibrated automatic-capture thresholds remain open; production automatic capture is
  therefore still disabled rather than relying on an invented threshold.
''',
    '''- `tools/verify-android-apk.py` permanently requires the live-analysis and full-resolution preview,
  buffer-free, cancellation, and durable-registration binding symbols in both packaged ABIs.
- GitHub Actions run `30403855033` applied the registration lifecycle hardening, then passed focused
  Android unit tests and lint before publishing the clean source commit.
- Physical-device usability/performance evidence and calibrated automatic-capture thresholds remain
  open; production automatic capture is therefore still disabled rather than relying on an invented
  threshold.
''',
)

replace_once(
    '''- [x] UI does not claim a page is saved until Rust confirms durable registration. Milestone 8.4 uses
      an explicit approved-for-registration/not-saved state; actual registration remains Milestone 9.
''',
    '''- [x] UI does not claim a page is saved until Rust confirms durable registration. The corrected
      review remains explicitly unsaved until `A2dClient.registerScan` succeeds, and only the returned
      typed `RegisteredScan` populates the saved result, warnings, and required actions.
''',
)

replace_once(
    '''## 9.1 Final scan registration

The Rust request includes captured path, capture source, parsed code, marker detections, layout ID, quality metrics, active Notebook, and timestamp.

- [ ] Validate staging path.
- [ ] Reparse and validate Page Code.
- [ ] Validate markers against layout.
- [ ] Resolve page identity.
- [ ] Process images.
- [ ] Register files and database records transactionally.
- [ ] Return typed warnings and required actions.
''',
    '''## 9.1 Final scan registration

The Android review artifact retains the Rust-resolved Page ID and Notebook, full-resolution Page Code
payload, final marker roles/IDs, image format and EXIF rotation, capture source and timestamp, and
explicit preview warnings. Rust treats every field as untrusted input and reopens, revalidates, and
reprocesses the staged image before any durable success is possible.

- [x] Validate staging path. Rust accepts only a regular, non-symlink file canonicalized beneath the
      library-owned `tmp/scanner-staging/` directory, bounds encoded size, rejects concurrent file
      changes, and never deletes an external or invalid source path.
- [x] Reparse and validate Page Code. Rust reparses canonical grammar/version/layout/CRC and rejects an
      unresolved, conflicting, or changed Page Code before committing assets.
- [x] Validate markers against layout. Full-resolution AprilTag detection and semantic role resolution
      rerun in Rust; exactly one TL/TR/BR/BL marker set must match the reviewed marker identities.
- [x] Resolve page identity. The reparsed code must resolve to the exact approved Page ID and fixed
      active Notebook, and the stored page record/layout must still agree inside the registration
      transaction.
- [x] Process images. Rust performs bounded decode, quality measurement, rectification, corrected-color
      generation, OCR-image generation, and thumbnail generation using the shared versioned pipeline.
- [x] Register files and database records transactionally. Original and derived assets are committed
      through the immutable asset store under a durable append-only filesystem journal; asset rows,
      scan row, page transition, preferred-scan invariant, and audit event commit in one SQLite
      transaction. Interrupted or failed commits retain the journal and staging path for explicit
      recovery and never return a saved result.
- [x] Return typed warnings and required actions. Success returns IDs and resolved paths for all assets,
      quality status, preferred/version status, typed quality/cleanup warnings, and explicit actions
      for existing-page review or incomplete cleanup.

Android integration:

- [x] CameraX writes directly into Rust's private scanner-staging directory rather than an unrelated
      cache directory.
- [x] Save Scan calls the generated UniFFI registration API off the main thread and shows durable
      success only after Rust returns.
- [x] Registration failure leaves the reviewed capture available for retry; retake deletes only an
      unregistered staging file and never touches committed originals.
- [x] Registration blocks destination changes and navigation, while lifecycle cleanup preserves a file
      that may still be borrowed by synchronous Rust registration.

Validation evidence:

- Rust tests cover first registration, preferred-page transition, existing-page rescan preservation,
  staging-path confinement, changed marker rejection, and retained interruption journals.
- Kotlin JVM tests cover request/rotation/marker/warning mapping and the tokened transition from review
  to accepted only after durable registration.
- GitHub Actions run `30403855033` passed focused Android unit tests and lint after applying the final
  lifecycle hardening. Permanent PR CI remains the merge gate for workspace Rust checks, dependency
  policy, both Android ABIs, binding drift, lint/unit/APK verification, and emulator integration.

Milestones 9.2–9.5 remain open; this section does not claim fingerprint comparison, revision decisions,
Needs Review resolution APIs, or version UI.
''',
)

path.write_text(text)
