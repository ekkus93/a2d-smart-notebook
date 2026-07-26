#!/usr/bin/env bash
# PostToolUse(Write|Edit): format edited Rust files with rustfmt.
# Runs standalone rustfmt (not `cargo fmt`) so it works before the workspace exists.
# Edition is read from the nearest Cargo.toml walking up from the edited file.
set -uo pipefail

f=$(jq -r '.tool_response.filePath // .tool_input.file_path // empty' 2>/dev/null)
[[ -n "$f" && "$f" == *.rs && -f "$f" ]] || exit 0
command -v rustfmt >/dev/null 2>&1 || exit 0

edition=""
dir=$(cd "$(dirname "$f")" 2>/dev/null && pwd) || exit 0
while [[ -n "$dir" && "$dir" != "/" ]]; do
  if [[ -f "$dir/Cargo.toml" ]]; then
    edition=$(grep -m1 -E '^[[:space:]]*edition[[:space:]]*=' "$dir/Cargo.toml" \
      | grep -oE '[0-9]{4}') || true
    [[ -n "$edition" ]] && break
  fi
  dir=$(dirname "$dir")
done

rustfmt --edition "${edition:-2021}" "$f"
