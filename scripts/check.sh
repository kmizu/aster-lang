#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
cd "$repo_root"

cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
./scripts/tests/check-architecture.sh
./scripts/tests/check-docs.sh
./scripts/tests/check-production-rust.sh
./scripts/tests/check-site.sh
./scripts/tests/check-release.sh
./scripts/check-architecture.sh
./scripts/check-production-rust.sh
./scripts/check-docs.sh
./scripts/check-site.sh
./scripts/check-release.sh
