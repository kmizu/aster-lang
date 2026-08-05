#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
checker="$repo_root/scripts/check-architecture.sh"

"$checker" "$repo_root"

fixture_root=$(mktemp -d)
trap 'rm -rf "$fixture_root"' EXIT

mkdir -p "$fixture_root/crates/aster-diagnostics/src" "$fixture_root/crates/aster-syntax/src"
cp "$checker" "$fixture_root/check-architecture.sh"

cat >"$fixture_root/Cargo.toml" <<'EOF'
[workspace]
members = ["crates/aster-diagnostics", "crates/aster-syntax"]
resolver = "2"
EOF
cat >"$fixture_root/crates/aster-diagnostics/Cargo.toml" <<'EOF'
[package]
name = "aster-diagnostics"
version = "0.1.0"
edition = "2024"

[dependencies]
aster-syntax = { path = "../aster-syntax" }
EOF
cat >"$fixture_root/crates/aster-diagnostics/src/lib.rs" <<'EOF'
pub fn invalid_upward_dependency() {}
EOF
cat >"$fixture_root/crates/aster-syntax/Cargo.toml" <<'EOF'
[package]
name = "aster-syntax"
version = "0.1.0"
edition = "2024"
EOF
cat >"$fixture_root/crates/aster-syntax/src/lib.rs" <<'EOF'
pub fn syntax() {}
EOF

set +e
output=$("$fixture_root/check-architecture.sh" "$fixture_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "architecture checker accepted a forbidden upward dependency" >&2
  exit 1
fi

expected="forbidden dependency: aster-diagnostics -> aster-syntax"
if [[ "$output" != *"$expected"* ]]; then
  echo "architecture checker did not explain the forbidden edge" >&2
  echo "$output" >&2
  exit 1
fi
