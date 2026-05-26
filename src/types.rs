use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub overwrite: bool,
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversionReport {
    pub source_path: String,
    pub output_path: String,
    pub report_path: String,
    pub asset_dir: String,
    pub paragraph_count: usize,
    pub table_count: usize,
    pub media_count: usize,
    pub unsupported_assets: Vec<UnsupportedAsset>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnsupportedAsset {
    pub relationship_id: String,
    pub kind: String,
    pub source_path: String,
    pub output_path: String,
}
