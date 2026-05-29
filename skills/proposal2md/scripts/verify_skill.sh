#!/usr/bin/env bash
set -euo pipefail

skill_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
validator="${SKILL_CREATOR_VALIDATOR:-/root/.codex/skills/.system/skill-creator/scripts/quick_validate.py}"

test -f "$skill_dir/SKILL.md"
test -f "$skill_dir/agents/openai.yaml"
test -x "$skill_dir/scripts/verify_repo.sh"
test -f "$skill_dir/references/project-map.md"

if [[ -f "$validator" ]]; then
  python3 "$validator" "$skill_dir"
else
  echo "warning: skill validator not found at $validator; structural checks passed" >&2
fi
