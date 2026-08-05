#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
checker="$repo_root/scripts/check-release.sh"

fixture_root=$(mktemp -d)
trap 'rm -rf "$fixture_root"' EXIT
mkdir -p \
  "$fixture_root/.github/workflows" \
  "$fixture_root/docs/releases" \
  "$fixture_root/site"

cat >"$fixture_root/Cargo.toml" <<'EOF'
[workspace]
members = []

[workspace.package]
version = "0.1.0"
EOF

cat >"$fixture_root/LICENSE" <<'EOF'
Apache License
Version 2.0, January 2004
EOF

cat >"$fixture_root/README.md" <<'EOF'
# ASTER

Release v0.1.0 uses SHA256SUMS.
EOF

cat >"$fixture_root/docs/releases/v0.1.0.md" <<'EOF'
# ASTER v0.1.0

Experimental fixture-backed reference processor.
EOF

cat >"$fixture_root/site/index.html" <<'EOF'
<a href="aster-v0.1.0-x86_64-unknown-linux-musl.tar.gz">Linux</a>
<a href="aster-v0.1.0-aarch64-apple-darwin.tar.gz">macOS Apple</a>
<a href="aster-v0.1.0-x86_64-apple-darwin.tar.gz">macOS Intel</a>
<a href="aster-v0.1.0-x86_64-pc-windows-msvc.zip">Windows</a>
<a href="SHA256SUMS">SHA256SUMS</a>
EOF

cat >"$fixture_root/.github/workflows/release.yml" <<'EOF'
name: Release v0.1.0
assets:
  - aster-v0.1.0-x86_64-unknown-linux-musl.tar.gz
  - aster-v0.1.0-aarch64-apple-darwin.tar.gz
  - aster-v0.1.0-x86_64-apple-darwin.tar.gz
  - aster-v0.1.0-x86_64-pc-windows-msvc.zip
  - SHA256SUMS
EOF

"$checker" "$fixture_root"

sed -i 's/version = "0.1.0"/version = "0.1.1"/' "$fixture_root/Cargo.toml"

set +e
output=$("$checker" "$fixture_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "release checker accepted the wrong workspace version" >&2
  exit 1
fi

expected="release version mismatch: expected 0.1.0"
if [[ "$output" != *"$expected"* ]]; then
  echo "release checker did not report the version mismatch" >&2
  echo "$output" >&2
  exit 1
fi

echo "release contract checker accepts the exact contract and rejects version drift"
