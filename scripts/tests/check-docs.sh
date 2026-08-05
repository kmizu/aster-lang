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
