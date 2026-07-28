//! Generates the permanent QR v1 compatibility vectors and rendered PNG fixtures.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use a2d_domain::{LayoutId, NotebookDesignId, PageSetId, SmartPageId};
use a2d_identity::qr::{PageCode, parse};
use image::{GrayImage, Luma};
use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};
use serde::Serialize;
use serde_json::{Value, json};

const DESIGN_ID: &str = "00000000000000000000000001";
const SMART_PAGE_ID: &str = "00000000000000000000000002";
const PAGE_SET_ID: &str = "00000000000000000000000003";
const MAX_LAYOUT_ID: &str = "ABCDEFGHIJKLMNOPQRST";
const QR_SCALE: u32 = 8;
const QR_QUIET_ZONE_MODULES: u32 = 4;

#[derive(Serialize)]
struct ValidVector {
    name: String,
    payload_text: String,
    decoded: Value,
}

#[derive(Serialize)]
struct MalformedVector {
    name: String,
    payload_text: String,
    expected_error: String,
}

fn known_layout(layout: &LayoutId) -> bool {
    matches!(
        layout.to_string().as_str(),
        "DEV-PAGE-V1" | "SP-A4-BLANK-V1" | "SP-LETTER-GRAPH-V1" | MAX_LAYOUT_ID
    )
}

fn valid(name: &str, code: PageCode, decoded: Value) -> Result<ValidVector, Box<dyn Error>> {
    let payload_text = code.encode()?;
    let reparsed = parse(&payload_text, known_layout)?;
    if reparsed != code {
        return Err(
            format!("generated vector {name} did not parse back to its source value").into(),
        );
    }
    Ok(ValidVector {
        name: name.to_string(),
        payload_text,
        decoded,
    })
}

fn malformed(
    name: &str,
    payload_text: String,
    expected_error: &str,
) -> Result<MalformedVector, Box<dyn Error>> {
    let error = parse(&payload_text, known_layout)
        .expect_err("malformed compatibility vector unexpectedly parsed successfully");
    if error.code.to_string() != expected_error {
        return Err(format!(
            "malformed vector {name} returned {}, expected {expected_error}",
            error.code
        )
        .into());
    }
    Ok(MalformedVector {
        name: name.to_string(),
        payload_text,
        expected_error: expected_error.to_string(),
    })
}

fn replace_crc(payload: &str) -> String {
    let (prefix, crc) = payload
        .rsplit_once(':')
        .expect("encoded payload always has crc");
    let replacement = if crc.ends_with('0') { '1' } else { '0' };
    let mut changed = crc[..crc.len() - 1].to_string();
    changed.push(replacement);
    format!("{prefix}:{changed}")
}

fn render_qr(path: &Path, payload: &str) -> Result<(), Box<dyn Error>> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)?;
    let modules = u32::try_from(code.width())?;
    let output_modules = modules + 2 * QR_QUIET_ZONE_MODULES;
    let output_size = output_modules
        .checked_mul(QR_SCALE)
        .ok_or("QR fixture size overflowed")?;
    let mut image = GrayImage::from_pixel(output_size, output_size, Luma([255]));
    let colors = code.to_colors();

    for row in 0..modules {
        for column in 0..modules {
            if colors[(row * modules + column) as usize] != QrColor::Dark {
                continue;
            }
            let top = (row + QR_QUIET_ZONE_MODULES) * QR_SCALE;
            let left = (column + QR_QUIET_ZONE_MODULES) * QR_SCALE;
            for y in top..top + QR_SCALE {
                for x in left..left + QR_SCALE {
                    image.put_pixel(x, y, Luma([0]));
                }
            }
        }
    }
    image.save(path)?;
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn Error>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: a2d-qr-v1-fixtures <output-directory>")?;
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir_all(output.join("rendered"))?;

    let design_id = NotebookDesignId::parse(DESIGN_ID)?;
    let smart_page_id = SmartPageId::parse(SMART_PAGE_ID)?;
    let page_set_id = PageSetId::parse(PAGE_SET_ID)?;

    let setup = valid(
        "notebook-setup-basic",
        PageCode::NotebookSetup {
            design_id: design_id.clone(),
        },
        json!({ "type": "NotebookSetup", "design_id": DESIGN_ID }),
    )?;
    let notebook_page = valid(
        "notebook-page-basic",
        PageCode::NotebookPage {
            design_id: design_id.clone(),
            logical_page_number: 42,
            layout_id: LayoutId::parse("DEV-PAGE-V1")?,
        },
        json!({
            "type": "NotebookPage",
            "design_id": DESIGN_ID,
            "logical_page_number": 42,
            "layout_id": "DEV-PAGE-V1"
        }),
    )?;
    let smart_standalone = valid(
        "smart-page-standalone",
        PageCode::SmartPage {
            smart_page_id: smart_page_id.clone(),
            layout_id: LayoutId::parse("SP-A4-BLANK-V1")?,
            visible_page_number: None,
            page_set_id: None,
        },
        json!({
            "type": "SmartPage",
            "smart_page_id": SMART_PAGE_ID,
            "layout_id": "SP-A4-BLANK-V1",
            "visible_page_number": null,
            "page_set_id": null
        }),
    )?;
    let smart_max = valid(
        "smart-page-max-fields",
        PageCode::SmartPage {
            smart_page_id,
            layout_id: LayoutId::parse(MAX_LAYOUT_ID)?,
            visible_page_number: Some(999_999),
            page_set_id: Some(page_set_id),
        },
        json!({
            "type": "SmartPage",
            "smart_page_id": SMART_PAGE_ID,
            "layout_id": MAX_LAYOUT_ID,
            "visible_page_number": 999999,
            "page_set_id": PAGE_SET_ID
        }),
    )?;

    let setup_vectors = vec![setup];
    let notebook_vectors = vec![notebook_page];
    let smart_vectors = vec![smart_standalone, smart_max];

    for vector in setup_vectors
        .iter()
        .chain(notebook_vectors.iter())
        .chain(smart_vectors.iter())
    {
        render_qr(
            &output.join("rendered").join(format!("{}.png", vector.name)),
            &vector.payload_text,
        )?;
    }

    let setup_payload = &setup_vectors[0].payload_text;
    let notebook_payload = &notebook_vectors[0].payload_text;
    let malformed_vectors = vec![
        malformed(
            "lowercase-character",
            setup_payload.replacen("A2D", "a2d", 1),
            "QR_INVALID_CHARACTER",
        )?,
        malformed(
            "unsupported-version",
            setup_payload.replacen("A2D:1:", "A2D:2:", 1),
            "QR_UNSUPPORTED_VERSION",
        )?,
        malformed(
            "unknown-type-code",
            setup_payload.replacen("A2D:1:S:", "A2D:1:X:", 1),
            "QR_UNKNOWN_TYPE_CODE",
        )?,
        malformed(
            "trailing-field",
            {
                let (prefix, crc) = setup_payload.rsplit_once(':').unwrap();
                format!("{prefix}:EXTRA:{crc}")
            },
            "QR_WRONG_FIELD_COUNT",
        )?,
        malformed(
            "leading-zero-number",
            notebook_payload.replacen(":42:", ":042:", 1),
            "QR_NUMERIC_FIELD_INVALID",
        )?,
        malformed(
            "unknown-layout",
            notebook_payload.replacen("DEV-PAGE-V1", "UNKNOWN-LAYOUT", 1),
            "QR_LAYOUT_ID_UNKNOWN",
        )?,
        malformed(
            "crc-mismatch",
            replace_crc(setup_payload),
            "QR_CRC_MISMATCH",
        )?,
        malformed("payload-too-long", "A".repeat(129), "QR_PAYLOAD_TOO_LONG")?,
    ];

    write_json(&output.join("notebook_setup_vectors.json"), &setup_vectors)?;
    write_json(
        &output.join("notebook_page_vectors.json"),
        &notebook_vectors,
    )?;
    write_json(&output.join("smart_page_vectors.json"), &smart_vectors)?;
    write_json(&output.join("malformed_vectors.json"), &malformed_vectors)?;
    Ok(())
}
