# proposal2md

`proposal2md` converts 3GPP `.docx` proposal files into Markdown. It is a
self-contained Rust CLI: it reads DOCX ZIP/XML directly and does not require
Pandoc, LibreOffice, or Microsoft Office.

## Features

- Converts one `.docx` file or a directory of `.docx` files.
- Preserves proposal text, headings, basic inline formatting, lists, and tables.
- Extracts embedded media into a per-document asset directory.
- Links Markdown-renderable images such as PNG/JPEG/GIF/SVG.
- Extracts unsupported EMF/WMF/Visio/OLE objects and inserts visible Markdown
  placeholders instead of silently dropping figures.
- Writes a JSON report with conversion counts, warnings, and unsupported assets.

## Usage

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

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```
