use std::fs;

use anyhow::{bail, Context, Result};
use roxmltree::Document;

use crate::docx::{parse_relationships, DocxPackage};
use crate::job::{display_path, plan_jobs, prepare_output, ConversionJob};
use crate::render::Renderer;
use crate::types::{ConversionReport, ConvertOptions};
use crate::word::{parse_numbering, parse_styles};

pub fn convert(options: ConvertOptions) -> Result<Vec<ConversionReport>> {
    let jobs = plan_jobs(&options)?;
    let mut reports = Vec::with_capacity(jobs.len());

    for job in jobs {
        let report = convert_job(&job, options.overwrite, options.strict)?;
        reports.push(report);
    }

    Ok(reports)
}

fn convert_job(job: &ConversionJob, overwrite: bool, strict: bool) -> Result<ConversionReport> {
    prepare_output(job, overwrite)?;

    let mut package = DocxPackage::open(&job.source_path)?;
    let document_xml = package
        .read_string("word/document.xml")
        .context("DOCX is missing word/document.xml")?;
    let relationships_xml = package
        .read_string("word/_rels/document.xml.rels")
        .unwrap_or_default();
    let styles_xml = package.read_string("word/styles.xml").unwrap_or_default();
    let numbering_xml = package
        .read_string("word/numbering.xml")
        .unwrap_or_default();

    let relationships = parse_relationships(&relationships_xml)?;
    let styles = parse_styles(&styles_xml)?;
    let numbering = parse_numbering(&numbering_xml)?;
    let document = Document::parse(&document_xml).context("failed to parse word/document.xml")?;

    let mut renderer = Renderer::new(&mut package, job, relationships, styles, numbering);
    let markdown = renderer.render_document(&document)?;

    fs::write(&job.output_path, markdown)
        .with_context(|| format!("failed to write {}", job.output_path.display()))?;

    let report = ConversionReport {
        source_path: display_path(&job.source_path),
        output_path: display_path(&job.output_path),
        report_path: display_path(&job.report_path),
        asset_dir: display_path(&job.asset_dir),
        paragraph_count: renderer.paragraph_count,
        table_count: renderer.table_count,
        media_count: renderer.media_count,
        unsupported_assets: renderer.unsupported_assets,
        warnings: renderer.warnings,
    };

    let report_json = serde_json::to_string_pretty(&report)?;
    fs::write(&job.report_path, format!("{report_json}\n"))
        .with_context(|| format!("failed to write {}", job.report_path.display()))?;

    if strict && (!report.unsupported_assets.is_empty() || !report.warnings.is_empty()) {
        bail!(
            "strict conversion failed for {}: {} unsupported assets, {} warnings",
            job.source_path.display(),
            report.unsupported_assets.len(),
            report.warnings.len()
        );
    }

    Ok(report)
}
