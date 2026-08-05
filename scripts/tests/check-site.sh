#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
checker="$repo_root/scripts/check-site.sh"

fixture_root=$(mktemp -d)
trap 'rm -rf "$fixture_root"' EXIT
valid_site="$fixture_root/valid"
invalid_site="$fixture_root/invalid"
mkdir -p "$valid_site" "$invalid_site"

cat >"$valid_site/index.html" <<'EOF'
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ASTER v0.1.0</title>
  <link rel="stylesheet" href="styles.css">
  <script src="site.js" defer></script>
</head>
<body>
  <a class="skip-link" href="#main-content">Skip to content</a>
  <header>ASTER</header>
  <main id="main-content">
    <section id="hero"><h1>Authority before action.</h1></section>
    <section id="boundary">Candidate Proposal Permit Reconciliation</section>
    <section id="evidence">driver calls 0</section>
    <section id="download">
      <a href="aster-v0.1.0-x86_64-unknown-linux-musl.tar.gz">Linux</a>
      <a href="aster-v0.1.0-aarch64-apple-darwin.tar.gz">macOS Apple</a>
      <a href="aster-v0.1.0-x86_64-apple-darwin.tar.gz">macOS Intel</a>
      <a href="aster-v0.1.0-x86_64-pc-windows-msvc.zip">Windows</a>
      <a href="SHA256SUMS">SHA256SUMS</a>
    </section>
    <section id="docs">Specification</section>
  </main>
  <footer>Experimental fixture-backed reference processor</footer>
</body>
</html>
EOF

cat >"$valid_site/styles.css" <<'EOF'
:focus-visible { outline: 2px solid #ffc247; }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation: none; } }
EOF

cat >"$valid_site/site.js" <<'EOF'
document.documentElement.classList.add("is-ready");
if ("IntersectionObserver" in window) {
  new IntersectionObserver(() => {}).observe(document.body);
}
EOF

cat >"$valid_site/404.html" <<'EOF'
<!doctype html>
<html lang="en"><body><main><h1>ASTER / 404</h1><a href="./">Return home</a></main></body></html>
EOF

"$checker" "$valid_site"

cp -R "$valid_site/." "$invalid_site/"
cat >>"$invalid_site/index.html" <<'EOF'
<script src="https://cdn.example.test/site.js"></script>
EOF

set +e
output=$("$checker" "$invalid_site" 2>&1)
status=$?
set -e

if [[ $status -eq 0 ]]; then
  echo "site checker accepted an external script dependency" >&2
  exit 1
fi

expected="external site dependency is forbidden"
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify the external dependency" >&2
  echo "$output" >&2
  exit 1
fi

echo "site checker accepts the static contract and rejects external dependencies"
