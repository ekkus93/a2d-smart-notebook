//! QR payload **encoding** for the v1 wire format `docs/decisions/0001-qr-v1-encoding-and-integrity.md`
//! specifies. Encoding only, not the strict parser/decoder — that, plus golden fixtures, is
//! Milestone 4.2/4.3's job once that ADR is Accepted. This exists now, ahead of Milestone 4,
//! specifically to give the ADR's own required Android decoder spike real canonical payload
//! strings to test against, rather than hand-typed examples.
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
}
