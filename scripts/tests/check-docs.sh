#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
checker="$repo_root/scripts/check-docs.sh"

"$checker" --allow-active-bootstrap "$repo_root"

fixture_root=$(mktemp -d)
trap 'rm -rf "$fixture_root"' EXIT
mkdir -p "$fixture_root"

set +e
output=$("$checker" --allow-active-bootstrap "$fixture_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "documentation checker accepted a repository with missing documents" >&2
  exit 1
fi

expected="missing required document: README.md"
if [[ "$output" != *"$expected"* ]]; then
  echo "documentation checker did not identify the missing document" >&2
  echo "$output" >&2
  exit 1
fi

contract_root="$fixture_root/contract"
mkdir -p "$contract_root/crates/aster-diagnostics/src"
cp "$repo_root/AGENTS.md" "$repo_root/README.md" "$repo_root/ARCHITECTURE.md" \
  "$contract_root/"
cp -R "$repo_root/docs" "$repo_root/examples" "$repo_root/scripts" \
  "$contract_root/"
cp "$repo_root/crates/aster-diagnostics/src/registry.rs" \
  "$contract_root/crates/aster-diagnostics/src/registry.rs"

python3 - "$contract_root/docs/spec/aster-host-protocol-0.2.md" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
path.write_text(text.replace("driver-free replay", "offline replay"), encoding="utf-8")
PY

set +e
output=$($contract_root/scripts/check-docs.sh --allow-active-bootstrap "$contract_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "documentation checker accepted an incomplete host protocol contract" >&2
  exit 1
fi

expected="normative host protocol missing required term: driver-free replay"
if [[ "$output" != *"$expected"* ]]; then
  echo "documentation checker did not identify the missing host protocol term" >&2
  echo "$output" >&2
  exit 1
fi
