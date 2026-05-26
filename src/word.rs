use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use roxmltree::{Document, Node};

pub(crate) const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Debug, Default)]
pub(crate) struct StyleMap {
    pub(crate) styles: HashMap<String, StyleInfo>,
}

#[derive(Debug)]
pub(crate) struct StyleInfo {
    pub(crate) name: String,
}

#[derive(Debug, Default)]
pub(crate) struct Numbering {
    formats: HashMap<(String, String), String>,
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

impl Numbering {
    pub(crate) fn format_for(&self, num_id: &str, ilvl: &str) -> Option<&str> {
        self.formats
            .get(&(num_id.to_string(), ilvl.to_string()))
            .map(String::as_str)
    }
}

pub(crate) fn parse_styles(xml: &str) -> Result<StyleMap> {
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

pub(crate) fn parse_numbering(xml: &str) -> Result<Numbering> {
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

pub(crate) fn collect_inline_text(node: Node<'_, '_>) -> String {
    let mut fragments = Vec::new();
    collect_inline_fragments(node, &mut fragments);
    render_inline_fragments(fragments)
}

pub(crate) fn collect_media_refs(paragraph: Node<'_, '_>) -> Vec<String> {
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

pub(crate) fn paragraph_style_id(paragraph: Node<'_, '_>) -> Option<String> {
    child(paragraph, "pPr")
        .and_then(|properties| child(properties, "pStyle"))
        .and_then(|style| style.attribute((W_NS, "val")))
        .map(str::to_string)
}

pub(crate) fn paragraph_numbering(paragraph: Node<'_, '_>) -> Option<(String, String)> {
    let num_pr = child(paragraph, "pPr").and_then(|properties| child(properties, "numPr"))?;
    let num_id = child(num_pr, "numId").and_then(|node| node.attribute((W_NS, "val")))?;
    let ilvl = child(num_pr, "ilvl")
        .and_then(|node| node.attribute((W_NS, "val")))
        .unwrap_or("0");
    Some((num_id.to_string(), ilvl.to_string()))
}

pub(crate) fn heading_level_from_style_id(style_id: &str) -> Option<usize> {
    if let Some(level) = style_id.strip_prefix("Heading") {
        return parse_heading_level(level);
    }

    if style_id.len() == 1 && style_id.chars().all(|ch| ch.is_ascii_digit()) {
        return parse_heading_level(style_id);
    }

    None
}

pub(crate) fn heading_level_from_style_name(style_name: &str) -> Option<usize> {
    let lowercase = style_name.to_ascii_lowercase();
    let level = lowercase.strip_prefix("heading ")?;
    parse_heading_level(level)
}

pub(crate) fn numeric_heading_level(text: &str) -> Option<usize> {
    if text.len() > 120 || text.ends_with(':') {
        return None;
    }

    let tokens = parse_section_tokens(text)?;
    if tokens.len() < 2 {
        return None;
    }

    Some(tokens.len().min(6))
}

pub(crate) fn child<'a, 'input>(
    node: Node<'a, 'input>,
    local_name: &str,
) -> Option<Node<'a, 'input>> {
    node.children().find(|child| is_w_tag(*child, local_name))
}

pub(crate) fn is_w_tag(node: Node<'_, '_>, local_name: &str) -> bool {
    node.is_element()
        && node.tag_name().name() == local_name
        && node.tag_name().namespace() == Some(W_NS)
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
        bold: child(properties, "b").is_some(),
        italic: child(properties, "i").is_some(),
        strike: child(properties, "strike").is_some() || child(properties, "dstrike").is_some(),
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

fn parse_heading_level(level: &str) -> Option<usize> {
    let level = level.parse::<usize>().ok()?;
    (1..=6).contains(&level).then_some(level)
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
}
