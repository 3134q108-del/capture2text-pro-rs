use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use capture2text_pro_rs_lib::leptonica::{LeptonicaError, Pix};
use chrono::Local;
use capture2text_pro_rs_lib::preprocess::{
    extract_text_block, ExtractParams, ExtractResult, OCR_SCALE_FACTOR_DEFAULT,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum CheckError {
    #[error("LOCALAPPDATA is not set")]
    MissingLocalAppData,
    #[error("capture directory does not exist: {0}")]
    CaptureDirectoryMissing(String),
    #[error("no PNG files found in capture directory: {0}")]
    NoPngFound(String),
    #[error("{op} failed for {path}: {source}")]
    Io {
        op: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error(transparent)]
    Leptonica(#[from] LeptonicaError),
    #[error("{0}")]
    Validation(String),
}

impl From<io::Error> for CheckError {
    fn from(source: io::Error) -> Self {
        Self::Io {
            op: "io",
            path: PathBuf::from("<unknown>"),
            source,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("leptonica_check failed: {err:#?}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CheckError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or(CheckError::MissingLocalAppData)?;
    let app_data_root = PathBuf::from(local_app_data).join("Capture2TextPro");
    let captures_dir = app_data_root.join("captures");
    if !captures_dir.exists() {
        return Err(CheckError::CaptureDirectoryMissing(
            captures_dir.display().to_string(),
        ));
    }

    let input_path = find_latest_png(&captures_dir)?;
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("leptonica_check");
    ensure_dir(&output_dir)?;

    let src = Pix::read(&input_path)?;
    let gray = src.convert_to_8()?;
    let scaled = gray.scale(1.25, 1.25)?;
    let binary = scaled.convert_to_1(128)?;
    let cleaned = binary.remove_border_conn_comps(8)?;
    let conn_comp_count = cleaned.conn_comp_count(8)?;

    let output_name = format!("leptonica_check_{}.png", Local::now().format("%Y%m%d_%H%M%S"));
    let output_path = output_dir.join(output_name);
    cleaned.write_png(&output_path)?;

    let written = Pix::read(&output_path)?;
    validate_output(&cleaned, &written, &output_path)?;

    println!("Input  : {}", input_path.display());
    println!("Output : {}", output_path.display());
    println!(
        "Size   : {}x{} -> {}x{}",
        src.width(),
        src.height(),
        written.width(),
        written.height()
    );
    println!("Depth  : {} -> {}", src.depth(), written.depth());
    println!("ConnComp(8): {conn_comp_count}");

    let extract_params = ExtractParams {
        pt_x: src.width() / 2,
        pt_y: src.height() / 2,
        lookahead: 14,
        lookbehind: 1,
        search_radius: 30,
        vertical: false,
        scale_factor: OCR_SCALE_FACTOR_DEFAULT,
    };

    match extract_text_block(&src, extract_params)? {
        Some(ExtractResult {
            cropped_bin,
            bbox_unscaled,
            bbox_scaled,
        }) => {
            let extract_output_name = format!(
                "leptonica_check_extract_{}.png",
                Local::now().format("%Y%m%d_%H%M%S")
            );
            let extract_output_path = output_dir.join(extract_output_name);
            cropped_bin.write_png(&extract_output_path)?;

            println!("Extract  : {}", extract_output_path.display());
            println!(
                "BBoxScaled  : x={},y={},w={},h={}",
                bbox_scaled.x, bbox_scaled.y, bbox_scaled.w, bbox_scaled.h
            );
            println!(
                "BBoxUnscale : x={},y={},w={},h={}",
                bbox_unscaled.x, bbox_unscaled.y, bbox_unscaled.w, bbox_unscaled.h
            );
            println!(
                "CroppedSize : {}x{} @ depth={}",
                cropped_bin.width(),
                cropped_bin.height(),
                cropped_bin.depth()
            );
        }
        None => {
            println!("extract: no text block (bbox too small)");
        }
    }

    Ok(())
}

fn find_latest_png(captures_dir: &Path) -> Result<PathBuf, CheckError> {
    let mut latest: Option<(SystemTime, PathBuf)> = None;

    for entry in read_dir_with_context(captures_dir)? {
        let entry = entry.map_err(|source| CheckError::Io {
            op: "read_dir_entry",
            path: captures_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !is_png_file(&path) {
            continue;
        }

        let modified = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);

        match &latest {
            Some((best_time, _)) if modified <= *best_time => {}
            _ => latest = Some((modified, path)),
        }
    }

    latest
        .map(|(_, path)| path)
        .ok_or_else(|| CheckError::NoPngFound(captures_dir.display().to_string()))
}

fn ensure_dir(path: &Path) -> Result<(), CheckError> {
    fs::create_dir_all(path).map_err(|source| CheckError::Io {
        op: "create_dir_all",
        path: path.to_path_buf(),
        source,
    })
}

fn read_dir_with_context(path: &Path) -> Result<fs::ReadDir, CheckError> {
    fs::read_dir(path).map_err(|source| CheckError::Io {
        op: "read_dir",
        path: path.to_path_buf(),
        source,
    })
}

fn is_png_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("png"))
        .unwrap_or(false)
}

fn validate_output(expected: &Pix, actual: &Pix, output_path: &Path) -> Result<(), CheckError> {
    if actual.width() != expected.width() || actual.height() != expected.height() {
        return Err(CheckError::Validation(format!(
            "dimension mismatch after write/read: expected {}x{}, got {}x{} ({})",
            expected.width(),
            expected.height(),
            actual.width(),
            actual.height(),
            output_path.display()
        )));
    }

    if actual.depth() != expected.depth() {
        return Err(CheckError::Validation(format!(
            "depth mismatch after write/read: expected {}, got {} ({})",
            expected.depth(),
            actual.depth(),
            output_path.display()
        )));
    }

    if actual.depth() != 1 {
        return Err(CheckError::Validation(format!(
            "expected 1bpp binary output, got {}bpp ({})",
            actual.depth(),
            output_path.display()
        )));
    }

    Ok(())
}
