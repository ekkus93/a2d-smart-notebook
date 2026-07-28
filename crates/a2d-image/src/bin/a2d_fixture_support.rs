//! Fixture-only renderer used by `tools/generate_scan_fixtures.py`.
//!
//! This binary is deliberately gated behind the `fixture-tools` feature. It renders
//! tagStandard41h12 markers through the same pinned official C implementation used by
//! production detection, and it renders canonical A2D Page Code QR modules through
//! the same Rust encoder used by the application.

use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use a2d_domain::{LayoutId, NotebookDesignId};
use a2d_identity::PageCode;
use a2d_image::{AprilTagDetector, DetectorConfig};
use qrcode::types::Color as QrColor;
use qrcode::{EcLevel, QrCode};

const MAIN_DESIGN_ID: &str = "00000000000000000000000001";
const MAIN_LAYOUT_ID: &str = "FIXTURE-MAIN-V1";
const WRONG_LAYOUT_ID: &str = "FIXTURE-WRONG-V1";
const LOGICAL_PAGE_NUMBER: u32 = 42;
const QR_SCALE: usize = 8;
const QR_QUIET_ZONE_MODULES: usize = 4;

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: a2d-fixture-support <output-directory>")?;
    fs::create_dir_all(&output_dir)?;

    render_tags(&output_dir, 0..8)?;

    let design_id = NotebookDesignId::parse(MAIN_DESIGN_ID)?;
    let main_layout_id = LayoutId::parse(MAIN_LAYOUT_ID)?;
    let wrong_layout_id = LayoutId::parse(WRONG_LAYOUT_ID)?;

    let main_payload = PageCode::NotebookPage {
        design_id: design_id.clone(),
        logical_page_number: LOGICAL_PAGE_NUMBER,
        layout_id: main_layout_id,
    }
    .encode()?;
    let wrong_layout_payload = PageCode::NotebookPage {
        design_id,
        logical_page_number: LOGICAL_PAGE_NUMBER,
        layout_id: wrong_layout_id,
    }
    .encode()?;

    render_qr(&output_dir.join("qr-main.pgm"), &main_payload)?;
    render_qr(
        &output_dir.join("qr-wrong-layout.pgm"),
        &wrong_layout_payload,
    )?;

    let payloads = format!(
        concat!(
            "{{\n",
            "  \"schema_version\": 1,\n",
            "  \"design_id\": \"{}\",\n",
            "  \"layout_id\": \"{}\",\n",
            "  \"wrong_layout_id\": \"{}\",\n",
            "  \"logical_page_number\": {},\n",
            "  \"main_payload\": \"{}\",\n",
            "  \"wrong_layout_payload\": \"{}\"\n",
            "}}\n"
        ),
        MAIN_DESIGN_ID,
        MAIN_LAYOUT_ID,
        WRONG_LAYOUT_ID,
        LOGICAL_PAGE_NUMBER,
        main_payload,
        wrong_layout_payload,
    );
    fs::write(output_dir.join("payloads.json"), payloads)?;
    Ok(())
}

fn render_tags(
    output_dir: &Path,
    tag_ids: impl IntoIterator<Item = u32>,
) -> Result<(), Box<dyn Error>> {
    let detector = AprilTagDetector::new(DetectorConfig::default())?;
    for tag_id in tag_ids {
        let tag = detector.render_tag(tag_id)?;
        let path = output_dir.join(format!("tag-{tag_id}.pgm"));
        write_pgm_strided(
            &path,
            tag.width(),
            tag.height(),
            tag.row_stride(),
            tag.bytes(),
        )?;
    }
    Ok(())
}

fn render_qr(path: &Path, payload: &str) -> Result<(), Box<dyn Error>> {
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)?;
    let modules = code.width();
    let output_modules = modules + 2 * QR_QUIET_ZONE_MODULES;
    let output_size = output_modules
        .checked_mul(QR_SCALE)
        .ok_or("QR output size overflowed")?;
    let mut pixels = vec![255_u8; output_size * output_size];
    let colors = code.to_colors();

    for row in 0..modules {
        for column in 0..modules {
            if colors[row * modules + column] != QrColor::Dark {
                continue;
            }
            let output_row = (row + QR_QUIET_ZONE_MODULES) * QR_SCALE;
            let output_column = (column + QR_QUIET_ZONE_MODULES) * QR_SCALE;
            for y in output_row..output_row + QR_SCALE {
                for x in output_column..output_column + QR_SCALE {
                    pixels[y * output_size + x] = 0;
                }
            }
        }
    }

    write_pgm_strided(path, output_size, output_size, output_size, &pixels)
}

fn write_pgm_strided(
    path: &Path,
    width: usize,
    height: usize,
    stride: usize,
    bytes: &[u8],
) -> Result<(), Box<dyn Error>> {
    let required = stride
        .checked_mul(height)
        .ok_or("PGM source size overflowed")?;
    if stride < width || bytes.len() < required {
        return Err("PGM source buffer is shorter than its declared geometry".into());
    }

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    write!(writer, "P5\n{width} {height}\n255\n")?;
    for row in 0..height {
        let start = row * stride;
        writer.write_all(&bytes[start..start + width])?;
    }
    writer.flush()?;
    Ok(())
}
