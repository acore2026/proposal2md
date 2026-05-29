#!/usr/bin/env bash
set -euo pipefail

run_public_corpus=0
if [[ "${1:-}" == "--public-corpus" ]]; then
  run_public_corpus=1
elif [[ $# -gt 0 ]]; then
  echo "usage: $0 [--public-corpus]" >&2
  exit 2
fi

export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/tmp/proposal2md-lo-runtime}"
export XDG_CACHE_HOME="${XDG_CACHE_HOME:-/tmp/proposal2md-lo-cache}"
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CACHE_HOME"
chmod 700 "$XDG_RUNTIME_DIR"

cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
cargo run -- proposal -o out --overwrite

if [[ "$run_public_corpus" -eq 1 ]]; then
  tools/verify_public_3gpp_samples.sh
fi
