use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use regex::Regex;
use roxmltree::{Document, Node};
use serde::Serialize;
use zip::ZipArchive;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

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

#[derive(Debug, Clone)]
struct ConversionJob {
    source_path: PathBuf,
    output_path: PathBuf,
    report_path: PathBuf,
    asset_dir: PathBuf,
    asset_link_prefix: String,
}

#[derive(Debug, Clone)]
struct Relationship {
    id: String,
    relationship_type: String,
    target: String,
    target_mode: Option<String>,
    zip_path: Option<String>,
}

#[derive(Debug, Default)]
struct StyleMap {
    styles: HashMap<String, StyleInfo>,
}

#[derive(Debug)]
struct StyleInfo {
    name: String,
}

#[derive(Debug, Default)]
struct Numbering {
    formats: HashMap<(String, String), String>,
}

#[derive(Debug, Clone)]
struct AssetLink {
    source_path: String,
    output_path: String,
    link_path: String,
    file_name: String,
    extension: String,
    supported: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    strike: bool,
}

#[derive(Debug, Clone)]
struct InlineFragment {
    text: String,
    style: InlineStyle,
}

pub fn convert(options: ConvertOptions) -> Result<Vec<ConversionReport>> {
    let jobs = plan_jobs(&options)?;
    let mut reports = Vec::with_capacity(jobs.len());

    for job in jobs {
        let report = convert_job(&job, options.overwrite, options.strict)?;
        reports.push(report);
    }

    Ok(reports)
}

fn plan_jobs(options: &ConvertOptions) -> Result<Vec<ConversionJob>> {
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

fn prepare_output(job: &ConversionJob, overwrite: bool) -> Result<()> {
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

struct Renderer<'a> {
    package: &'a mut DocxPackage,
    job: &'a ConversionJob,
    relationships: HashMap<String, Relationship>,
    styles: StyleMap,
    numbering: Numbering,
    copied_assets: HashMap<String, AssetLink>,
    used_file_names: HashSet<String>,
    paragraph_count: usize,
    table_count: usize,
    media_count: usize,
    unsupported_assets: Vec<UnsupportedAsset>,
    warnings: Vec<String>,
}

impl<'a> Renderer<'a> {
    fn new(
        package: &'a mut DocxPackage,
        job: &'a ConversionJob,
        relationships: HashMap<String, Relationship>,
        styles: StyleMap,
        numbering: Numbering,
    ) -> Self {
        Self {
            package,
            job,
            relationships,
            styles,
            numbering,
            copied_assets: HashMap::new(),
            used_file_names: HashSet::new(),
            paragraph_count: 0,
            table_count: 0,
            media_count: 0,
            unsupported_assets: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn render_document(&mut self, document: &Document<'_>) -> Result<String> {
        let body = document
            .descendants()
            .find(|node| is_w_tag(*node, "body"))
            .ok_or_else(|| anyhow!("DOCX document has no w:body"))?;

        let mut blocks = Vec::new();
        for child in body.children().filter(Node::is_element) {
            if is_w_tag(child, "p") {
                if let Some(block) = self.render_paragraph(child)? {
                    blocks.push(block);
                }
            } else if is_w_tag(child, "tbl") {
                if let Some(block) = self.render_table(child) {
                    blocks.push(block);
                }
            }
        }

        let mut output = String::from("<!-- Generated by proposal2md. -->\n\n");
        output.push_str(&blocks.join("\n\n"));
        output.push('\n');
        Ok(output)
    }

    fn render_paragraph(&mut self, paragraph: Node<'_, '_>) -> Result<Option<String>> {
        let text = normalize_paragraph_text(&collect_inline_text(paragraph));
        let media_refs = collect_media_refs(paragraph);

        if text.is_empty() && media_refs.is_empty() {
            return Ok(None);
        }

        self.paragraph_count += 1;
        let mut blocks = Vec::new();

        if !text.is_empty() {
            if let Some(level) = self.heading_level(paragraph, &text) {
                blocks.push(format!("{} {}", "#".repeat(level), text));
            } else if let Some(list_item) = self.render_list_item(paragraph, &text) {
                blocks.push(list_item);
            } else {
                blocks.push(text);
            }
        }

        if let Some(media_block) = self.render_media_refs(&media_refs)? {
            blocks.push(media_block);
        }

        Ok(Some(blocks.join("\n\n")))
    }

    fn render_table(&mut self, table: Node<'_, '_>) -> Option<String> {
        let mut rows = Vec::new();

        for row in table.children().filter(|node| is_w_tag(*node, "tr")) {
            let mut cells = Vec::new();
            for cell in row.children().filter(|node| is_w_tag(*node, "tc")) {
                let mut parts = Vec::new();
                for paragraph in cell.children().filter(|node| is_w_tag(*node, "p")) {
                    let text = normalize_cell_text(&collect_inline_text(paragraph));
                    if !text.is_empty() {
                        parts.push(text);
                    }
                }
                cells.push(escape_table_cell(&parts.join("<br>")));
            }
            if !cells.is_empty() {
                rows.push(cells);
            }
        }

        if rows.is_empty() {
            return None;
        }

        self.table_count += 1;
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        for row in &mut rows {
            row.resize(width, String::new());
        }

        let mut lines = Vec::new();
        lines.push(markdown_table_row(&rows[0]));
        lines.push(markdown_table_separator(width));
        for row in rows.iter().skip(1) {
            lines.push(markdown_table_row(row));
        }

        Some(lines.join("\n"))
    }

    fn render_media_refs(&mut self, refs: &[String]) -> Result<Option<String>> {
        if refs.is_empty() {
            return Ok(None);
        }

        let mut assets = Vec::new();
        for relationship_id in refs {
            if let Some(asset) = self.copy_relationship_asset(relationship_id)? {
                assets.push(asset);
            }
        }

        if assets.is_empty() {
            return Ok(None);
        }

        let mut blocks = Vec::new();
        for asset in assets.iter().filter(|asset| asset.supported) {
            blocks.push(format!(
                "![{}]({})",
                markdown_alt_text(&asset.file_name),
                markdown_link_target(&asset.link_path)
            ));
        }

        let unsupported = assets
            .iter()
            .filter(|asset| !asset.supported)
            .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            let mut lines =
                vec!["> **Unsupported figure:** extracted original asset(s):".to_string()];
            for asset in unsupported {
                lines.push(format!(
                    "> - [{}]({}) ({})",
                    asset.file_name,
                    markdown_link_target(&asset.link_path),
                    asset.extension
                ));
            }
            blocks.push(lines.join("\n"));
        }

        Ok(Some(blocks.join("\n\n")))
    }

    fn copy_relationship_asset(&mut self, relationship_id: &str) -> Result<Option<AssetLink>> {
        let Some(relationship) = self.relationships.get(relationship_id).cloned() else {
            self.warnings.push(format!(
                "missing relationship for media id {relationship_id}"
            ));
            return Ok(None);
        };

        if relationship.target_mode.as_deref() == Some("External") {
            self.warnings.push(format!(
                "external relationship skipped: {} -> {}",
                relationship.id, relationship.target
            ));
            return Ok(None);
        }

        let Some(zip_path) = relationship.zip_path.clone() else {
            self.warnings.push(format!(
                "relationship {} has no package target: {}",
                relationship.id, relationship.target
            ));
            return Ok(None);
        };

        if let Some(asset) = self.copied_assets.get(&zip_path) {
            return Ok(Some(asset.clone()));
        }

        let bytes = match self.package.read_bytes(&zip_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.warnings.push(format!(
                    "failed to read asset {} for {}: {error}",
                    zip_path, relationship.id
                ));
                return Ok(None);
            }
        };

        let file_name = self.unique_asset_file_name(&zip_path)?;
        let output_path = self.job.asset_dir.join(&file_name);
        fs::write(&output_path, bytes)
            .with_context(|| format!("failed to write asset {}", output_path.display()))?;

        let extension = Path::new(&file_name)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_ascii_lowercase();
        let supported = is_supported_markdown_image(&extension);
        let link_path = format!("{}/{}", self.job.asset_link_prefix, file_name);

        let asset = AssetLink {
            source_path: zip_path.clone(),
            output_path: display_path(&output_path),
            link_path,
            file_name,
            extension,
            supported,
        };

        self.media_count += 1;
        if !asset.supported {
            self.unsupported_assets.push(UnsupportedAsset {
                relationship_id: relationship.id,
                kind: media_kind(&relationship.relationship_type, &asset.extension).to_string(),
                source_path: asset.source_path.clone(),
                output_path: asset.output_path.clone(),
            });
        }

        self.copied_assets.insert(zip_path, asset.clone());
        Ok(Some(asset))
    }

    fn unique_asset_file_name(&mut self, zip_path: &str) -> Result<String> {
        let path = Path::new(zip_path);
        let base_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("asset path has no file name: {zip_path}"))?;

        let mut candidate = base_name.to_string();
        if self.used_file_names.insert(candidate.clone()) {
            return Ok(candidate);
        }

        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("asset");
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        for index in 2usize.. {
            candidate = if extension.is_empty() {
                format!("{stem}_{index}")
            } else {
                format!("{stem}_{index}.{extension}")
            };
            if self.used_file_names.insert(candidate.clone()) {
                return Ok(candidate);
            }
        }

        unreachable!("unbounded filename search should always return")
    }

    fn heading_level(&self, paragraph: Node<'_, '_>, text: &str) -> Option<usize> {
        if let Some(style_id) = paragraph_style_id(paragraph) {
            if let Some(level) = heading_level_from_style_id(&style_id) {
                return Some(level);
            }
            if let Some(style) = self.styles.styles.get(&style_id) {
                if let Some(level) = heading_level_from_style_name(&style.name) {
                    return Some(level);
                }
            }
        }

        numeric_heading_level(text)
    }

    fn render_list_item(&self, paragraph: Node<'_, '_>, text: &str) -> Option<String> {
        if let Some((num_id, ilvl)) = paragraph_numbering(paragraph) {
            let depth = ilvl.parse::<usize>().unwrap_or(0);
            let indent = "  ".repeat(depth);
            let marker = match self.numbering.format_for(&num_id, &ilvl) {
                Some("bullet") => "-",
                _ => "1.",
            };
            return Some(format!(
                "{indent}{marker} {}",
                trim_existing_list_marker(text)
            ));
        }

        let trimmed = text.trim_start();
        if let Some(rest) = trimmed.strip_prefix('-') {
            return Some(format!("- {}", rest.trim_start()));
        }
        if let Some(rest) = trimmed.strip_prefix('•') {
            return Some(format!("- {}", rest.trim_start()));
        }

        None
    }
}

struct DocxPackage {
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

impl DocxPackage {
    fn open(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let cursor = Cursor::new(bytes);
        let archive = ZipArchive::new(cursor)
            .with_context(|| format!("failed to open DOCX zip {}", path.display()))?;
        Ok(Self { archive })
    }

    fn read_string(&mut self, path: &str) -> Result<String> {
        let mut file = self
            .archive
            .by_name(path)
            .with_context(|| format!("missing DOCX member {path}"))?;
        let mut output = String::new();
        file.read_to_string(&mut output)
            .with_context(|| format!("failed to read DOCX member {path} as UTF-8"))?;
        Ok(output)
    }

    fn read_bytes(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut file = self
            .archive
            .by_name(path)
            .with_context(|| format!("missing DOCX member {path}"))?;
        let mut output = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut output)
            .with_context(|| format!("failed to read DOCX member {path}"))?;
        Ok(output)
    }
}

impl Numbering {
    fn format_for(&self, num_id: &str, ilvl: &str) -> Option<&str> {
        self.formats
            .get(&(num_id.to_string(), ilvl.to_string()))
            .map(String::as_str)
    }
}

fn parse_relationships(xml: &str) -> Result<HashMap<String, Relationship>> {
    if xml.trim().is_empty() {
        return Ok(HashMap::new());
    }

    let document = Document::parse(xml).context("failed to parse document relationships")?;
    let mut relationships = HashMap::new();

    for node in document
        .descendants()
        .filter(|node| node.has_tag_name("Relationship"))
    {
        let id = node
            .attribute("Id")
            .ok_or_else(|| anyhow!("relationship without Id"))?
            .to_string();
        let relationship_type = node.attribute("Type").unwrap_or_default().to_string();
        let target = node.attribute("Target").unwrap_or_default().to_string();
        let target_mode = node.attribute("TargetMode").map(str::to_string);
        let zip_path = if target_mode.as_deref() == Some("External") {
            None
        } else {
            Some(resolve_zip_target("word", &target))
        };

        relationships.insert(
            id.clone(),
            Relationship {
                id,
                relationship_type,
                target,
                target_mode,
                zip_path,
            },
        );
    }

    Ok(relationships)
}

fn parse_styles(xml: &str) -> Result<StyleMap> {
    if xml.trim().is_empty() {
        return Ok(StyleMap::default());
    }

    let document = Document::parse(xml).context("failed to parse word/styles.xml")?;
    let mut styles = HashMap::new();

    for node in document
        .descendants()
        .filter(|node| is_w_tag(*node, "style"))
    {
        let Some(style_id) = node.attribute((W_NS, "styleId")) else {
            continue;
        };
        let name = child(node, "name")
            .and_then(|name| name.attribute((W_NS, "val")))
            .unwrap_or(style_id)
            .to_string();
        styles.insert(style_id.to_string(), StyleInfo { name });
    }

    Ok(StyleMap { styles })
}

fn parse_numbering(xml: &str) -> Result<Numbering> {
    if xml.trim().is_empty() {
        return Ok(Numbering::default());
    }

    let document = Document::parse(xml).context("failed to parse word/numbering.xml")?;
    let mut abstract_formats: HashMap<(String, String), String> = HashMap::new();
    let mut num_to_abstract: HashMap<String, String> = HashMap::new();

    for abstract_num in document
        .descendants()
        .filter(|node| is_w_tag(*node, "abstractNum"))
    {
        let Some(abstract_id) = abstract_num.attribute((W_NS, "abstractNumId")) else {
            continue;
        };
        for level in abstract_num
            .children()
            .filter(|node| is_w_tag(*node, "lvl"))
        {
            let ilvl = level.attribute((W_NS, "ilvl")).unwrap_or("0");
            let Some(num_fmt) = child(level, "numFmt").and_then(|fmt| fmt.attribute((W_NS, "val")))
            else {
                continue;
            };
            abstract_formats.insert(
                (abstract_id.to_string(), ilvl.to_string()),
                num_fmt.to_string(),
            );
        }
    }

    for num in document.descendants().filter(|node| is_w_tag(*node, "num")) {
        let Some(num_id) = num.attribute((W_NS, "numId")) else {
            continue;
        };
        let Some(abstract_id) =
            child(num, "abstractNumId").and_then(|node| node.attribute((W_NS, "val")))
        else {
            continue;
        };
        num_to_abstract.insert(num_id.to_string(), abstract_id.to_string());
    }

    let mut formats = HashMap::new();
    for (num_id, abstract_id) in num_to_abstract {
        for ((candidate_abstract_id, ilvl), format) in &abstract_formats {
            if candidate_abstract_id == &abstract_id {
                formats.insert((num_id.clone(), ilvl.clone()), format.clone());
            }
        }
    }

    Ok(Numbering { formats })
}

fn collect_inline_text(node: Node<'_, '_>) -> String {
    let mut fragments = Vec::new();
    collect_inline_fragments(node, &mut fragments);
    render_inline_fragments(fragments)
}

fn collect_inline_fragments(node: Node<'_, '_>, fragments: &mut Vec<InlineFragment>) {
    if is_w_tag(node, "del") {
        return;
    }

    if is_w_tag(node, "r") {
        let text = collect_run_raw_text(node);
        if !text.is_empty() {
            fragments.push(InlineFragment {
                text,
                style: run_style(node),
            });
        }
        return;
    }

    for child_node in node.children() {
        collect_inline_fragments(child_node, fragments);
    }
}

fn collect_run_raw_text(run: Node<'_, '_>) -> String {
    let mut output = String::new();

    for node in run.descendants() {
        if is_w_tag(node, "t") {
            output.push_str(node.text().unwrap_or_default());
        } else if is_w_tag(node, "tab") {
            output.push(' ');
        } else if is_w_tag(node, "br") {
            output.push('\n');
        }
    }

    output
}

fn run_style(run: Node<'_, '_>) -> InlineStyle {
    let Some(properties) = child(run, "rPr") else {
        return InlineStyle::default();
    };

    InlineStyle {
        bold: has_child(properties, "b"),
        italic: has_child(properties, "i"),
        strike: has_child(properties, "strike") || has_child(properties, "dstrike"),
    }
}

fn render_inline_fragments(fragments: Vec<InlineFragment>) -> String {
    let mut merged: Vec<InlineFragment> = Vec::new();

    for fragment in fragments {
        if let Some(previous) = merged.last_mut() {
            if previous.style == fragment.style {
                previous.text.push_str(&fragment.text);
                continue;
            }
        }
        merged.push(fragment);
    }

    merged
        .into_iter()
        .map(|fragment| {
            apply_inline_style(
                &fragment.text,
                fragment.style.bold,
                fragment.style.italic,
                fragment.style.strike,
            )
        })
        .collect()
}

fn apply_inline_style(text: &str, bold: bool, italic: bool, strike: bool) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }

    let leading_len = text.len() - text.trim_start().len();
    let trailing_len = text.len() - text.trim_end().len();
    let leading = &text[..leading_len];
    let trailing = if trailing_len == 0 {
        ""
    } else {
        &text[text.len() - trailing_len..]
    };
    let core_end = text.len() - trailing_len;
    let core = &text[leading_len..core_end];

    if core.is_empty() {
        return text.to_string();
    }

    let mut styled = match (bold, italic) {
        (true, true) => format!("***{core}***"),
        (true, false) => format!("**{core}**"),
        (false, true) => format!("*{core}*"),
        (false, false) => core.to_string(),
    };

    if strike {
        styled = format!("~~{styled}~~");
    }

    format!("{leading}{styled}{trailing}")
}

fn collect_media_refs(paragraph: Node<'_, '_>) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    for node in paragraph.descendants() {
        let relationship_id = if node.has_tag_name("imagedata") {
            node.attribute((R_NS, "id"))
        } else if node.has_tag_name("blip") {
            node.attribute((R_NS, "embed"))
        } else if node.has_tag_name("OLEObject") {
            node.attribute((R_NS, "id"))
        } else {
            None
        };

        if let Some(relationship_id) = relationship_id {
            if seen.insert(relationship_id.to_string()) {
                refs.push(relationship_id.to_string());
            }
        }
    }

    refs
}

fn paragraph_style_id(paragraph: Node<'_, '_>) -> Option<String> {
    child(paragraph, "pPr")
        .and_then(|properties| child(properties, "pStyle"))
        .and_then(|style| style.attribute((W_NS, "val")))
        .map(str::to_string)
}

fn paragraph_numbering(paragraph: Node<'_, '_>) -> Option<(String, String)> {
    let num_pr = child(paragraph, "pPr").and_then(|properties| child(properties, "numPr"))?;
    let num_id = child(num_pr, "numId").and_then(|node| node.attribute((W_NS, "val")))?;
    let ilvl = child(num_pr, "ilvl")
        .and_then(|node| node.attribute((W_NS, "val")))
        .unwrap_or("0");
    Some((num_id.to_string(), ilvl.to_string()))
}

fn heading_level_from_style_id(style_id: &str) -> Option<usize> {
    if let Some(level) = style_id.strip_prefix("Heading") {
        return parse_heading_level(level);
    }

    if style_id.len() == 1 && style_id.chars().all(|ch| ch.is_ascii_digit()) {
        return parse_heading_level(style_id);
    }

    None
}

fn heading_level_from_style_name(style_name: &str) -> Option<usize> {
    let lowercase = style_name.to_ascii_lowercase();
    let level = lowercase.strip_prefix("heading ")?;
    parse_heading_level(level)
}

fn parse_heading_level(level: &str) -> Option<usize> {
    let level = level.parse::<usize>().ok()?;
    (1..=6).contains(&level).then_some(level)
}

fn numeric_heading_level(text: &str) -> Option<usize> {
    if text.len() > 120 || text.ends_with(':') {
        return None;
    }

    let tokens = parse_section_tokens(text)?;
    if tokens.len() < 2 {
        return None;
    }

    Some(tokens.len().min(6))
}

fn parse_section_tokens(text: &str) -> Option<Vec<String>> {
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0;
    let mut tokens = Vec::new();

    let first = consume_digits(&chars, &mut index)?;
    tokens.push(first);

    while index < chars.len() && chars[index] == '.' {
        let dot_index = index;
        index += 1;

        if let Some(number) = consume_digits(&chars, &mut index) {
            tokens.push(number);
            continue;
        }

        if tokens.len() >= 2
            && index < chars.len()
            && chars[index].is_ascii_uppercase()
            && looks_like_letter_section_token(&chars, index)
        {
            tokens.push(chars[index].to_string());
            index += 1;
            continue;
        }

        index = dot_index + 1;
        break;
    }

    let rest = chars[index..].iter().collect::<String>();
    (!rest.trim().is_empty()
        && rest
            .trim_start()
            .starts_with(|ch: char| ch.is_alphanumeric()))
    .then_some(tokens)
}

fn consume_digits(chars: &[char], index: &mut usize) -> Option<String> {
    let start = *index;
    while *index < chars.len() && chars[*index].is_ascii_digit() {
        *index += 1;
    }

    (*index > start).then(|| chars[start..*index].iter().collect())
}

fn looks_like_letter_section_token(chars: &[char], index: usize) -> bool {
    if index + 1 >= chars.len() {
        return true;
    }

    let next = chars[index + 1];
    next == '.' || next.is_ascii_uppercase() || next.is_whitespace()
}

fn normalize_paragraph_text(text: &str) -> String {
    text.replace('\u{00a0}', " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("<br>")
        .trim()
        .to_string()
}

fn normalize_cell_text(text: &str) -> String {
    text.replace('\u{00a0}', " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("<br>")
        .trim()
        .to_string()
}

fn trim_existing_list_marker(text: &str) -> &str {
    let trimmed = text.trim_start();
    if let Some(rest) = trimmed.strip_prefix('-') {
        return rest.trim_start();
    }
    if let Some(rest) = trimmed.strip_prefix('•') {
        return rest.trim_start();
    }

    let ordered = Regex::new(r"^\d+[\.)]\s*").expect("valid ordered-list regex");
    if let Some(match_) = ordered.find(trimmed) {
        return &trimmed[match_.end()..];
    }

    trimmed
}

fn markdown_table_row(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

fn markdown_table_separator(width: usize) -> String {
    format!("| {} |", vec!["---"; width].join(" | "))
}

fn escape_table_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn markdown_alt_text(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("figure")
        .replace(['[', ']'], "")
}

fn markdown_link_target(path: &str) -> String {
    if path
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '(' | ')' | '<' | '>'))
    {
        format!("<{}>", path.replace('>', "%3E"))
    } else {
        path.to_string()
    }
}

fn is_supported_markdown_image(extension: &str) -> bool {
    matches!(
        extension,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp"
    )
}

fn media_kind(relationship_type: &str, extension: &str) -> &'static str {
    if relationship_type.ends_with("/oleObject") {
        "ole-object"
    } else if relationship_type.ends_with("/package") {
        "embedded-package"
    } else if matches!(extension, "emf" | "wmf") {
        "unsupported-image"
    } else {
        "unsupported-media"
    }
}

fn child<'a, 'input>(node: Node<'a, 'input>, local_name: &str) -> Option<Node<'a, 'input>> {
    node.children().find(|child| is_w_tag(*child, local_name))
}

fn has_child(node: Node<'_, '_>, local_name: &str) -> bool {
    child(node, local_name).is_some()
}

fn is_w_tag(node: Node<'_, '_>, local_name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == local_name
        && node.tag_name().namespace() == Some(W_NS)
}

fn resolve_zip_target(base_dir: &str, target: &str) -> String {
    if target.starts_with('/') {
        return target.trim_start_matches('/').to_string();
    }

    let mut parts = base_dir
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();

    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value.to_string()),
        }
    }

    parts.join("/")
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

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_heading_levels() {
        assert_eq!(
            numeric_heading_level("6.18.YSolution #18.Y: Title"),
            Some(3)
        );
        assert_eq!(numeric_heading_level("6.18.Y.1Description"), Some(4));
        assert_eq!(
            numeric_heading_level("1.Study the support for control signalling:"),
            None
        );
        assert_eq!(numeric_heading_level("1.Introduction"), None);
    }

    #[test]
    fn skips_deleted_text_and_keeps_inserted_text() {
        let xml = r#"
        <w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:r><w:t>Keep </w:t></w:r>
          <w:del><w:r><w:delText>Drop</w:delText></w:r></w:del>
          <w:ins><w:r><w:t>Insert</w:t></w:r></w:ins>
        </w:p>"#;
        let document = Document::parse(xml).unwrap();
        assert_eq!(collect_inline_text(document.root_element()), "Keep Insert");
    }

    #[test]
    fn resolves_document_relationship_targets() {
        assert_eq!(
            resolve_zip_target("word", "media/image1.emf"),
            "word/media/image1.emf"
        );
        assert_eq!(
            resolve_zip_target("word", "../customXml/item1.xml"),
            "customXml/item1.xml"
        );
    }

    #[test]
    fn table_cells_escape_pipes() {
        assert_eq!(escape_table_cell("a | b"), "a \\| b");
    }
}
