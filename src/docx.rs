use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use roxmltree::Document;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub(crate) struct Relationship {
    pub(crate) id: String,
    pub(crate) relationship_type: String,
    pub(crate) target: String,
    pub(crate) target_mode: Option<String>,
    pub(crate) zip_path: Option<String>,
}

pub(crate) struct DocxPackage {
    archive: ZipArchive<Cursor<Vec<u8>>>,
}

impl DocxPackage {
    pub(crate) fn open(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let cursor = Cursor::new(bytes);
        let archive = ZipArchive::new(cursor)
            .with_context(|| format!("failed to open DOCX zip {}", path.display()))?;
        Ok(Self { archive })
    }

    pub(crate) fn read_string(&mut self, path: &str) -> Result<String> {
        let mut file = self
            .archive
            .by_name(path)
            .with_context(|| format!("missing DOCX member {path}"))?;
        let mut output = String::new();
        file.read_to_string(&mut output)
            .with_context(|| format!("failed to read DOCX member {path} as UTF-8"))?;
        Ok(output)
    }

    pub(crate) fn read_bytes(&mut self, path: &str) -> Result<Vec<u8>> {
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

pub(crate) fn parse_relationships(xml: &str) -> Result<HashMap<String, Relationship>> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
