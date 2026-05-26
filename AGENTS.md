# Repository Guidelines

## Project Structure & Module Organization

This repository currently contains a minimal root with `README.md` and no committed crate files yet. When implementation is added, use the standard Rust layout:

- `Cargo.toml` for package metadata and dependencies.
- `src/main.rs` for a CLI entry point, or `src/lib.rs` for reusable library logic.
- `src/bin/` for additional command binaries.
- `tests/` for integration tests.
- `fixtures/` or `testdata/` for sample proposal inputs and expected Markdown outputs.

Keep parsing, conversion, and output formatting in separate modules where practical so behavior can be tested without invoking the CLI.

## Build, Test, and Development Commands

Use Cargo commands once `Cargo.toml` exists:

- `cargo build` compiles the project in debug mode.
- `cargo run -- <args>` runs the local CLI with arguments.
- `cargo test` runs unit and integration tests.
- `cargo fmt --check` verifies Rust formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` runs lint checks with warnings treated as errors.

If a command requires sample files, place them under `fixtures/` and reference them with relative paths.

## Coding Style & Naming Conventions

Follow idiomatic Rust formatting with `rustfmt`; use four-space indentation and keep lines readable. Use `snake_case` for functions, modules, variables, and test names; `PascalCase` for types and traits; and `SCREAMING_SNAKE_CASE` for constants.

Prefer small, explicit functions over broad utility modules. Return `Result` from fallible operations and preserve useful error context when reading files, parsing proposals, or writing Markdown.

## Testing Guidelines

Use Rust's built-in test framework. Put focused unit tests next to the code they exercise with `#[cfg(test)]`, and use `tests/` for end-to-end CLI or conversion behavior. Name tests after the expected behavior, for example `converts_heading_blocks_to_markdown`.

Include fixture-based tests for representative proposal inputs, edge cases, and malformed input. Run `cargo test` before opening a pull request.

## Commit & Pull Request Guidelines

The current history only shows an initial commit, so no project-specific commit convention is established. Use concise, imperative commit subjects such as `Add proposal parser` or `Handle empty sections`.

Pull requests should include a short description, the commands run for verification, and linked issues when applicable. For output-format changes, include a before/after Markdown example or reference the updated fixture.

## Agent-Specific Instructions

Keep generated changes narrow and avoid introducing build systems or dependencies without a clear project need. Do not modify unrelated files when adding documentation, tests, or implementation.
