use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use image::{GenericImageView, ImageReader, Rgba};

const TRIM_TOLERANCE: u8 = 18;
const TRIM_PADDING: u32 = 16;

pub(crate) fn can_convert_to_png(extension: &str) -> bool {
    matches!(extension, "emf" | "wmf" | "vsd" | "vsdx")
}

pub(crate) fn convert_to_png(
    bytes: &[u8],
    source_name: &str,
    output_dir: &Path,
    output_name: &str,
) -> Result<PathBuf> {
    let temp_dir = output_dir.join(format!(
        ".proposal2md-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir)
        .with_context(|| format!("failed to create {}", temp_dir.display()))?;

    let input_path = temp_dir.join(source_name);
    fs::write(&input_path, bytes)
        .with_context(|| format!("failed to write {}", input_path.display()))?;

    let output = Command::new("soffice")
        .args(["--headless", "--convert-to", "png", "--outdir"])
        .arg(output_dir)
        .arg(&input_path)
        .output()
        .context("failed to run soffice; install LibreOffice Draw for EMF/Visio PNG conversion")?;

    let expected = output_dir.join(
        Path::new(source_name)
            .with_extension("png")
            .file_name()
            .ok_or_else(|| anyhow!("invalid conversion source name: {source_name}"))?,
    );
    let final_path = output_dir.join(output_name);

    if !output.status.success() || !expected.exists() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = fs::remove_dir_all(&temp_dir);
        bail_conversion(source_name, &stdout, &stderr)?;
    }

    if expected != final_path {
        fs::rename(&expected, &final_path).with_context(|| {
            format!(
                "failed to rename converted PNG {} to {}",
                expected.display(),
                final_path.display()
            )
        })?;
    }

    trim_png_margin(&final_path)
        .with_context(|| format!("failed to trim PNG margin for {}", final_path.display()))?;

    fs::remove_dir_all(&temp_dir)
        .with_context(|| format!("failed to remove {}", temp_dir.display()))?;
    Ok(final_path)
}

fn bail_conversion(source_name: &str, stdout: &str, stderr: &str) -> Result<()> {
    Err(anyhow!(
        "failed to convert {source_name} to PNG with LibreOffice Draw; stdout: {}; stderr: {}",
        stdout.trim(),
        stderr.trim()
    ))
}

fn trim_png_margin(path: &Path) -> Result<()> {
    let image = ImageReader::open(path)
        .with_context(|| format!("failed to open {}", path.display()))?
        .decode()
        .with_context(|| format!("failed to decode {}", path.display()))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let background = *image.get_pixel(0, 0);

    let Some((min_x, min_y, max_x, max_y)) = content_bounds(&image, background) else {
        return Ok(());
    };

    let min_x = min_x.saturating_sub(TRIM_PADDING);
    let min_y = min_y.saturating_sub(TRIM_PADDING);
    let max_x = (max_x + TRIM_PADDING).min(width - 1);
    let max_y = (max_y + TRIM_PADDING).min(height - 1);
    let crop_width = max_x - min_x + 1;
    let crop_height = max_y - min_y + 1;

    if crop_width == width && crop_height == height {
        return Ok(());
    }

    let cropped = image.view(min_x, min_y, crop_width, crop_height).to_image();
    cropped
        .save(path)
        .with_context(|| format!("failed to write trimmed PNG {}", path.display()))?;

    Ok(())
}

fn content_bounds(image: &image::RgbaImage, background: Rgba<u8>) -> Option<(u32, u32, u32, u32)> {
    let (width, height) = image.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;

    for y in 0..height {
        for x in 0..width {
            if !is_background(*image.get_pixel(x, y), background) {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }

    (min_x <= max_x && min_y <= max_y).then_some((min_x, min_y, max_x, max_y))
}

fn is_background(pixel: Rgba<u8>, background: Rgba<u8>) -> bool {
    if pixel[3] <= TRIM_TOLERANCE {
        return true;
    }

    channels_close(pixel[0], background[0])
        && channels_close(pixel[1], background[1])
        && channels_close(pixel[2], background[2])
        && channels_close(pixel[3], background[3])
}

fn channels_close(left: u8, right: u8) -> bool {
    left.abs_diff(right) <= TRIM_TOLERANCE
}
