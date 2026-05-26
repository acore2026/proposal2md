#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK_DIR=${PROPOSAL2MD_PUBLIC_CORPUS:-/tmp/proposal2md-3gpp-corpus}
OUT_DIR=${PROPOSAL2MD_PUBLIC_OUT:-/tmp/proposal2md-3gpp-out}

URLS=(
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_173_Goa_2026-02/Docs/S2-2600001.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_173_Goa_2026-02/Docs/S2-2600043.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_173_Goa_2026-02/Docs/S2-2600434.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_174_Malta_2026-04/Docs/S2-2601720.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_174_Malta_2026-04/Docs/S2-2602109.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_174_Malta_2026-04/Docs/S2-2602500.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_175_Dalian_2026-05/Docs/S2-2603538.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_175_Dalian_2026-05/Docs/S2-2603600.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_175_Dalian_2026-05/Docs/S2-2603800.zip"
  "https://www.3gpp.org/ftp/tsg_sa/WG2_Arch/TSGS2_175_Dalian_2026-05/Docs/S2-2604000.zip"
)

require() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require curl
require unzip
require python3

rm -rf "$WORK_DIR" "$OUT_DIR"
mkdir -p "$WORK_DIR/zips" "$WORK_DIR/docx"

for url in "${URLS[@]}"; do
  name=${url##*/}
  echo "download $name"
  curl -fL -sS "$url" -o "$WORK_DIR/zips/$name"
  unzip -q -o "$WORK_DIR/zips/$name" -d "$WORK_DIR/docx"
done

cargo run --manifest-path "$ROOT_DIR/Cargo.toml" -- "$WORK_DIR/docx" -o "$OUT_DIR" --overwrite

python3 - "$OUT_DIR" <<'PY'
import json
import pathlib
import sys

out_dir = pathlib.Path(sys.argv[1])
reports = sorted(out_dir.glob("*.report.json"))
if len(reports) != 10:
    raise SystemExit(f"expected 10 reports, found {len(reports)}")

unsupported = []
warnings = []
for report_path in reports:
    report = json.load(open(report_path))
    if report["unsupported_assets"]:
        unsupported.append((report_path.name, len(report["unsupported_assets"])))
    if report["warnings"]:
        warnings.append((report_path.name, report["warnings"]))
    print(
        f"{report_path.stem.replace('.report', '')}: "
        f"paragraphs={report['paragraph_count']} "
        f"tables={report['table_count']} "
        f"media={report['media_count']} "
        f"unsupported={len(report['unsupported_assets'])} "
        f"warnings={len(report['warnings'])}"
    )

if unsupported:
    raise SystemExit(f"unsupported assets found: {unsupported}")
if warnings:
    raise SystemExit(f"warnings found: {warnings}")

for markdown_path in out_dir.glob("*.md"):
    text = markdown_path.read_text(errors="replace")
    forbidden = ["Unsupported figure", ".emf", ".wmf", ".vsd", ".vsdx"]
    hits = [item for item in forbidden if item in text]
    if hits:
        raise SystemExit(f"{markdown_path.name} contains unsupported references: {hits}")

print(f"verified {len(reports)} public 3GPP samples")
print(f"png assets: {len(list(out_dir.glob('*_assets/*.png')))}")
PY
