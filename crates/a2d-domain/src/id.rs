//! Opaque, strongly typed 128-bit identifiers shared by every crate (TODO 2.1, spec §13).
//!
//! Every independently persisted, referenceable, or FFI-crossing domain entity gets its own
//! newtype here rather than a raw `String`. All of them share one canonical wire form: 26
//! uppercase Crockford Base32 characters, MSB-first, zero-padded on the most-significant end.

use std::fmt;

use crate::error::{A2dError, ErrorCategory, ErrorCode, ErrorSeverity};

const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Used only when the OS randomness source fails while an error envelope is already being built.
/// It is intentionally not 26 characters and never claims uniqueness or entity-ID validity.
pub(crate) const EMERGENCY_CORRELATION_ID: &str = "correlation-rng-unavailable";

fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'H' => Some(byte - b'A' + 10),
        b'J' | b'K' => Some(byte - b'A' + 9),
        b'M' | b'N' => Some(byte - b'A' + 8),
        b'P'..=b'T' => Some(byte - b'A' + 7),
        b'V'..=b'Z' => Some(byte - b'A' + 6),
        _ => None,
    }
}

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

fn decode_128(value: &str) -> Result<[u8; 16], IdDecodeError> {
    if value.len() != 26 {
        return Err(IdDecodeError::InvalidLength {
            actual: value.len(),
        });
    }
    let mut digits = [0u8; 26];
    for (index, &byte) in value.as_bytes().iter().enumerate() {
        digits[index] = decode_digit(byte).ok_or(IdDecodeError::InvalidAlphabet)?;
    }
    if digits[0] > 0b111 {
        return Err(IdDecodeError::NonCanonical);
    }
    let mut decoded: u128 = digits[0] as u128;
    for &digit in &digits[1..] {
        decoded = (decoded << 5) | digit as u128;
    }
    Ok(decoded.to_be_bytes())
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
                "{type_name} contains a character outside the canonical Crockford Base32 alphabet (uppercase 0-9, A-Z excluding I/L/O/U)"
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

fn random_128() -> Result<[u8; 16], getrandom::Error> {
    let mut buffer = [0u8; 16];
    getrandom::getrandom(&mut buffer)?;
    Ok(buffer)
}

fn correlation_id_from_random_bytes(bytes: Option<[u8; 16]>) -> String {
    bytes
        .map(encode_128)
        .unwrap_or_else(|| EMERGENCY_CORRELATION_ID.to_string())
}

pub(crate) fn generate_correlation_id() -> String {
    correlation_id_from_random_bytes(random_128().ok())
}

fn id_generation_error(
    type_name: &'static str,
    code_prefix: &'static str,
    source: getrandom::Error,
) -> A2dError {
    A2dError::new(
        ErrorCode::new(format!("{code_prefix}_RANDOMNESS_UNAVAILABLE")),
        ErrorCategory::PlatformAdapter,
        ErrorSeverity::Critical,
        "error.id.randomness_unavailable",
        format!(
            "OS cryptographic randomness is unavailable while generating {type_name}: {source}"
        ),
        false,
    )
    .with_detail("id_type", type_name)
}

macro_rules! define_id {
    ($name:ident, $code_prefix:literal) => {
        #[doc = concat!("Opaque identifier for a `", stringify!($name), "`. See `id` module docs for the wire format.")]
        #[derive(Clone, Eq, PartialEq, Hash)]
        pub struct $name(String);

        impl $name {
            /// Fallible production ID generation. Callers must propagate failure before performing
            /// any canonical database or filesystem mutation.
            pub fn try_generate() -> Result<Self, A2dError> {
                random_128()
                    .map(|bytes| Self(encode_128(bytes)))
                    .map_err(|source| {
                        id_generation_error(stringify!($name), $code_prefix, source)
                    })
            }

            /// Compatibility constructor retained while production call sites migrate to
            /// [`Self::try_generate`]. New production code must not use this method.
            pub fn generate() -> Self {
                Self::try_generate().unwrap_or_else(|error| {
                    panic!("cryptographic ID generation failed: {error}")
                })
            }

            pub fn parse(value: &str) -> Result<Self, A2dError> {
                match decode_128(value) {
                    Ok(_) => Ok(Self(value.to_string())),
                    Err(cause) => Err(id_decode_error(
                        stringify!($name),
                        $code_prefix,
                        value,
                        cause,
                    )),
                }
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[cfg(feature = "test-util")]
            pub fn from_raw_for_test(bytes: [u8; 16]) -> Self {
                Self(encode_128(bytes))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!(stringify!($name), "({})"), self.0)
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
    use std::collections::HashSet;

    use super::*;

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
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
            );
            assert_eq!(decode_128(&encoded).unwrap(), bytes);
        }
    }

    #[test]
    fn known_encoding_vector() {
        assert_eq!(encode_128([0u8; 16]), "0".repeat(26));
        assert_eq!(encode_128([0xFFu8; 16]), format!("7{}", "Z".repeat(25)));
    }

    #[test]
    fn rejects_wrong_length_alphabet_case_and_padding() {
        assert!(
            PageId::parse("SHORT")
                .unwrap_err()
                .code
                .to_string()
                .contains("PAGE_ID_INVALID_LENGTH")
        );
        let ambiguous = format!("I{}", "0".repeat(25));
        assert!(
            PageId::parse(&ambiguous)
                .unwrap_err()
                .code
                .to_string()
                .contains("PAGE_ID_INVALID_ALPHABET")
        );
        let lowercase = encode_128([1u8; 16]).to_lowercase();
        assert!(
            PageId::parse(&lowercase)
                .unwrap_err()
                .code
                .to_string()
                .contains("PAGE_ID_INVALID_ALPHABET")
        );
        let noncanonical = format!("8{}", "0".repeat(25));
        assert!(
            PageId::parse(&noncanonical)
                .unwrap_err()
                .code
                .to_string()
                .contains("PAGE_ID_NON_CANONICAL")
        );
    }

    #[test]
    fn generate_then_parse_round_trips() {
        let id = ScanId::generate();
        assert_eq!(ScanId::parse(&id.to_string()).unwrap(), id);
    }

    #[test]
    fn fallible_generation_round_trips_and_large_sample_is_unique() {
        let mut seen = HashSet::new();
        for _ in 0..10_000 {
            let id = AssetId::try_generate().unwrap();
            assert!(seen.insert(id.to_string()), "duplicate ID generated");
            assert_eq!(AssetId::parse(id.as_str()).unwrap(), id);
        }
    }

    #[test]
    fn emergency_correlation_marker_is_stable_and_noncanonical() {
        let first = correlation_id_from_random_bytes(None);
        let second = correlation_id_from_random_bytes(None);
        assert_eq!(first, EMERGENCY_CORRELATION_ID);
        assert_eq!(first, second);
        assert_ne!(first.len(), 26);
        assert!(PageId::parse(&first).is_err());
    }

    #[test]
    fn distinct_types_are_not_interchangeable_at_compile_time() {
        let page = PageId::generate();
        let scan = ScanId::generate();
        assert_ne!(page.as_str(), scan.as_str());
    }
}
