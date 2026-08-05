#!/usr/bin/env bash
set -euo pipefail

allow_active=false
if [[ ${1:-} == "--allow-active-bootstrap" ]]; then
  allow_active=true
  shift
fi
repo_root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}

python3 - "$repo_root" "$allow_active" <<'PY'
import pathlib
import re
import sys

root = pathlib.Path(sys.argv[1]).resolve()
allow_active = sys.argv[2] == "true"
required = [
    "AGENTS.md",
    "README.md",
    "ARCHITECTURE.md",
    "docs/spec/aster-0.1.md",
    "docs/design-docs/core-beliefs.md",
    "docs/design-docs/runtime-and-replay.md",
    "docs/design-docs/security-model.md",
    "docs/design-docs/diagnostics.md",
    "docs/adr/0001-rust-workspace-and-layering.md",
    "docs/adr/0002-explicit-effect-machine.md",
    "docs/adr/0003-trace-canonicalization.md",
]
errors = []
for relative in required:
    if not (root / relative).is_file():
        errors.append(f"missing required document: {relative}")

for command in (
    "scripts/check.sh",
    "scripts/check-architecture.sh",
    "scripts/check-docs.sh",
    "scripts/check-production-rust.sh",
):
    path = root / command
    if not path.is_file():
        errors.append(f"missing documented command: {command}")

link_pattern = re.compile(r"\[[^\]]+\]\(([^)]+)\)")
for relative in ("AGENTS.md", "README.md", "ARCHITECTURE.md"):
    document = root / relative
    if not document.is_file():
        continue
    text = document.read_text(encoding="utf-8")
    for target in link_pattern.findall(text):
        target = target.strip().split("#", 1)[0]
        if not target or "://" in target or target.startswith("mailto:"):
            continue
        resolved = (document.parent / target).resolve()
        try:
            resolved.relative_to(root)
        except ValueError:
            errors.append(f"link escapes repository in {relative}: {target}")
            continue
        if not resolved.exists():
            errors.append(f"broken link in {relative}: {target}")

registry = root / "crates/aster-diagnostics/src/registry.rs"
reference = root / "docs/design-docs/diagnostics.md"
if registry.is_file() and reference.is_file():
    registered = sorted(
        set(re.findall(r'"(ASTER-[A-Z]+-[0-9]{4})"\s*=>', registry.read_text(encoding="utf-8")))
    )
    reference_text = reference.read_text(encoding="utf-8")
    for code in registered:
        if code not in reference_text:
            errors.append(f"registered diagnostic missing from reference: {code}")

active_plan = root / "docs/exec-plans/active/bootstrap-aster-0.1.md"
if active_plan.exists() and not allow_active:
    errors.append(
        "active bootstrap plan remains: docs/exec-plans/active/bootstrap-aster-0.1.md"
    )

if errors:
    for error in sorted(errors):
        print(error, file=sys.stderr)
    raise SystemExit(1)

print("documentation structure and links are valid")
PY

if [[ "$allow_active" == false ]]; then
  while IFS= read -r example; do
    cargo run --quiet --manifest-path "$repo_root/Cargo.toml" \
      -p aster-cli --bin aster -- fmt "$example" --check
  done < <(find "$repo_root/examples" -type f -name '*.aster' -print | sort)
fi
