# proposal2md

`proposal2md` converts 3GPP `.docx` proposal files into Markdown. It is a
self-contained Rust CLI: it reads DOCX ZIP/XML directly and does not require
Pandoc, LibreOffice, or Microsoft Office.

## Features

- Converts one `.docx` file or a directory of `.docx` files.
- Preserves proposal text, headings, basic inline formatting, lists, and tables.
- Extracts embedded media into a per-document asset directory.
- Links Markdown-renderable images such as PNG/JPEG/GIF/SVG.
- Converts EMF/WMF/Visio figures to PNG with LibreOffice Draw when available,
  so Markdown previews can render them directly.
- Trims the large white page margins from converted figures while keeping a
  small padding around the diagram.
- Falls back to visible placeholders only when PNG conversion fails, avoiding
  silent figure loss.
- Writes a JSON report with conversion counts, warnings, and unsupported assets.

## Usage

Install LibreOffice Draw if your proposals contain EMF, WMF, VSD, or VSDX
figures:

```bash
sudo apt-get install libreoffice-draw
```

```bash
cargo run -- proposal -o out --overwrite
```

Convert a single file:

```bash
cargo run -- proposal/S2-2600434.docx -o out --overwrite
```

Write one specific Markdown file:

```bash
cargo run -- proposal/S2-2600434.docx -o S2-2600434.md --overwrite
```

Run in strict mode to fail when unsupported figures or warnings are found:

```bash
cargo run -- proposal/S2-2600434.docx -o out --strict
```

## Output

For `proposal/S2-2600434.docx`, directory output creates:

```text
out/S2-2600434.md
out/S2-2600434.report.json
out/S2-2600434_assets/
```

Converted figures are written as PNG files, for example:

```markdown
![image1](S2-2600434_assets/image1.png)
```

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Run the public 3GPP sample corpus check:

```bash
tools/verify_public_3gpp_samples.sh
```

The script downloads 10 official SA2 contribution ZIP files from recent 3GPP
meeting folders into `/tmp`, extracts their DOCX files, converts them, and
fails if any unsupported figure placeholder or EMF/Visio Markdown reference
remains.
