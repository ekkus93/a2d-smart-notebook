//! Opaque, strongly typed 128-bit identifiers shared by every crate (TODO 2.1, spec §13).
//!
//! Every independently persisted, referenceable, or FFI-crossing domain entity gets its own
//! newtype here rather than a raw `String`. All of them share one canonical wire form: 26
//! uppercase Crockford Base32 characters, MSB-first, zero-padded on the most-significant end —
//! the same scheme `docs/decisions/0001-qr-v1-encoding-and-integrity.md` specifies for the
//! subset of IDs that additionally appear in QR payloads. Using one format everywhere (rather
//! than a QR-specific format plus a separate general-purpose one) was an open decision; see
//! `memory.md` for the reasoning.

use std::fmt;

use crate::error::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'H' => Some(byte - b'A' + 10),
        // 'I' skipped: not in the canonical alphabet.
        b'J' | b'K' => Some(byte - b'A' + 9),
        // 'L' skipped.
        b'M' | b'N' => Some(byte - b'A' + 8),
        // 'O' skipped.
        b'P'..=b'T' => Some(byte - b'A' + 7),
        // 'U' skipped.
        b'V'..=b'Z' => Some(byte - b'A' + 6),
        _ => None,
    }
}

/// Encodes 16 bytes (128 bits) as 26 canonical, uppercase Crockford Base32 characters.
fn encode_128(bytes: [u8; 16]) -> String {
    let value = u128::from_be_bytes(bytes);
    let mut out = String::with_capacity(26);
    let first = ((value >> 125) & 0b111) as usize;
    out.push(ALPHABET[first] as char);
    for group in 0..25u32 {
        let shift = 120 - 5 * group;
        let digit = ((value >> shift) & 0b1_1111) as usize;
        out.push(ALPHABET[digit] as char);
    }
    out
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdDecodeError {
    InvalidLength { actual: usize },
    InvalidAlphabet,
    NonCanonical,
}

/// Strictly decodes a canonical 26-character Crockford Base32 string back to 16 bytes. No alias
/// normalization: `I`, `L`, `O`, `U`, and any lowercase input are rejected outright, never mapped
/// to a look-alike digit.
fn decode_128(s: &str) -> Result<[u8; 16], IdDecodeError> {
    if s.len() != 26 {
        return Err(IdDecodeError::InvalidLength { actual: s.len() });
    }
    let mut digits = [0u8; 26];
    for (i, &b) in s.as_bytes().iter().enumerate() {
        digits[i] = decode_digit(b).ok_or(IdDecodeError::InvalidAlphabet)?;
    }
    if digits[0] > 0b111 {
        return Err(IdDecodeError::NonCanonical);
    }
    let mut value: u128 = digits[0] as u128;
    for &d in &digits[1..] {
        value = (value << 5) | d as u128;
    }
    Ok(value.to_be_bytes())
}

fn id_decode_error(
    type_name: &str,
    code_prefix: &str,
    input: &str,
    cause: IdDecodeError,
) -> A2dError {
    let (suffix, message) = match cause {
        IdDecodeError::InvalidLength { actual } => (
            "INVALID_LENGTH",
            format!("{type_name} must be exactly 26 characters, got {actual}"),
        ),
        IdDecodeError::InvalidAlphabet => (
            "INVALID_ALPHABET",
            format!(
                "{type_name} contains a character outside the canonical Crockford Base32 \
                 alphabet (uppercase 0-9, A-Z excluding I/L/O/U)"
            ),
        ),
        IdDecodeError::NonCanonical => (
            "NON_CANONICAL",
            format!(
                "{type_name} is not a canonical encoding: the first character must represent a value 0-7"
            ),
        ),
    };
    A2dError::new(
        ErrorCode::new(format!("{code_prefix}_{suffix}")),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.id.invalid",
        message,
        false,
    )
    .with_detail("input", input)
}

fn random_128() -> [u8; 16] {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("OS cryptographic randomness source must be available");
    buf
}

pub(crate) fn generate_correlation_id() -> String {
    encode_128(random_128())
}

macro_rules! define_id {
    ($name:ident, $code_prefix:literal) => {
        #[doc = concat!("Opaque identifier for a `", stringify!($name), "`. See `id` module docs for the wire format.")]
        #[derive(Clone, Eq, PartialEq, Hash)]
        pub struct $name(String);

        impl $name {
            /// Generates a new, random, canonically encoded identifier using OS cryptographic
            /// randomness. Production code MUST use this rather than any deterministic source.
            pub fn generate() -> Self {
                Self(encode_128(random_128()))
            }

            /// Parses a canonical identifier. Rejects wrong length, invalid alphabet (including
            /// lowercase and I/L/O/U), and non-canonical padding.
            pub fn parse(s: &str) -> Result<Self, A2dError> {
                match decode_128(s) {
                    Ok(_) => Ok(Self(s.to_string())),
                    Err(cause) => Err(id_decode_error(stringify!($name), $code_prefix, s, cause)),
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Builds an identifier from caller-supplied bytes rather than OS randomness. Only
            /// available to tests (TODO 2.1: "a deterministic RNG is available only through
            /// test interfaces"), including other crates' tests via the `test-util` feature.
            #[cfg(feature = "test-util")]
            pub fn from_raw_for_test(bytes: [u8; 16]) -> Self {
                Self(encode_128(bytes))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

define_id!(InstallationId, "INSTALLATION_ID");
define_id!(NotebookDesignId, "NOTEBOOK_DESIGN_ID");
define_id!(NotebookId, "NOTEBOOK_ID");
define_id!(PageId, "PAGE_ID");
define_id!(PageSetId, "PAGE_SET_ID");
define_id!(SmartPageId, "SMART_PAGE_ID");
define_id!(PhysicalCopyId, "PHYSICAL_COPY_ID");
define_id!(ScanId, "SCAN_ID");
define_id!(AssetId, "ASSET_ID");
define_id!(OcrRunId, "OCR_RUN_ID");
define_id!(TextRegionId, "TEXT_REGION_ID");
define_id!(TextCorrectionId, "TEXT_CORRECTION_ID");
define_id!(CollectionId, "COLLECTION_ID");
define_id!(AnnotationId, "ANNOTATION_ID");
define_id!(ReviewItemId, "REVIEW_ITEM_ID");
define_id!(SkillId, "SKILL_ID");
define_id!(SkillRunId, "SKILL_RUN_ID");
define_id!(AuditEventId, "AUDIT_EVENT_ID");
define_id!(BackupId, "BACKUP_ID");

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn round_trips_through_encode_and_decode() {
        for seed in 0u8..50 {
            let mut bytes = [0u8; 16];
            bytes[0] = seed;
            bytes[15] = seed.wrapping_mul(7);
            let encoded = encode_128(bytes);
            assert_eq!(encoded.len(), 26);
            assert!(
                encoded
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            );
            let decoded = decode_128(&encoded).expect("round trip must decode");
            assert_eq!(decoded, bytes);
        }
    }

    #[test]
    fn known_encoding_vector() {
        // All-zero 128 bits must encode as 26 '0' characters.
        assert_eq!(encode_128([0u8; 16]), "0".repeat(26));

        // All-ones 128 bits: first char carries only 3 set bits (value 7 -> '7'), rest are 'Z' (31).
        let expected = format!("7{}", "Z".repeat(25));
        assert_eq!(encode_128([0xFFu8; 16]), expected);
    }

    #[test]
    fn rejects_wrong_length() {
        let err = PageId::parse("SHORT").unwrap_err();
        assert!(err.code.to_string().contains("PAGE_ID_INVALID_LENGTH"));
    }

    #[test]
    fn rejects_invalid_alphabet_including_ambiguous_letters() {
        // 26 chars, but contains 'I' which is deliberately excluded from the alphabet.
        let candidate = format!("I{}", "0".repeat(25));
        let err = PageId::parse(&candidate).unwrap_err();
        assert!(err.code.to_string().contains("PAGE_ID_INVALID_ALPHABET"));
    }

    #[test]
    fn rejects_lowercase() {
        let valid = PageId::generate().to_string();
        let lowered = valid.to_lowercase();
        let err = PageId::parse(&lowered).unwrap_err();
        assert!(err.code.to_string().contains("PAGE_ID_INVALID_ALPHABET"));
    }

    #[test]
    fn rejects_non_canonical_padding() {
        // First char '8' has value 8 (0b01000), which sets a padding bit that must be zero.
        let candidate = format!("8{}", "0".repeat(25));
        let err = PageId::parse(&candidate).unwrap_err();
        assert!(err.code.to_string().contains("PAGE_ID_NON_CANONICAL"));
    }

    #[test]
    fn generate_then_parse_round_trips() {
        let id = ScanId::generate();
        let parsed = ScanId::parse(&id.to_string()).expect("generated id must parse");
        assert_eq!(id, parsed);
    }

    #[test]
    fn large_sample_is_unique() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let id = AssetId::generate();
            assert!(
                seen.insert(id.to_string()),
                "collision after {} ids",
                seen.len()
            );
        }
    }

    #[test]
    fn distinct_types_are_not_interchangeable_at_compile_time() {
        // This test exists to document intent: PageId and ScanId are distinct types even
        // though both wrap a String, so passing one where the other is expected is a compile
        // error, not a runtime bug. No runtime assertion needed beyond type-checking below.
        let page = PageId::generate();
        let scan = ScanId::generate();
        assert_ne!(page.as_str(), scan.as_str());
    }
}
