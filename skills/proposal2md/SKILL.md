---
name: proposal2md
description: Use when working with the proposal2md Rust CLI for converting 3GPP DOCX proposal files to Markdown, including running conversions, verifying output, debugging EMF/WMF/Visio figure handling, maintaining tests, or preparing releases for this tool.
---

# proposal2md

## Overview

`proposal2md` is a Rust CLI that converts 3GPP `.docx` proposal files into Markdown. Use this skill when a task involves converting proposal documents, inspecting generated Markdown/assets, fixing conversion behavior, or verifying release readiness.

The tool reads DOCX packages directly. It should not depend on Pandoc or Microsoft Office. LibreOffice Draw (`soffice`) is required when EMF, WMF, VSD, or VSDX figures need to become PNG files.

## Prerequisites

Install Rust and LibreOffice Draw before running real conversions or repository verification. LibreOffice must provide the `soffice` command; without it, text and tables can still convert, but EMF/WMF/Visio figures will fall back to warnings instead of PNG output.

On Debian or Ubuntu:

```bash
sudo apt-get update
sudo apt-get install libreoffice-draw
soffice --version
```

On macOS, install LibreOffice and ensure the `soffice` binary is available on `PATH`.

Before treating figure output as valid, run a sample conversion and check that generated Markdown references `.png` files rather than `.emf`, `.wmf`, `.vsd`, or `.vsdx` assets.

## Repository Orientation

When inside the repository, read `AGENTS.md` and `README.md` first. The main code paths are:

- `src/main.rs`: CLI argument parsing.
- `src/lib.rs`: small public API and module surface.
- `src/convert.rs`, `src/docx.rs`, `src/word.rs`: conversion orchestration and DOCX parsing.
- `src/render.rs`, `src/markdown.rs`: Markdown rendering helpers.
- `src/figure.rs`: figure conversion to PNG and margin trimming.
- `src/job.rs`, `src/types.rs`: output planning and shared report types.
- `tests/cli.rs`: integration tests and conversion fixtures.
- `proposal/`: local sample DOCX inputs.
- `tools/verify_public_3gpp_samples.sh`: public 3GPP corpus smoke test.

Keep `src/lib.rs` small. Put parsing, rendering, figure conversion, and filesystem planning in separate modules.

## Conversion Workflow

Use these commands from the repository root:

```bash
cargo run -- proposal -o out --overwrite
cargo run -- path/to/input.docx -o out --overwrite
cargo run -- path/to/docx-directory -o out --overwrite
```

Expected output includes Markdown files, per-document report JSON files, and asset directories such as `out/<proposal>_assets/`. Converted figures should be referenced as PNG from Markdown.

For release binaries:

```bash
cargo build --release
target/release/proposal2md proposal -o out --overwrite
```

## Figure Handling Rules

Do not silently drop embedded figures. If LibreOffice can convert an EMF, WMF, VSD, or VSDX asset, Markdown should reference the generated PNG and the original Office/vector asset does not need to be kept in the final asset directory.

If conversion fails or `soffice` is unavailable, preserve enough information for diagnosis and report a warning. Check:

```bash
soffice --version
rg "Unsupported figure|\\.emf|\\.wmf|\\.vsd|\\.vsdx" out
```

PNG margins matter. When changing figure conversion, inspect generated PNG dimensions and Markdown previews, and preserve tests that cover whitespace trimming.

## Verification

For normal code changes, run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- proposal -o out --overwrite
```

For release or conversion-behavior changes, also run:

```bash
tools/verify_public_3gpp_samples.sh
```

This downloads official 3GPP samples into `/tmp`, converts them, and checks that unsupported figure placeholders and EMF/Visio references do not remain in generated Markdown.

You can also run the bundled helper:

```bash
skills/proposal2md/scripts/verify_skill.sh
skills/proposal2md/scripts/verify_repo.sh
skills/proposal2md/scripts/verify_repo.sh --public-corpus
```

`verify_skill.sh` validates the skill package. `verify_repo.sh` validates the Rust project and requires a LibreOffice installation that can actually import the sample EMF files; if it fails with `javaldx`, `dconf`, or unsupported figure warnings, diagnose LibreOffice before treating conversion output as good.

## Debugging Checklist

Start with the report JSON beside the generated Markdown. Confirm whether warnings are from missing `soffice`, failed conversion, unsupported DOCX relationships, or bad output paths.

Then inspect the Markdown and asset directory:

```bash
rg "Unsupported figure|warning|\\.emf|\\.wmf|\\.vsd|\\.vsdx" out
find out -maxdepth 2 -type f
```

Do not commit generated `out/` files, release binaries, or public corpus downloads. Add or update tests for malformed input, media fallback, figure conversion, and PNG trimming when changing behavior.
