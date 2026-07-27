# 0003. QR image decoder boundary

**Status:** Accepted  
**Date:** 2026-07-27  
**Decision owners/authors:** A2D project

## Context and problem

Milestone 7.4 requires deciding whether QR image decoding belongs in platform code or the shared Rust/native image-processing layer. The project already has two distinct responsibilities that must not be conflated:

1. locating and decoding QR modules from pixels into text; and
2. deciding whether that text is a valid, supported A2D Page Code.

The second responsibility is security- and compatibility-sensitive. A QR library can successfully decode arbitrary text, URLs, malformed A2D-looking data, or a payload with an invalid checksum. Decoder success therefore cannot be treated as A2D acceptance.

Android already has a bounded local decoding implementation in `QrCapture.kt`, while `a2d-identity::qr::parse` is the canonical strict Page Code parser. This ADR records that split as the production v0.1 boundary rather than adding a duplicate QR image decoder to Rust.

## Constraints

- The core app must work offline and without an account.
- QR image decoding must remain local.
- Android owns camera and platform image acquisition.
- Rust owns Page Code grammar, versioning, integrity checks, layout validation, and typed acceptance or rejection.
- A decoded string must never be reinterpreted as a URL or arbitrary content after A2D validation fails.
- Image and allocation sizes must be bounded before decoding.
- Decoding and parsing failures must remain visible; no empty-string, cached-value, or fabricated-success fallback is permitted.
- CameraX live scanning remains Milestone 8 work and is not introduced by this decision.

## Options considered

### Option 1 — Decode QR pixels in the shared Rust/native layer

This could centralize the pixel decoder and potentially make distorted-image fixtures identical across platforms. It would also add another native imaging dependency, expand the mobile packaging and unsafe-code surface, and duplicate a mature capability already available on Android.

The QR payload itself is canonical uppercase ASCII text. Cross-platform compatibility is determined primarily by the shared Rust parser, not by requiring every platform to use the same pixel decoder.

### Option 2 — Decode pixels on the platform and validate payloads in Rust

Android uses a bounded local QR decoder to convert image pixels into an untrusted string. The string is then passed immediately to the canonical Rust Page Code parser. Future clients may use their platform QR decoder while sharing the same Rust acceptance logic.

This preserves a small platform adapter while keeping all A2D semantics in one implementation.

## Decision

Select **Option 2: platform QR image decoding with mandatory canonical Rust payload validation**.

For Android v0.1:

- ZXing is the local QR image decoder.
- The decoder is restricted to `BarcodeFormat.QR_CODE`.
- source dimensions are checked before allocation;
- oversized captures are rejected;
- large captures are downsampled to a bounded decode side;
- pixel decoding runs off the main thread at the call site;
- the returned text is untrusted until Rust accepts it; and
- every workflow must pass that text to the applicable Rust `a2d-identity`/`a2d-core` operation rather than acting on decoder success alone.

The shared Rust parser remains authoritative for:

- the `A2D` magic prefix;
- supported wire version;
- type code and field count;
- canonical alphabet and numeric representation;
- identifier and layout constraints;
- maximum payload length;
- CRC-32C integrity; and
- typed rejection without URL or arbitrary-content fallback.

This decision does not expose QR decoding through `a2d-image`, does not add CameraX, and does not claim that distortion fixtures are complete.

## Detailed rationale

Pixel decoding and protocol validation have different portability requirements. A platform QR library is well suited to camera/image integration, pixel formats, and local QR recognition. The A2D protocol parser is where long-lived compatibility and trust decisions reside and must remain shared.

Keeping these responsibilities separate prevents a dangerous semantic shortcut: “the QR library returned text, therefore this is an A2D page.” Decoder output is only input to validation. Rust must return a typed successful Page Code or a typed failure before any identity, notebook selection, page registration, or navigation decision occurs.

The current Android adapter also avoids Base64 and JSON. It passes the decoded text directly to Rust operations, while image bytes stay within the bounded platform decoder path.

## Security/privacy implications

All decoding is local. No image or payload is uploaded.

Platform decoder output is untrusted data. It must not trigger a browser, deep link, arbitrary command, or A2D success state. Invalid A2D-looking codes remain invalid and visible to the caller.

The Android adapter limits captured pixel count before bitmap allocation and bounds the decoder-side image dimension. These limits are safety controls, not image-quality guarantees.

Failures must not be converted into a previous successful payload, an empty valid value, or a generic success state.

## Portability implications for Android and future iOS

Android uses ZXing because it is already integrated and proven by emulator instrumentation and the Milestone 6 setup-code flow.

A future iOS client may use Vision or AVFoundation for pixel decoding, but it must submit the resulting text to the same Rust parser and obey the same rejection semantics. This is a future portability constraint only; no iOS application work is required for Android v0.1.

Decoder-specific pixel behavior may differ between platforms. Therefore blur, rotation, scale, glare, and damage fixtures must be run against each shipped platform decoder while the expected protocol result remains defined by Rust.

## Compatibility/fixture implications

Two fixture layers are required:

1. protocol fixtures proving canonical Rust parsing and typed rejection; and
2. rendered/photographed image fixtures proving the shipped platform decoder can recover the expected text under supported conditions.

Milestone 7.4's blur/rotation/scale/damage fixture item remains open. A decoder change must rerun the platform image corpus. A Rust grammar change must follow the versioning rules in ADR 0001.

## Consequences and tradeoffs

Positive consequences:

- no additional native QR imaging dependency;
- Android remains responsible for platform image acquisition and decoding;
- Rust remains the single semantic trust boundary;
- arbitrary QR content cannot bypass A2D validation;
- future platforms can use native decoders without duplicating protocol logic; and
- current Milestone 6 code becomes the documented production boundary rather than an implicit implementation choice.

Costs and risks:

- rendered-image behavior can vary by platform decoder;
- each shipped platform requires its own image-fixture validation;
- ZXing remains an Android application dependency; and
- final CameraX integration must preserve bounded background execution and immediate Rust validation.

## Validation evidence

- [x] Android local decoder restricts recognition to QR codes.
- [x] Android validates image bounds and downsamples oversized decode dimensions.
- [x] Decode work is dispatched off the main thread by the current capture adapter.
- [x] Android setup-code workflows pass decoded text to Rust rather than accepting it directly.
- [x] Canonical Rust parser enforces grammar, bounds, layout membership, and CRC integrity.
- [x] Rust parser returns typed failures and has no URL/arbitrary-content fallback.
- [x] Android instrumentation proves all three current Page Code variants round-trip through a real QR image decoder and the Rust-generated wire format.
- [ ] Blur, rotation, scale, and damage fixtures committed and run against the shipped decoder.
- [ ] Milestone 8 CameraX analysis path proven to preserve this boundary.

## Follow-up tasks

1. Add legally usable rendered and photographed QR fixtures covering rotation, scale, blur, glare, partial damage, malformed payloads, and non-A2D QR content.
2. Record expected decoder and Rust-parser outcomes separately in fixture metadata.
3. Reuse this boundary in Milestone 8 CameraX scanning without moving semantic acceptance into Kotlin.
4. Keep decoded-text handling bounded and off the main thread.
5. Rerun the platform fixture corpus before changing the Android QR decoder library or major version.

## Superseding ADR reference

None.
