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
  .github/workflows/ci.yml \
  .github/workflows/pages.yml \
  .github/workflows/release.yml; do
  require_file "$relative"
done

workflow_files=(
  "$repo_root/.github/workflows/ci.yml"
  "$repo_root/.github/workflows/pages.yml"
  "$repo_root/.github/workflows/release.yml"
)
if rg --fixed-strings --quiet -- 'pull_request_target' "${workflow_files[@]}"; then
  fail "forbidden release workflow trigger: pull_request_target"
fi
if ! rg --fixed-strings --quiet -- 'workflow_dispatch:' \
  "$repo_root/.github/workflows/release.yml"; then
  fail "missing release recovery contract: workflow_dispatch"
fi

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

require_text .github/workflows/ci.yml 'contents: read'
require_text .github/workflows/ci.yml 'ripgrep'

for contract in \
  'actions/configure-pages@v5' \
  'actions/upload-pages-artifact@v4' \
  'actions/deploy-pages@v4' \
  'needs: build' \
  'pages: write' \
  'id-token: write'; do
  require_text .github/workflows/pages.yml "$contract"
done

for contract in \
  'tags:' \
  '"v*"' \
  'ubuntu-24.04' \
  'x86_64-unknown-linux-musl' \
  'macos-15' \
  'aarch64-apple-darwin' \
  'macos-15-intel' \
  'x86_64-apple-darwin' \
  'windows-2025' \
  'x86_64-pc-windows-msvc' \
  'actions/upload-artifact@v4' \
  'actions/download-artifact@v4' \
  'contents: write' \
  'RELEASE_TAG' \
  'refs/tags/${{ env.RELEASE_TAG }}' \
  'git checkout-index --force --all' \
  'aster.exe --version' \
  'aster.exe check examples/meeting-scheduler/main.aster' \
  'sha256sum --check SHA256SUMS' \
  'gh release create'; do
  require_text .github/workflows/release.yml "$contract"
done

echo "release contract is coherent for $expected_tag"
