# Scan fixture corpus

This directory contains Milestone 7.8 scan-processing fixtures.

## Generated fixtures

`tools/generate_scan_fixtures.py` creates deterministic synthetic controls using:

- official `tagStandard41h12` marker rasters produced by the fixture-only Rust helper,
- canonical A2D Page Code payloads produced by the existing Rust encoder,
- Pillow-based page composition and deterministic camera-like transforms.

Generated fixtures exercise rotation, perspective, blur, glare, exposure, missing markers, wrong layouts, duplicate markers, revisions, and corrupt input. Their source is project code and their license is Apache-2.0. `manifest.json` records the generator version, identity, expected marker roles, intended quality state, warnings, dimensions, byte length, SHA-256 digest, and transform parameters for each asset.

The intended quality state in the synthetic manifest is a fixture expectation for test design. It is **not** a production threshold recommendation. Production thresholds remain versioned configuration and must be calibrated with real Android captures.

## Photographed fixtures

`photographed/` is reserved for real camera captures. The synthetic generator must never delete or overwrite that directory. Every photographed fixture must include explicit source/consent/license metadata and device/capture conditions.

Synthetic fixtures do not satisfy the photographed-device evidence required to accept ADR 0002 or complete Milestone 7.

## Regeneration

From the repository root:

```bash
cargo run -p a2d-image --features fixture-tools --bin a2d-fixture-support -- target/fixture-support
python3 -m pip install -r tools/scan-fixtures-requirements.txt
python3 tools/generate_scan_fixtures.py \
  --support-dir target/fixture-support \
  --output-dir fixtures/scans
```
