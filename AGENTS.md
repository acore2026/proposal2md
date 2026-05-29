# Repository Guidelines

## Project Structure & Module Organization

This repository is a Rust CLI crate for converting 3GPP DOCX proposals to Markdown.

- `src/main.rs` contains CLI argument parsing.
- `src/lib.rs` exposes the public API and module surface.
- `src/convert.rs`, `src/render.rs`, `src/docx.rs`, `src/word.rs`, and `src/figure.rs` contain the conversion pipeline.
- `src/job.rs`, `src/markdown.rs`, and `src/types.rs` contain output planning, Markdown helpers, and shared types.
- `tests/cli.rs` contains integration tests with synthetic DOCX fixtures and sample proposal smoke tests.
- `proposal/` contains local sample DOCX inputs.
- `tools/verify_public_3gpp_samples.sh` downloads and verifies a public 3GPP corpus.

Keep parsing, rendering, figure conversion, and filesystem/output planning in separate modules. Avoid growing `src/lib.rs`; it should remain a small public surface.

## Build, Test, and Development Commands

- `cargo build` compiles the project in debug mode.
- `cargo run -- proposal -o out --overwrite` converts the local sample proposals.
- `cargo test` runs unit and integration tests.
- `cargo fmt --check` verifies Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` runs lint checks with warnings treated as errors.
- `tools/verify_public_3gpp_samples.sh` downloads 10 official 3GPP sample ZIPs into `/tmp` and verifies conversion output.

LibreOffice Draw (`soffice`) is required to convert EMF/WMF/VSD/VSDX figures to PNG. Without it, the converter should preserve originals and report warnings instead of silently losing figures.

## Coding Style & Naming Conventions

Follow idiomatic Rust formatting with `rustfmt`; use four-space indentation and keep lines readable. Use `snake_case` for functions, modules, variables, and test names; `PascalCase` for types and traits; and `SCREAMING_SNAKE_CASE` for constants.

Prefer small, explicit functions over broad utility modules. Return `Result` from fallible operations and preserve useful error context when reading files, parsing proposals, or writing Markdown.

## Testing Guidelines

Use Rust's built-in test framework. Put focused unit tests next to the code they exercise with `#[cfg(test)]`, and use `tests/` for end-to-end CLI or conversion behavior. Name tests after the expected behavior, for example `converts_heading_blocks_to_markdown`.

Include fixture-based tests for representative proposal inputs, edge cases, malformed input, media conversion fallback, and PNG trimming. Run `cargo test` before opening a pull request. Run the public corpus script before releases or conversion-behavior changes.

## Commit & Pull Request Guidelines

Use concise, imperative commit subjects such as `Refactor renderer modules` or `Trim converted figure margins`.

Pull requests should include a short description, the commands run for verification, and linked issues when applicable. For output-format changes, include a before/after Markdown example or reference the updated fixture.

## Agent-Specific Instructions

Keep generated changes narrow and avoid introducing build systems or dependencies without a clear project need. Do not commit generated `out/` files or temporary public corpus downloads from `/tmp`. Do not modify unrelated files when adding documentation, tests, or implementation.
