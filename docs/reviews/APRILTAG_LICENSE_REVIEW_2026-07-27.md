# AprilTag detector dependency license review

**Date:** 2026-07-27  
**Milestone:** 7.1  
**Reviewed dependency:** `apriltag-sys = 0.4.0` and its bundled `apriltag-src` source tree  
**Project license:** Apache License 2.0

## Conclusion

The selected dependency is acceptable for A2D Smart Notebook v0.1 from an engineering license-compliance perspective.

- `apriltag-sys 0.4.0` declares `BSD-2-Clause`.
- The bundled official AprilRobotics AprilTag C source also uses the BSD 2-Clause License.
- BSD-2-Clause code may be redistributed in an Apache-2.0 application, provided the BSD copyright notice, conditions, and disclaimer are retained as required.
- The dependency does not impose copyleft requirements on A2D source code.

This is a project engineering review, not legal advice.

## Evidence reviewed

1. The published `apriltag-sys 0.4.0` Cargo manifest declares:
   - license: `BSD-2-Clause`
   - repository: `https://github.com/jerry73204/apriltag-rust.git`
   - bundled build dependencies used to compile the native source.
2. The crate contains an `apriltag-src/` tree with the official AprilTag C implementation, including `tagStandard41h12.c` and `LICENSE.md`.
3. The official AprilRobotics repository identifies the project as AprilTag 3, recommends `tagStandard41h12` for most applications, and publishes the source under the BSD 2-Clause License.
4. A2D's `deny.toml` already permits `BSD-2-Clause`; `cargo deny check` remains a required CI gate.

Primary references:

- `https://docs.rs/crate/apriltag-sys/0.4.0/source/Cargo.toml`
- `https://docs.rs/crate/apriltag-sys/0.4.0/source/apriltag-src/`
- `https://github.com/AprilRobotics/apriltag`
- `https://github.com/AprilRobotics/apriltag/blob/master/LICENSE.md`

## Distribution obligations

Before the first distributed APK or source release containing the detector:

1. Preserve the AprilTag BSD 2-Clause copyright notice, license conditions, and disclaimer.
2. Preserve the `apriltag-sys` license notice.
3. Include those notices in a repository-level third-party notices file and in the Android application's open-source notices or equivalent distributed documentation.
4. Do not describe AprilRobotics or the University of Michigan as endorsing A2D.
5. Re-run this review if the crate version, bundled upstream source, or build method changes.

The release-notice work is intentionally tracked as a release requirement rather than falsely considered complete merely because the dependency compiles.

## Supply-chain and reproducibility controls

- `apriltag-sys` is pinned exactly to `0.4.0` rather than a semver range.
- `Cargo.lock` records the resolved package and checksum.
- `.cargo/config.toml` sets `APRILTAG_SYS_METHOD = "raw,static"`, forcing compilation of the crate's bundled C sources.
- The build must not silently use a workstation-installed `pkg-config` AprilTag library.
- CI compiles the dependency directly for the supported Android ABIs and checks future iOS target feasibility.

## Decision impact

The license does not create a packaging reason to reject the official detector. A pure-Rust alternative therefore does not need to replace it merely for licensing reasons. A different implementation should be considered only if reproducible Android/iOS builds, safety, performance, or binary packaging prove materially worse during validation.
