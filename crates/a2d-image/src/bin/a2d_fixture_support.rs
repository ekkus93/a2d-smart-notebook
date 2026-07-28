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
use std::ptr::NonNull;
use std::slice;

use a2d_domain::{LayoutId, NotebookDesignId};
use a2d_identity::PageCode;
use apriltag_sys as sys;
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
    // SAFETY: constructor takes no borrowed pointers; null is checked before use.
    let family = NonNull::new(unsafe { sys::tagStandard41h12_create() })
        .ok_or("tagStandard41h12_create returned null")?;

    let result = (|| {
        // SAFETY: family remains live for this entire closure.
        let code_count = unsafe { family.as_ref().ncodes };
        for tag_id in tag_ids {
            if tag_id >= code_count {
                return Err(format!("tag ID {tag_id} is outside tagStandard41h12").into());
            }

            // SAFETY: tag_id is in range and family remains live until the image is copied.
            let image = NonNull::new(unsafe { sys::apriltag_to_image(family.as_ptr(), tag_id) })
                .ok_or_else(|| format!("apriltag_to_image returned null for tag ID {tag_id}"))?;

            let write_result = (|| {
                // SAFETY: image is live and owned by this scope.
                let native = unsafe { image.as_ref() };
                if native.width <= 0
                    || native.height <= 0
                    || native.stride < native.width
                    || native.buf.is_null()
                {
                    return Err(
                        format!("official renderer returned invalid tag image {tag_id}").into(),
                    );
                }

                let length = usize::try_from(native.stride)?
                    .checked_mul(usize::try_from(native.height)?)
                    .ok_or("rendered tag size overflowed")?;
                // SAFETY: the official image owns at least stride * height bytes.
                let bytes = unsafe { slice::from_raw_parts(native.buf, length) };
                let path = output_dir.join(format!("tag-{tag_id}.pgm"));
                write_pgm_strided(
                    &path,
                    usize::try_from(native.width)?,
                    usize::try_from(native.height)?,
                    usize::try_from(native.stride)?,
                    bytes,
                )
            })();

            // SAFETY: image came from apriltag_to_image and has not been freed.
            unsafe { sys::image_u8_destroy(image.as_ptr()) };
            write_result?;
        }
        Ok::<(), Box<dyn Error>>(())
    })();

    // SAFETY: family came from tagStandard41h12_create and remains uniquely owned.
    unsafe { sys::tagStandard41h12_destroy(family.as_ptr()) };
    result
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
