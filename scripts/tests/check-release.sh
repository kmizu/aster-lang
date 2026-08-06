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
version = "0.2.0"
EOF

cat >"$fixture_root/LICENSE" <<'EOF'
Apache License
Version 2.0, January 2004
EOF

cat >"$fixture_root/README.md" <<'EOF'
# ASTER

Release v0.2.0 uses SHA256SUMS.
EOF

cat >"$fixture_root/docs/releases/v0.2.0.md" <<'EOF'
# ASTER v0.2.0

Experimental fixture-backed reference processor.
EOF

cat >"$fixture_root/site/index.html" <<'EOF'
<a href="aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz">Linux</a>
<a href="aster-v0.2.0-aarch64-apple-darwin.tar.gz">macOS Apple</a>
<a href="aster-v0.2.0-x86_64-apple-darwin.tar.gz">macOS Intel</a>
<a href="aster-v0.2.0-x86_64-pc-windows-msvc.zip">Windows</a>
<a href="SHA256SUMS">SHA256SUMS</a>
EOF

cat >"$fixture_root/.github/workflows/release.yml" <<'EOF'
name: Release
on:
  push:
    tags: ["v*"]
  workflow_dispatch:
    inputs:
      tag:
        required: true
env:
  RELEASE_TAG: ${{ inputs.tag || github.ref_name }}
permissions:
  contents: read
jobs:
  build:
    strategy:
      matrix:
        include:
          - runner: ubuntu-24.04
            target: x86_64-unknown-linux-musl
            binary: aster
            format: tar.gz
          - runner: macos-15
            target: aarch64-apple-darwin
            binary: aster
            format: tar.gz
          - runner: macos-15-intel
            target: x86_64-apple-darwin
            binary: aster
            format: tar.gz
          - runner: windows-2025
            target: x86_64-pc-windows-msvc
            binary: aster.exe
            format: zip
    steps:
      - uses: actions/checkout@v6
        with:
          ref: refs/tags/${{ env.RELEASE_TAG }}
      - run: git checkout-index --force --all
      - run: cargo test --workspace --all-features
      - run: aster.exe --version
      - run: aster.exe check examples/meeting-scheduler/main.aster
      - run: aster.exe check examples/governed-note/main.aster
      - id: release-metadata
        run: |
          version=$(awk '/^version = / { gsub(/"/, "", $3); print $3; exit }' Cargo.toml)
          bundle="aster-v${version}-${{ matrix.target }}"
          case "${{ matrix.format }}" in
            tar.gz) asset="${bundle}.tar.gz" ;;
            zip) asset="${bundle}.zip" ;;
          esac
          echo "bundle=$bundle" >> "$GITHUB_OUTPUT"
          echo "asset=$asset" >> "$GITHUB_OUTPUT"
      - uses: actions/upload-artifact@v4
        with:
          name: ${{ steps.release-metadata.outputs.bundle }}
          path: dist/${{ steps.release-metadata.outputs.asset }}
  publish:
    permissions:
      contents: write
    steps:
      - uses: actions/download-artifact@v4
      - run: |
          version=$(awk '/^version = / { gsub(/"/, "", $3); print $3; exit }' Cargo.toml)
          expected=(
            "aster-v${version}-aarch64-apple-darwin.tar.gz"
            "aster-v${version}-x86_64-apple-darwin.tar.gz"
            "aster-v${version}-x86_64-pc-windows-msvc.zip"
            "aster-v${version}-x86_64-unknown-linux-musl.tar.gz"
          )
          windows="dist/aster-v${version}-x86_64-pc-windows-msvc.zip"
          notes="docs/releases/v${version}.md"
          sha256sum --check SHA256SUMS
          gh release create "$RELEASE_TAG" dist/* --notes-file "$notes"
EOF

cat >"$fixture_root/.github/workflows/ci.yml" <<'EOF'
name: CI
permissions:
  contents: read
jobs:
  check:
    steps:
      - run: sudo apt-get install --yes ripgrep
EOF

cat >"$fixture_root/.github/workflows/pages.yml" <<'EOF'
name: Pages
permissions:
  contents: read
jobs:
  build:
    steps:
      - uses: actions/configure-pages@v5
      - uses: actions/upload-pages-artifact@v4
  deploy:
    needs: build
    permissions:
      pages: write
      id-token: write
    steps:
      - uses: actions/deploy-pages@v4
EOF

"$checker" "$fixture_root"

nonrecoverable_root="$fixture_root-nonrecoverable"
cp -R "$fixture_root" "$nonrecoverable_root"
sed -i '/workflow_dispatch:/d' "$nonrecoverable_root/.github/workflows/release.yml"

set +e
nonrecoverable_output=$("$checker" "$nonrecoverable_root" 2>&1)
nonrecoverable_status=$?
set -e

if [[ $nonrecoverable_status -eq 0 ]]; then
  echo "release checker accepted a workflow without tag recovery dispatch" >&2
  exit 1
fi

nonrecoverable_expected="missing release recovery contract: workflow_dispatch"
if [[ "$nonrecoverable_output" != *"$nonrecoverable_expected"* ]]; then
  echo "release checker did not identify the missing recovery dispatch" >&2
  echo "$nonrecoverable_output" >&2
  exit 1
fi

unsafe_root="$fixture_root-unsafe"
cp -R "$fixture_root" "$unsafe_root"
trap 'rm -rf "$fixture_root" "$nonrecoverable_root" "$unsafe_root"' EXIT
cat >>"$unsafe_root/.github/workflows/release.yml" <<'EOF'
pull_request_target:
EOF

set +e
unsafe_output=$("$checker" "$unsafe_root" 2>&1)
unsafe_status=$?
set -e

if [[ $unsafe_status -eq 0 ]]; then
  echo "release checker accepted pull_request_target" >&2
  exit 1
fi

unsafe_expected="forbidden release workflow trigger: pull_request_target"
if [[ "$unsafe_output" != *"$unsafe_expected"* ]]; then
  echo "release checker did not identify the unsafe workflow trigger" >&2
  echo "$unsafe_output" >&2
  exit 1
fi

sed -i 's/version = "0.2.0"/version = "0.2.1"/' "$fixture_root/Cargo.toml"

set +e
output=$("$checker" "$fixture_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "release checker accepted the wrong workspace version" >&2
  exit 1
fi

expected="release version drift: workspace 0.2.1 requires docs/releases/v0.2.1.md"
if [[ "$output" != *"$expected"* ]]; then
  echo "release checker did not report the version mismatch" >&2
  echo "$output" >&2
  exit 1
fi

echo "release contract checker accepts the exact contract and rejects version drift"
