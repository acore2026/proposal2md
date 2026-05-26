use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use assert_cmd::Command;
use image::GenericImageView;
use image::ImageReader;
use predicates::prelude::*;
use tempfile::tempdir;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

#[test]
fn converts_basic_docx_to_markdown() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("basic.docx");
    let output = temp.path().join("basic.md");

    write_docx(
        &input,
        r#"
        <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>1.Introduction</w:t></w:r></w:p>
        <w:p><w:r><w:t>Hello proposal</w:t></w:r></w:p>
        <w:tbl>
          <w:tr>
            <w:tc><w:p><w:r><w:t>Aspect</w:t></w:r></w:p></w:tc>
            <w:tc><w:p><w:r><w:t>Covered</w:t></w:r></w:p></w:tc>
          </w:tr>
          <w:tr>
            <w:tc><w:p><w:r><w:t>Intent</w:t></w:r></w:p></w:tc>
            <w:tc><w:p><w:r><w:t>Yes</w:t></w:r></w:p></w:tc>
          </w:tr>
        </w:tbl>
        "#,
        "",
        &[],
    );

    Command::cargo_bin("proposal2md")
        .unwrap()
        .args([input.as_os_str(), "-o".as_ref(), output.as_os_str()])
        .assert()
        .success()
        .stdout(predicate::str::contains("basic.docx"));

    let markdown = fs::read_to_string(&output).unwrap();
    assert!(markdown.contains("# 1.Introduction"));
    assert!(markdown.contains("Hello proposal"));
    assert!(markdown.contains("| Aspect | Covered |"));
    assert!(markdown.contains("| Intent | Yes |"));
}

#[test]
fn keeps_original_asset_when_png_conversion_fails() {
    let temp = tempdir().unwrap();
    let input = temp.path().join("media.docx");
    let output = temp.path().join("out");

    write_docx(
        &input,
        r#"
        <w:p>
          <w:r><w:t>Figure 1</w:t></w:r>
          <w:r><w:pict><v:shape><v:imagedata r:id="rId1"/></v:shape><o:OLEObject r:id="rId2"/></w:pict></w:r>
        </w:p>
        <w:p>
          <w:r><w:drawing><a:blip r:embed="rId3"/></w:drawing></w:r>
        </w:p>
        "#,
        r#"
        <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.emf"/>
        <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/package" Target="embeddings/diagram.vsdx"/>
        <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>
        "#,
        &[
            ("word/media/image1.emf", b"emf".as_slice()),
            ("word/embeddings/diagram.vsdx", b"vsdx".as_slice()),
            ("word/media/image2.png", b"png".as_slice()),
        ],
    );

    Command::cargo_bin("proposal2md")
        .unwrap()
        .args([input.as_os_str(), "-o".as_ref(), output.as_os_str()])
        .assert()
        .success();

    let markdown = fs::read_to_string(output.join("media.md")).unwrap();
    assert!(markdown.contains("Unsupported figure"));
    assert!(markdown.contains("[image1.emf](media_assets/image1.emf)"));
    assert!(!markdown.contains("diagram.vsdx"));
    assert!(markdown.contains("![image2](media_assets/image2.png)"));

    assert!(output.join("media_assets/image1.emf").exists());
    assert!(!output.join("media_assets/diagram.vsdx").exists());
    assert!(output.join("media_assets/image2.png").exists());

    let report = fs::read_to_string(output.join("media.report.json")).unwrap();
    assert!(report.contains("\"kind\": \"unsupported-image\""));
    assert!(report.contains("PNG conversion failed"));
}

#[test]
fn converts_sample_proposal_directory_when_present() {
    let proposal_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("proposal");
    if !proposal_dir.exists() {
        return;
    }

    let temp = tempdir().unwrap();
    let output = temp.path().join("converted");

    Command::cargo_bin("proposal2md")
        .unwrap()
        .args([proposal_dir.as_os_str(), "-o".as_ref(), output.as_os_str()])
        .assert()
        .success();

    assert!(output.join("S2-2600434.md").exists());
    assert!(output.join("S2-2602109.md").exists());
    assert!(output.join("S2-2600434.report.json").exists());
    assert!(output.join("S2-2602109.report.json").exists());

    let (width, height) = image_dimensions(&output.join("S2-2600434_assets/image1.png"));
    assert!(width < 794, "expected trimmed width, got {width}");
    assert!(height < 1123, "expected trimmed height, got {height}");

    let first = fs::read_to_string(output.join("S2-2600434.md")).unwrap();
    let second = fs::read_to_string(output.join("S2-2602109.md")).unwrap();
    assert!(!first.contains("Unsupported figure"));
    assert!(!second.contains("Unsupported figure"));
    assert!(!first.contains(".emf"));
    assert!(!second.contains(".vsdx"));
    assert!(first.contains("![image1](S2-2600434_assets/image1.png)"));
    assert!(second.contains("![image16](S2-2602109_assets/image16.png)"));

    let first_report = fs::read_to_string(output.join("S2-2600434.report.json")).unwrap();
    let second_report = fs::read_to_string(output.join("S2-2602109.report.json")).unwrap();
    assert!(first_report.contains("\"unsupported_assets\": []"));
    assert!(second_report.contains("\"unsupported_assets\": []"));
}

fn image_dimensions(path: &Path) -> (u32, u32) {
    ImageReader::open(path)
        .unwrap()
        .decode()
        .unwrap()
        .dimensions()
}

fn write_docx(path: &Path, body_xml: &str, relationships_xml: &str, files: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(CompressionMethod::Stored);

    zip.start_file("[Content_Types].xml", options).unwrap();
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
</Types>"#,
    )
    .unwrap();

    zip.start_file("word/document.xml", options).unwrap();
    zip.write_all(document_xml(body_xml).as_bytes()).unwrap();

    zip.start_file("word/_rels/document.xml.rels", options)
        .unwrap();
    zip.write_all(relationships(relationships_xml).as_bytes())
        .unwrap();

    for (path, contents) in files {
        zip.start_file(*path, options).unwrap();
        zip.write_all(contents).unwrap();
    }

    zip.finish().unwrap();
}

fn document_xml(body_xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document
  xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
  xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
  xmlns:v="urn:schemas-microsoft-com:vml"
  xmlns:o="urn:schemas-microsoft-com:office:office"
  xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <w:body>{body_xml}<w:sectPr/></w:body>
</w:document>"#
    )
}

fn relationships(relationships_xml: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  {relationships_xml}
</Relationships>"#
    )
}
