#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}
expected_version="0.1.0"
expected_tag="v${expected_version}"

fail() {
  echo "$1" >&2
  exit 1
}

require_file() {
  local relative=$1
  [[ -f "$repo_root/$relative" ]] || fail "missing release file: $relative"
}

require_text() {
  local relative=$1
  local text=$2
  rg --fixed-strings --quiet -- "$text" "$repo_root/$relative" ||
    fail "missing release contract text in $relative: $text"
}

for relative in \
  Cargo.toml \
  LICENSE \
  README.md \
  docs/releases/v0.1.0.md \
  site/index.html \
  .github/workflows/release.yml; do
  require_file "$relative"
done

workspace_version=$(
  awk '
    /^\[workspace\.package\][[:space:]]*$/ { in_workspace_package = 1; next }
    /^\[/ { in_workspace_package = 0 }
    in_workspace_package && /^[[:space:]]*version[[:space:]]*=/ {
      value = $0
      sub(/^[^=]*=[[:space:]]*"/, "", value)
      sub(/"[[:space:]]*$/, "", value)
      print value
      exit
    }
  ' "$repo_root/Cargo.toml"
)

[[ -n "$workspace_version" ]] || fail "workspace package version is missing"
[[ "$workspace_version" == "$expected_version" ]] ||
  fail "release version mismatch: expected $expected_version, found $workspace_version"

assets=(
  "aster-${expected_tag}-x86_64-unknown-linux-musl.tar.gz"
  "aster-${expected_tag}-aarch64-apple-darwin.tar.gz"
  "aster-${expected_tag}-x86_64-apple-darwin.tar.gz"
  "aster-${expected_tag}-x86_64-pc-windows-msvc.zip"
)

for asset in "${assets[@]}"; do
  require_text site/index.html "$asset"
  require_text .github/workflows/release.yml "$asset"
done

require_text README.md "$expected_tag"
require_text docs/releases/v0.1.0.md "$expected_tag"
require_text site/index.html "SHA256SUMS"
require_text .github/workflows/release.yml "SHA256SUMS"
require_text README.md "SHA256SUMS"
require_text LICENSE "Apache License"
require_text LICENSE "Version 2.0, January 2004"

echo "release contract is coherent for $expected_tag"
