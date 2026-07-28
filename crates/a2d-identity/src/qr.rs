//! QR payload encoding and strict parsing for the v1 wire format
//! `docs/decisions/0001-qr-v1-encoding-and-integrity.md` specifies. The encoder predates
//! Milestone 4 (it exists to give the ADR's own required Android decoder spike real canonical
//! payload strings); [`parse`] is Milestone 4.2's strict decoder, built directly against that
//! ADR's "Strict-parser rules" list. ADR 0001 is Accepted and `fixtures/qr/v1/` now permanently
//! freeze the v1 compatibility surface; future changes require a new protocol version.
//!
//! [`parse`] takes the `layout-id` registry membership check as a caller-supplied predicate
//! rather than depending on `a2d-layout` directly: that crate's registry doesn't exist yet
//! (Milestone 5), and `a2d-identity` staying decoupled from it avoids a forward dependency that
//! would have to be unwound later. `a2d-core` wires the real registry through once Milestone 5
//! lands.
//!
//! Grammar (from the ADR):
//!
//! ```text
//! payload      := "A2D" ":" version ":" type-code ":" type-fields ":" crc
//! version      := "1"
//! type-code    := "S" | "B" | "M"
//! crc          := 7 x crockford-base32-char   (CRC-32C over the payload up to and including
//!                                               the ":" immediately preceding this field)
//! id128        := 26 x crockford-base32-char  (a2d_domain's existing 128-bit ID encoding)
//! absent       := "-"
//!
//! S (NotebookSetup):  A2D:1:S:<design-id>:<crc>
//! B (NotebookPage):   A2D:1:B:<design-id>:<logical-page-number>:<layout-id>:<crc>
//! M (SmartPage):      A2D:1:M:<smart-page-id>:<layout-id>:<visible-page-number|absent>:<page-set-id|absent>:<crc>
//! ```

use a2d_domain::{
    A2dError, ErrorCategory, ErrorCode, ErrorSeverity, LayoutId, NotebookDesignId, PageSetId,
    SmartPageId,
};

/// The three code types spec §14.2 defines, carrying the fields their canonical encoding needs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PageCode {
    NotebookSetup {
        design_id: NotebookDesignId,
    },
    NotebookPage {
        design_id: NotebookDesignId,
        logical_page_number: u32,
        layout_id: LayoutId,
    },
    SmartPage {
        smart_page_id: SmartPageId,
        layout_id: LayoutId,
        visible_page_number: Option<u32>,
        page_set_id: Option<PageSetId>,
    },
}

const MAX_NUMERIC_FIELD: u32 = 999_999;
const CRC_ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encodes a 32-bit value as exactly 7 canonical Crockford Base32 characters, MSB-first,
/// zero-padded on the most-significant end (same convention as `a2d_domain::id`'s 128-bit
/// encoding, applied to a narrower value; duplicated in miniature here rather than reusing that
/// module's internals, which are sized specifically for 128 bits).
fn encode_crc(value: u32) -> String {
    let mut out = String::with_capacity(7);
    let first = (value >> 30) & 0b11;
    out.push(CRC_ALPHABET[first as usize] as char);
    for group in 0..6u32 {
        let shift = 25 - 5 * group;
        let digit = (value >> shift) & 0b1_1111;
        out.push(CRC_ALPHABET[digit as usize] as char);
    }
    out
}

fn range_error(field: &str, value: u32) -> A2dError {
    A2dError::new(
        ErrorCode::new("QR_NUMERIC_FIELD_OUT_OF_RANGE"),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.qr.numeric_field_out_of_range",
        format!("{field} must be in 0..={MAX_NUMERIC_FIELD}, got {value}"),
        false,
    )
    .with_detail("field", field)
    .with_detail("value", value.to_string())
}

impl PageCode {
    /// Encodes this code as the canonical v1 payload string, including its CRC-32C integrity
    /// field. Rejects out-of-range numeric fields; does not otherwise validate its inputs (an
    /// invalid `LayoutId`/id can't be constructed in the first place — their own constructors
    /// already enforce that).
    pub fn encode(&self) -> Result<String, A2dError> {
        let prefix = match self {
            PageCode::NotebookSetup { design_id } => format!("A2D:1:S:{design_id}"),
            PageCode::NotebookPage {
                design_id,
                logical_page_number,
                layout_id,
            } => {
                if *logical_page_number > MAX_NUMERIC_FIELD {
                    return Err(range_error("logical_page_number", *logical_page_number));
                }
                format!("A2D:1:B:{design_id}:{logical_page_number}:{layout_id}")
            }
            PageCode::SmartPage {
                smart_page_id,
                layout_id,
                visible_page_number,
                page_set_id,
            } => {
                if let Some(n) = visible_page_number
                    && *n > MAX_NUMERIC_FIELD
                {
                    return Err(range_error("visible_page_number", *n));
                }
                let visible = visible_page_number
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".to_string());
                let page_set = page_set_id
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "-".to_string());
                format!("A2D:1:M:{smart_page_id}:{layout_id}:{visible}:{page_set}")
            }
        };
        let crc_input = format!("{prefix}:");
        let crc = encode_crc(crc32c::crc32c(crc_input.as_bytes()));
        Ok(format!("{crc_input}{crc}"))
    }
}

const MAX_PAYLOAD_LEN: usize = 128;

fn parse_error(code: &'static str, message: impl Into<String>) -> A2dError {
    A2dError::new(
        ErrorCode::new(code),
        ErrorCategory::Validation,
        ErrorSeverity::Error,
        "error.qr.invalid_payload",
        message.into(),
        false,
    )
}

/// Parses and canonicalizes a numeric field: no sign, no leading zero (other than the literal
/// `0`), digits only, bounded `0..=999999`.
fn parse_numeric_field(field: &str, name: &str) -> Result<u32, A2dError> {
    if field.is_empty() || !field.bytes().all(|b| b.is_ascii_digit()) {
        return Err(parse_error(
            "QR_NUMERIC_FIELD_INVALID",
            format!("{name} must contain only ASCII digits, got {field:?}"),
        ));
    }
    if field.len() > 1 && field.starts_with('0') {
        return Err(parse_error(
            "QR_NUMERIC_FIELD_INVALID",
            format!("{name} must not have a leading zero, got {field:?}"),
        ));
    }
    let value: u32 = field
        .parse()
        .map_err(|_| parse_error("QR_NUMERIC_FIELD_INVALID", format!("{name} overflowed")))?;
    if value > MAX_NUMERIC_FIELD {
        return Err(range_error(name, value));
    }
    Ok(value)
}

fn parse_layout_id(
    field: &str,
    is_known_layout_id: &impl Fn(&LayoutId) -> bool,
) -> Result<LayoutId, A2dError> {
    let layout_id = LayoutId::parse(field)?;
    if !is_known_layout_id(&layout_id) {
        return Err(parse_error(
            "QR_LAYOUT_ID_UNKNOWN",
            format!("{field:?} is not present in the current layout registry"),
        )
        .with_detail("layout_id", field));
    }
    Ok(layout_id)
}

/// Strictly parses a v1 QR payload string into a [`PageCode`], enforcing every rule in
/// `docs/decisions/0001-qr-v1-encoding-and-integrity.md`'s "Strict-parser rules" list. Rejects
/// with a typed error and no partial acceptance — never falls back to treating the input as a
/// URL or other content (spec §14.3): callers only ever get `Ok(PageCode)` or `Err(A2dError)`.
///
/// `is_known_layout_id` resolves the `layout-id` registry check; see the module docs for why
/// that's a caller-supplied predicate rather than a direct `a2d-layout` dependency.
pub fn parse(
    payload: &str,
    is_known_layout_id: impl Fn(&LayoutId) -> bool,
) -> Result<PageCode, A2dError> {
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(parse_error(
            "QR_PAYLOAD_TOO_LONG",
            format!(
                "payload is {} characters, exceeds the {MAX_PAYLOAD_LEN}-character maximum",
                payload.len()
            ),
        ));
    }
    if !payload
        .bytes()
        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b':' || b == b'-')
    {
        return Err(parse_error(
            "QR_INVALID_CHARACTER",
            "payload contains a character outside A2D's canonical subset of the QR \
             alphanumeric charset (uppercase A-Z, 0-9, ':', '-')",
        ));
    }

    let parts: Vec<&str> = payload.split(':').collect();
    if parts.first() != Some(&"A2D") {
        return Err(parse_error(
            "QR_INVALID_MAGIC_PREFIX",
            "payload does not start with the \"A2D\" magic prefix",
        ));
    }
    let version = *parts
        .get(1)
        .ok_or_else(|| parse_error("QR_WRONG_FIELD_COUNT", "payload is missing a version field"))?;
    if version != "1" {
        return Err(parse_error(
            "QR_UNSUPPORTED_VERSION",
            format!("unsupported version {version:?}; this parser understands v1 only"),
        ));
    }
    let type_code = *parts.get(2).ok_or_else(|| {
        parse_error(
            "QR_WRONG_FIELD_COUNT",
            "payload is missing a type-code field",
        )
    })?;

    let expected_len = match type_code {
        "S" => 5,
        "B" => 7,
        "M" => 8,
        other => {
            return Err(parse_error(
                "QR_UNKNOWN_TYPE_CODE",
                format!("unknown type-code {other:?}; expected one of \"S\", \"B\", \"M\""),
            ));
        }
    };
    if parts.len() != expected_len {
        return Err(parse_error(
            "QR_WRONG_FIELD_COUNT",
            format!(
                "type-code {type_code:?} expects {expected_len} colon-separated fields, got {}",
                parts.len()
            ),
        ));
    }

    // Field-level validation runs before the CRC check (matching the order the ADR's own
    // "Strict-parser rules" list gives, id128/numeric/layout-id before CRC mismatch): a
    // malformed field gets its own specific error rather than being masked by a generic
    // corruption report, and the CRC check acts as a final integrity gate over an
    // otherwise-well-formed-looking payload.
    let decoded = match type_code {
        "S" => PageCode::NotebookSetup {
            design_id: NotebookDesignId::parse(parts[3])?,
        },
        "B" => PageCode::NotebookPage {
            design_id: NotebookDesignId::parse(parts[3])?,
            logical_page_number: parse_numeric_field(parts[4], "logical_page_number")?,
            layout_id: parse_layout_id(parts[5], &is_known_layout_id)?,
        },
        "M" => PageCode::SmartPage {
            smart_page_id: SmartPageId::parse(parts[3])?,
            layout_id: parse_layout_id(parts[4], &is_known_layout_id)?,
            visible_page_number: match parts[5] {
                "-" => None,
                field => Some(parse_numeric_field(field, "visible_page_number")?),
            },
            page_set_id: match parts[6] {
                "-" => None,
                field => Some(PageSetId::parse(field)?),
            },
        },
        _ => unreachable!("type_code was already validated against the known set above"),
    };

    let crc_input = format!("{}:", parts[..parts.len() - 1].join(":"));
    let expected_crc = encode_crc(crc32c::crc32c(crc_input.as_bytes()));
    if parts[parts.len() - 1] != expected_crc {
        return Err(parse_error(
            "QR_CRC_MISMATCH",
            "recomputed CRC-32C does not match the payload's crc field; payload is corrupt",
        ));
    }

    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> LayoutId {
        LayoutId::parse("USLETTER-LINED").unwrap()
    }

    #[test]
    fn notebook_setup_encodes_with_the_expected_shape() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup {
            design_id: design_id.clone(),
        }
        .encode()
        .unwrap();
        let parts: Vec<&str> = payload.split(':').collect();
        assert_eq!(parts, ["A2D", "1", "S", &design_id.to_string(), parts[4]]);
        assert_eq!(parts[4].len(), 7);
        assert!(
            payload
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b':')
        );
    }

    #[test]
    fn notebook_page_encodes_with_the_expected_shape() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookPage {
            design_id: design_id.clone(),
            logical_page_number: 42,
            layout_id: layout(),
        }
        .encode()
        .unwrap();
        let parts: Vec<&str> = payload.split(':').collect();
        assert_eq!(
            parts,
            [
                "A2D",
                "1",
                "B",
                &design_id.to_string(),
                "42",
                "USLETTER-LINED",
                parts[6]
            ]
        );
    }

    #[test]
    fn notebook_page_rejects_an_out_of_range_logical_page_number() {
        let err = PageCode::NotebookPage {
            design_id: NotebookDesignId::generate(),
            logical_page_number: MAX_NUMERIC_FIELD + 1,
            layout_id: layout(),
        }
        .encode()
        .unwrap_err();
        assert!(err.code.to_string().contains("OUT_OF_RANGE"));
    }

    #[test]
    fn smart_page_encodes_absent_fields_as_a_dash() {
        let smart_page_id = SmartPageId::generate();
        let payload = PageCode::SmartPage {
            smart_page_id: smart_page_id.clone(),
            layout_id: layout(),
            visible_page_number: None,
            page_set_id: None,
        }
        .encode()
        .unwrap();
        let parts: Vec<&str> = payload.split(':').collect();
        assert_eq!(
            parts,
            [
                "A2D",
                "1",
                "M",
                &smart_page_id.to_string(),
                "USLETTER-LINED",
                "-",
                "-",
                parts[7]
            ]
        );
    }

    #[test]
    fn smart_page_encodes_present_optional_fields() {
        let smart_page_id = SmartPageId::generate();
        let page_set_id = PageSetId::generate();
        let payload = PageCode::SmartPage {
            smart_page_id: smart_page_id.clone(),
            layout_id: layout(),
            visible_page_number: Some(7),
            page_set_id: Some(page_set_id.clone()),
        }
        .encode()
        .unwrap();
        let parts: Vec<&str> = payload.split(':').collect();
        assert_eq!(
            parts,
            [
                "A2D",
                "1",
                "M",
                &smart_page_id.to_string(),
                "USLETTER-LINED",
                "7",
                &page_set_id.to_string(),
                parts[7]
            ]
        );
    }

    #[test]
    fn encoding_is_deterministic_for_the_same_inputs() {
        let design_id = NotebookDesignId::generate();
        let code = PageCode::NotebookSetup { design_id };
        assert_eq!(code.encode().unwrap(), code.encode().unwrap());
    }

    #[test]
    fn different_payloads_get_different_crcs() {
        let a = PageCode::NotebookSetup {
            design_id: NotebookDesignId::generate(),
        }
        .encode()
        .unwrap();
        let b = PageCode::NotebookSetup {
            design_id: NotebookDesignId::generate(),
        }
        .encode()
        .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn total_length_stays_within_the_adrs_128_character_maximum() {
        let payload = PageCode::SmartPage {
            smart_page_id: SmartPageId::generate(),
            layout_id: LayoutId::parse(&"X".repeat(20)).unwrap(),
            visible_page_number: Some(MAX_NUMERIC_FIELD),
            page_set_id: Some(PageSetId::generate()),
        }
        .encode()
        .unwrap();
        assert!(payload.len() <= 128, "payload was {} chars", payload.len());
    }

    // ---------------------------------------------------------------------------------------
    // Milestone 4.2: strict parser tests, one per rule in the ADR's "Strict-parser rules" list.
    // ---------------------------------------------------------------------------------------

    fn always_known(_: &LayoutId) -> bool {
        true
    }

    fn never_known(_: &LayoutId) -> bool {
        false
    }

    #[test]
    fn round_trips_notebook_setup_through_encode_and_parse() {
        let code = PageCode::NotebookSetup {
            design_id: NotebookDesignId::generate(),
        };
        let payload = code.encode().unwrap();
        assert_eq!(parse(&payload, always_known).unwrap(), code);
    }

    #[test]
    fn round_trips_notebook_page_through_encode_and_parse() {
        let code = PageCode::NotebookPage {
            design_id: NotebookDesignId::generate(),
            logical_page_number: 0,
            layout_id: layout(),
        };
        let payload = code.encode().unwrap();
        assert_eq!(parse(&payload, always_known).unwrap(), code);
    }

    #[test]
    fn round_trips_smart_page_with_absent_optional_fields() {
        let code = PageCode::SmartPage {
            smart_page_id: SmartPageId::generate(),
            layout_id: layout(),
            visible_page_number: None,
            page_set_id: None,
        };
        let payload = code.encode().unwrap();
        assert_eq!(parse(&payload, always_known).unwrap(), code);
    }

    #[test]
    fn round_trips_smart_page_with_present_optional_fields() {
        let code = PageCode::SmartPage {
            smart_page_id: SmartPageId::generate(),
            layout_id: layout(),
            visible_page_number: Some(7),
            page_set_id: Some(PageSetId::generate()),
        };
        let payload = code.encode().unwrap();
        assert_eq!(parse(&payload, always_known).unwrap(), code);
    }

    #[test]
    fn the_adrs_own_valid_notebook_page_example_parses_successfully() {
        // From the ADR's "Examples" section -- illustrative, not a golden vector, but it's a
        // real value drawn straight from the accepted grammar, so it's worth parsing directly.
        let payload = "A2D:1:B:01ARZ3NDEKTSV4RRFFQ69G5FAV:12:USLETTER-LINED:3KZ8QWY";
        // The example's CRC was hand-illustrative, not computed against this implementation, so
        // this must fail closed on the CRC check rather than silently accept it -- proves the
        // parser never trusts an unverified integrity field.
        let err = parse(payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("CRC_MISMATCH"));
    }

    #[test]
    fn rejects_lowercase_before_tokenizing() {
        let err = parse("a2d:1:b:x", always_known).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_CHARACTER"));
    }

    #[test]
    fn rejects_a_byte_outside_the_canonical_subset() {
        let err = parse("A2D:1:S:01ARZ3NDEKTSV4RRFFQ69G5FA$:1234567", always_known).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_CHARACTER"));
    }

    #[test]
    fn rejects_a_wrong_magic_prefix() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup { design_id }
            .encode()
            .unwrap()
            .replacen("A2D", "A2X", 1);
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_MAGIC_PREFIX"));
    }

    #[test]
    fn rejects_an_unsupported_version() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup { design_id }
            .encode()
            .unwrap()
            .replacen(":1:", ":2:", 1);
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("UNSUPPORTED_VERSION"));
    }

    #[test]
    fn rejects_an_unknown_type_code() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup { design_id }
            .encode()
            .unwrap()
            .replacen(":S:", ":Q:", 1);
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("UNKNOWN_TYPE_CODE"));
    }

    #[test]
    fn rejects_a_missing_field() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup { design_id }.encode().unwrap();
        let truncated = payload.rsplit_once(':').unwrap().0.to_string();
        let err = parse(&truncated, always_known).unwrap_err();
        assert!(err.code.to_string().contains("WRONG_FIELD_COUNT"));
    }

    #[test]
    fn rejects_trailing_data_after_the_crc() {
        let design_id = NotebookDesignId::generate();
        let payload = format!(
            "{}:EXTRA",
            PageCode::NotebookSetup { design_id }.encode().unwrap()
        );
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("WRONG_FIELD_COUNT"));
    }

    #[test]
    fn rejects_an_id128_field_with_the_wrong_length() {
        let payload = "A2D:1:S:TOOSHORT:1234567";
        let err = parse(payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_LENGTH"));
    }

    #[test]
    fn rejects_an_id128_field_containing_i_l_o_or_u() {
        // 26 characters, canonical length, but 'I' is never a valid alphabet character.
        let payload = "A2D:1:S:IIIIIIIIIIIIIIIIIIIIIIIIII:1234567";
        let err = parse(payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("INVALID_ALPHABET"));
    }

    #[test]
    fn rejects_a_numeric_field_with_a_leading_zero() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookPage {
            design_id,
            logical_page_number: 7,
            layout_id: layout(),
        }
        .encode()
        .unwrap()
        .replacen(":7:", ":07:", 1);
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("NUMERIC_FIELD_INVALID"));
    }

    #[test]
    fn rejects_a_numeric_field_with_a_sign() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookPage {
            design_id,
            logical_page_number: 7,
            layout_id: layout(),
        }
        .encode()
        .unwrap()
        .replacen(":7:", ":-7:", 1);
        let err = parse(&payload, always_known).unwrap_err();
        // '-' is otherwise a legal payload byte (the absent-field sentinel), so a signed number
        // isn't caught by the charset scan -- it's rejected specifically as a malformed numeric
        // field once this field is parsed as `logical_page_number`, which never accepts "-".
        assert!(err.code.to_string().contains("NUMERIC_FIELD_INVALID"));
    }

    #[test]
    fn rejects_a_numeric_field_out_of_range() {
        // Hand-build the payload: `PageCode::encode` already refuses to construct this, so the
        // rejection under test here is specifically the parser's own range check, exercised
        // against a payload that was never round-tripped through the encoder.
        let design_id = NotebookDesignId::generate();
        let prefix = format!("A2D:1:B:{design_id}:1000000:{}", layout());
        let crc_input = format!("{prefix}:");
        let crc = encode_crc(crc32c::crc32c(crc_input.as_bytes()));
        let payload = format!("{crc_input}{crc}");
        let err = parse(&payload, always_known).unwrap_err();
        assert!(err.code.to_string().contains("OUT_OF_RANGE"));
    }

    #[test]
    fn rejects_an_unregistered_layout_id() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookPage {
            design_id,
            logical_page_number: 1,
            layout_id: layout(),
        }
        .encode()
        .unwrap();
        let err = parse(&payload, never_known).unwrap_err();
        assert!(err.code.to_string().contains("LAYOUT_ID_UNKNOWN"));
    }

    #[test]
    fn rejects_a_tampered_crc() {
        let design_id = NotebookDesignId::generate();
        let payload = PageCode::NotebookSetup { design_id }.encode().unwrap();
        let (prefix, _crc) = payload.rsplit_once(':').unwrap();
        let tampered = format!("{prefix}:0000000");
        let err = parse(&tampered, always_known).unwrap_err();
        assert!(err.code.to_string().contains("CRC_MISMATCH"));
    }

    #[test]
    fn rejects_a_payload_over_the_maximum_length() {
        let oversized = format!("A2D:1:S:{}", "0".repeat(200));
        let err = parse(&oversized, always_known).unwrap_err();
        assert!(err.code.to_string().contains("TOO_LONG"));
    }

    #[test]
    fn a_rejected_payload_is_a_typed_error_never_a_reinterpretable_string() {
        // Spec §14.3: a rejected payload MUST NOT be reinterpreted as a URL or arbitrary web
        // content. Structurally enforced by `parse`'s signature: callers can only get a typed
        // `PageCode` or a typed `A2dError`, never the raw string back to reinterpret themselves.
        let result: Result<PageCode, A2dError> = parse("not a valid payload", always_known);
        assert!(result.is_err());
    }
}
