#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
checker="$repo_root/scripts/check-production-rust.sh"

"$checker" "$repo_root"

fixture_root=$(mktemp -d)
trap 'rm -rf "$fixture_root"' EXIT
mkdir -p "$fixture_root/crates/example/src" "$fixture_root/crates/example/tests"

cat >"$fixture_root/crates/example/src/lib.rs" <<'EOF'
pub fn bad(input: Option<u8>) -> u8 {
    input.expect("reachable from user input")
}
EOF
cat >"$fixture_root/crates/example/tests/allowed.rs" <<'EOF'
#[test]
fn tests_may_use_expect() {
    assert_eq!(Some(1).expect("fixture"), 1);
}
EOF

set +e
output=$("$checker" "$fixture_root" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "production Rust checker accepted expect() in library code" >&2
  exit 1
fi
if [[ "$output" != *"crates/example/src/lib.rs"* ]]; then
  echo "production Rust checker did not identify the unsafe production file" >&2
  echo "$output" >&2
  exit 1
fi
