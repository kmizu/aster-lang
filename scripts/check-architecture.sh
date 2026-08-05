#!/usr/bin/env bash
set -euo pipefail

repo_root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}

python3 - "$repo_root" <<'PY'
import json
import pathlib
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
manifest = root / "Cargo.toml"
result = subprocess.run(
    [
        "cargo",
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
        str(manifest),
    ],
    check=False,
    capture_output=True,
    text=True,
)
if result.returncode != 0:
    sys.stderr.write("architecture check could not read cargo metadata:\n")
    sys.stderr.write(result.stderr)
    raise SystemExit(1)

metadata = json.loads(result.stdout)
rank = {
    "aster-diagnostics": 0,
    "aster-syntax": 1,
    "aster-semantics": 2,
    "aster-ir": 3,
    "aster-runtime": 4,
    "aster-cli": 5,
}
errors = []
workspace_members = set(metadata["workspace_members"])
for package in metadata["packages"]:
    if package["id"] not in workspace_members or package["name"] not in rank:
        continue
    source_rank = rank[package["name"]]
    for dependency in package["dependencies"]:
        target = dependency["name"]
        if target in rank and rank[target] >= source_rank:
            errors.append(
                f"forbidden dependency: {package['name']} -> {target}; "
                "dependencies must point toward a lower ASTER layer"
            )

if errors:
    for error in sorted(errors):
        print(error, file=sys.stderr)
    raise SystemExit(1)

print("architecture dependency direction is valid")
PY
