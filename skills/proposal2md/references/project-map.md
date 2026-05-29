# proposal2md Project Map

Use this reference when you need a quick map of where behavior belongs.

- CLI parsing: `src/main.rs`
- Public crate surface: `src/lib.rs`
- Conversion orchestration: `src/convert.rs`
- DOCX package reading and relationship lookup: `src/docx.rs`
- WordprocessingML traversal: `src/word.rs`
- Markdown rendering: `src/render.rs` and `src/markdown.rs`
- Figure conversion, PNG generation, and trimming: `src/figure.rs`
- Output path planning: `src/job.rs`
- Shared report and option types: `src/types.rs`
- Integration tests: `tests/cli.rs`
- Local sample inputs: `proposal/`
- Public corpus verifier: `tools/verify_public_3gpp_samples.sh`

Behavioral invariant: generated Markdown should be useful even when figure conversion fails. Successful EMF, WMF, VSD, and VSDX conversions should produce PNG references, not Markdown references to the original Office/vector assets.
