use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::types::ConvertOptions;

#[derive(Debug, Clone)]
pub(crate) struct ConversionJob {
    pub(crate) source_path: PathBuf,
    pub(crate) output_path: PathBuf,
    pub(crate) report_path: PathBuf,
    pub(crate) asset_dir: PathBuf,
    pub(crate) asset_link_prefix: String,
}

pub(crate) fn plan_jobs(options: &ConvertOptions) -> Result<Vec<ConversionJob>> {
    let input = &options.input;
    if !input.exists() {
        bail!("input does not exist: {}", input.display());
    }

    let output = options
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from("out"));

    if input.is_dir() {
        if is_markdown_path(&output) {
            bail!("directory input requires an output directory, not a .md file");
        }

        let mut sources = fs::read_dir(input)
            .with_context(|| format!("failed to read input directory {}", input.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.is_file() && is_docx_path(path) && !is_temporary_office_file(path))
            .collect::<Vec<_>>();
        sources.sort();

        if sources.is_empty() {
            bail!("no .docx files found in {}", input.display());
        }

        sources
            .into_iter()
            .map(|source| job_for_output_dir(source, &output))
            .collect()
    } else if input.is_file() {
        if !is_docx_path(input) {
            bail!(
                "input file must have a .docx extension: {}",
                input.display()
            );
        }

        if is_markdown_path(&output) {
            Ok(vec![job_for_markdown_file(input.clone(), output)?])
        } else {
            Ok(vec![job_for_output_dir(input.clone(), &output)?])
        }
    } else {
        bail!(
            "input is neither a file nor a directory: {}",
            input.display()
        );
    }
}

pub(crate) fn prepare_output(job: &ConversionJob, overwrite: bool) -> Result<()> {
    ensure_parent_dir(&job.output_path)?;
    ensure_parent_dir(&job.report_path)?;

    for path in [&job.output_path, &job.report_path] {
        if path.exists() && !overwrite {
            bail!(
                "output already exists: {} (use --overwrite to replace it)",
                path.display()
            );
        }
        if path.exists() && path.is_dir() {
            bail!("output path is a directory: {}", path.display());
        }
    }

    if job.asset_dir.exists() {
        if !overwrite {
            bail!(
                "asset directory already exists: {} (use --overwrite to replace it)",
                job.asset_dir.display()
            );
        }
        if job.asset_dir.is_dir() {
            fs::remove_dir_all(&job.asset_dir)
                .with_context(|| format!("failed to remove {}", job.asset_dir.display()))?;
        } else {
            fs::remove_file(&job.asset_dir)
                .with_context(|| format!("failed to remove {}", job.asset_dir.display()))?;
        }
    }

    fs::create_dir_all(&job.asset_dir)
        .with_context(|| format!("failed to create {}", job.asset_dir.display()))?;

    Ok(())
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn job_for_output_dir(source_path: PathBuf, output_dir: &Path) -> Result<ConversionJob> {
    let source_stem = file_stem_string(&source_path)?;
    let output_path = output_dir.join(format!("{source_stem}.md"));
    let report_path = output_dir.join(format!("{source_stem}.report.json"));
    let asset_dir = output_dir.join(format!("{source_stem}_assets"));
    let asset_link_prefix = asset_dir_name(&asset_dir)?;

    Ok(ConversionJob {
        source_path,
        output_path,
        report_path,
        asset_dir,
        asset_link_prefix,
    })
}

fn job_for_markdown_file(source_path: PathBuf, output_path: PathBuf) -> Result<ConversionJob> {
    let output_stem = file_stem_string(&output_path)?;
    let parent = output_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let report_path = parent.join(format!("{output_stem}.report.json"));
    let asset_dir = parent.join(format!("{output_stem}_assets"));
    let asset_link_prefix = asset_dir_name(&asset_dir)?;

    Ok(ConversionJob {
        source_path,
        output_path,
        report_path,
        asset_dir,
        asset_link_prefix,
    })
}

fn is_docx_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("docx"))
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

fn is_temporary_office_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("~$"))
}

fn file_stem_string(path: &Path) -> Result<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("path has no valid file stem: {}", path.display()))
}

fn asset_dir_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("asset directory has no valid name: {}", path.display()))
}

fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}
