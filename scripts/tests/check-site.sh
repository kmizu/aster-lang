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
  <title>ASTER v0.2.0</title>
  <link rel="stylesheet" href="styles.css">
  <script src="site.js" defer></script>
</head>
<body>
  <a class="skip-link" href="#main-content">Skip to content</a>
  <header>ASTER</header>
  <main id="main-content">
    <section id="hero"><h1>Authority before action.</h1></section>
    <section id="why">Judgment without authority.</section>
    <section id="quickstart">
      <h2>Prove it in five minutes.</h2>
      <p>Fixture-backed record</p>
      <p>Driver-free replay</p>
      <p>34 trace entries</p>
    </section>
    <section id="evidence">
      <p>record and replay states match</p>
      <ol class="ledger">
        <li>
          <strong>effect_requested</strong>
          <small>Replay matches each regenerated request</small>
          <code>request match</code>
        </li>
        <li class="ledger-result">
          <strong>run_completed</strong>
          <small>Recorded and replayed final outcomes agree</small>
          <code>outcome match</code>
        </li>
      </ol>
      <p class="replay-property">Replay property: driver calls 0</p>
    </section>
    <section id="boundary">
      Candidate Proposal Permit Reconciliation
      The desired write immutably binds its action, arguments, intent, risk,
      capability request, and program identity.
    </section>
    <section id="protocol">
      effect_preview effect_admission execute_grant effect_resolution
      A preview is not authority.
      A conforming host must not execute before receiving the matching grant.
      A malicious host can act early, falsify provider behavior, or under-report usage by using authority it already has.
    </section>
    <section id="download">
      <a href="aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz">Linux</a>
      <a href="aster-v0.2.0-aarch64-apple-darwin.tar.gz">macOS Apple</a>
      <a href="aster-v0.2.0-x86_64-apple-darwin.tar.gz">macOS Intel</a>
      <a href="aster-v0.2.0-x86_64-pc-windows-msvc.zip">Windows</a>
      <a href="SHA256SUMS">SHA256SUMS</a>
    </section>
    <section id="docs">Specification</section>
  </main>
  <footer>Experimental fixture-backed reference processor</footer>
</body>
</html>
EOF

cat >"$valid_site/styles.css" <<'EOF'
:root {
  --paper: #eee9de;
  --amber-on-paper: #805500;
}
:focus-visible { outline: 2px solid #ffc247; }
.why .section-index,
.why .comparison-aster > span,
.why .comparison-aster strong { color: var(--amber-on-paper); }
.quickstart-step pre { scrollbar-color: #4a4d50 #07080a; scrollbar-width: thin; }
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

missing_protocol_site="$fixture_root/missing-protocol"
mkdir -p "$missing_protocol_site"
cp -R "$valid_site/." "$missing_protocol_site/"
sed -i 's/execute_grant/execution_grant/' "$missing_protocol_site/index.html"

set +e
output=$("$checker" "$missing_protocol_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted a page without execute_grant" >&2
  exit 1
fi

expected="missing site contract text in index.html: execute_grant"
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify the missing host grant" >&2
  echo "$output" >&2
  exit 1
fi

missing_conforming_host_site="$fixture_root/missing-conforming-host"
mkdir -p "$missing_conforming_host_site"
cp -R "$valid_site/." "$missing_conforming_host_site/"
sed -i \
  's/A conforming host must not execute before receiving the matching grant\./Only the matching grant permits execution./' \
  "$missing_conforming_host_site/index.html"

set +e
output=$("$checker" "$missing_conforming_host_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted an unconditional execute_grant claim" >&2
  exit 1
fi

expected="missing site contract text in index.html: A conforming host must not execute before receiving the matching grant."
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify the missing conforming-host obligation" >&2
  echo "$output" >&2
  exit 1
fi

missing_malicious_host_site="$fixture_root/missing-malicious-host"
mkdir -p "$missing_malicious_host_site"
cp -R "$valid_site/." "$missing_malicious_host_site/"
sed -i \
  's/A malicious host can act early, falsify provider behavior, or under-report/A malicious host remains outside the trust boundary and may under-report/' \
  "$missing_malicious_host_site/index.html"

set +e
output=$("$checker" "$missing_malicious_host_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted a page without explicit malicious-host risks" >&2
  exit 1
fi

expected="missing site contract text in index.html: A malicious host can act early, falsify provider behavior, or under-report usage by using authority it already has."
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify missing malicious-host risks" >&2
  echo "$output" >&2
  exit 1
fi

missing_scrollbar_site="$fixture_root/missing-scrollbar"
mkdir -p "$missing_scrollbar_site"
cp -R "$valid_site/." "$missing_scrollbar_site/"
sed -i 's/scrollbar-color/scrollbar-tone/' "$missing_scrollbar_site/styles.css"

set +e
output=$("$checker" "$missing_scrollbar_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted a page without a themed command scrollbar" >&2
  exit 1
fi

expected="missing site contract text in styles.css: scrollbar-color"
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify the missing command scrollbar theme" >&2
  echo "$output" >&2
  exit 1
fi

low_contrast_why_site="$fixture_root/low-contrast-why"
mkdir -p "$low_contrast_why_site"
cp -R "$valid_site/." "$low_contrast_why_site/"
sed -i 's/--amber-on-paper: #805500/--amber-on-paper: #ffc247/' \
  "$low_contrast_why_site/styles.css"

set +e
output=$("$checker" "$low_contrast_why_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted low-contrast amber text on the light Why section" >&2
  exit 1
fi

expected="light Why amber text contrast must be at least 4.5:1"
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify the low-contrast Why accent" >&2
  echo "$output" >&2
  exit 1
fi

incorrect_replay_attribution_site="$fixture_root/incorrect-replay-attribution"
mkdir -p "$incorrect_replay_attribution_site"
cp -R "$valid_site/." "$incorrect_replay_attribution_site/"
python3 - "$incorrect_replay_attribution_site/index.html" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
text = text.replace(
    "Replay matches each regenerated request",
    "Canonical typed request recorded",
)
text = text.replace(
    "Recorded and replayed final outcomes agree",
    "Record and replay requests agree",
)
text = text.replace("<code>outcome match</code>", "<code>driver calls 0</code>")
text = text.replace(
    '<p class="replay-property">Replay property: driver calls 0</p>',
    '<p class="replay-property">Replay has no fixture input</p>',
)
path.write_text(text, encoding="utf-8")
PY

set +e
output=$("$checker" "$incorrect_replay_attribution_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted inaccurate replay evidence attribution" >&2
  exit 1
fi

expected="run_completed must describe final outcome agreement, not request matching or driver calls"
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify inaccurate replay evidence attribution" >&2
  echo "$output" >&2
  exit 1
fi

incorrect_proposal_site="$fixture_root/incorrect-proposal"
mkdir -p "$incorrect_proposal_site"
cp -R "$valid_site/." "$incorrect_proposal_site/"
sed -i 's/program identity/policy evidence/' "$incorrect_proposal_site/index.html"

set +e
output=$("$checker" "$incorrect_proposal_site" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "site checker accepted incorrect proposal binding copy" >&2
  exit 1
fi

expected="missing site contract text in index.html: capability request, and program identity."
if [[ "$output" != *"$expected"* ]]; then
  echo "site checker did not identify incorrect proposal binding copy" >&2
  echo "$output" >&2
  exit 1
fi

echo "site checker accepts the static contract and rejects external dependencies, low-contrast Why accents, inaccurate replay evidence attribution, incorrect proposal copy, unconditional grant claims, missing malicious-host risks, missing execute_grant, and unthemed command scrollbars"
