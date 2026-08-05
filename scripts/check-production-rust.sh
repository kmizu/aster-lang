#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}
source_root="$repo_root/crates"

if [[ ! -d "$source_root" ]]; then
  echo "production Rust source root does not exist: $source_root" >&2
  exit 1
fi

source_dirs=("$source_root"/*/src)
if [[ ! -d ${source_dirs[0]} ]]; then
  echo "no production Rust source directories found under: $source_root" >&2
  exit 1
fi

set +e
matches=$(rg --line-number --glob '*.rs' \
  --pcre2 '\.(?:unwrap|expect)\s*\(|\b(?:panic|todo|unimplemented)!\s*\(' \
  "${source_dirs[@]}" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "forbidden panic/placeholder API in production Rust:" >&2
  echo "$matches" >&2
  exit 1
fi
if [[ $status -ne 1 ]]; then
  echo "production Rust scan failed:" >&2
  echo "$matches" >&2
  exit 1
fi

echo "production Rust contains no forbidden panic or placeholder APIs"
