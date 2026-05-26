use std::path::Path;

use regex::Regex;

pub(crate) fn normalize_paragraph_text(text: &str) -> String {
    text.replace('\u{00a0}', " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("<br>")
        .trim()
        .to_string()
}

pub(crate) fn normalize_cell_text(text: &str) -> String {
    text.replace('\u{00a0}', " ")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("<br>")
        .trim()
        .to_string()
}

pub(crate) fn trim_existing_list_marker(text: &str) -> &str {
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

pub(crate) fn table_row(cells: &[String]) -> String {
    format!("| {} |", cells.join(" | "))
}

pub(crate) fn table_separator(width: usize) -> String {
    format!("| {} |", vec!["---"; width].join(" | "))
}

pub(crate) fn escape_table_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

pub(crate) fn alt_text(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("figure")
        .replace(['[', ']'], "")
}

pub(crate) fn link_target(path: &str) -> String {
    if path
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '(' | ')' | '<' | '>'))
    {
        format!("<{}>", path.replace('>', "%3E"))
    } else {
        path.to_string()
    }
}

pub(crate) fn is_supported_image(extension: &str) -> bool {
    matches!(
        extension,
        "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp"
    )
}

pub(crate) fn media_kind(relationship_type: &str, extension: &str) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_cells_escape_pipes() {
        assert_eq!(escape_table_cell("a | b"), "a \\| b");
    }
}
