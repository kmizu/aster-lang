#!/usr/bin/env bash
set -euo pipefail

site_root=${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../site" && pwd -P)}

fail() {
  echo "$1" >&2
  exit 1
}

require_file() {
  local relative=$1
  [[ -f "$site_root/$relative" ]] || fail "missing site file: $relative"
}

require_text() {
  local relative=$1
  local text=$2
  rg --fixed-strings --quiet -- "$text" "$site_root/$relative" ||
    fail "missing site contract text in $relative: $text"
}

require_pattern() {
  local relative=$1
  local pattern=$2
  rg --ignore-case --pcre2 --quiet -- "$pattern" "$site_root/$relative" ||
    fail "missing site contract pattern in $relative: $pattern"
}

for relative in index.html styles.css site.js 404.html; do
  require_file "$relative"
done

require_pattern index.html "<header(?:\\s|>)"
require_pattern index.html "<main\\s+[^>]*id=[\"']main-content[\"']"
require_pattern index.html "<footer(?:\\s|>)"
for section in hero why quickstart evidence boundary protocol download docs; do
  require_pattern index.html "<section\\s+[^>]*id=[\"']${section}[\"']"
done

require_pattern index.html "<a\\s+[^>]*href=[\"']#main-content[\"']"
require_text index.html 'styles.css'
require_text index.html 'site.js'
require_text index.html 'v0.2.0'
require_text index.html 'SHA256SUMS'

for text in \
  'Judgment without authority.' \
  'Prove it in five minutes.' \
  'Fixture-backed record' \
  'Driver-free replay' \
  '34 trace entries' \
  'record and replay states match' \
  'capability request, and program identity.' \
  'effect_preview' \
  'effect_admission' \
  'execute_grant' \
  'effect_resolution' \
  'A preview is not authority.' \
  'A conforming host must not execute before receiving the matching grant.' \
  'A malicious host can act early, falsify provider behavior, or under-report usage by using authority it already has.'; do
  require_text index.html "$text"
done

for asset in \
  aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz \
  aster-v0.2.0-aarch64-apple-darwin.tar.gz \
  aster-v0.2.0-x86_64-apple-darwin.tar.gz \
  aster-v0.2.0-x86_64-pc-windows-msvc.zip; do
  require_text index.html "$asset"
done

require_text styles.css ':focus-visible'
require_text styles.css 'prefers-reduced-motion'
require_text styles.css 'scrollbar-color'
require_text styles.css 'scrollbar-width'
require_text site.js 'IntersectionObserver'
require_text 404.html 'ASTER'
require_text 404.html '404'
require_pattern 404.html "<a\\s+[^>]*href=[\"']\\./[\"']"

if rg --ignore-case --pcre2 --quiet \
  '<(?:script|img|iframe|video|audio|source|link)\b[^>]+https?://' \
  "$site_root/index.html" "$site_root/404.html"; then
  fail "external site dependency is forbidden"
fi

if rg --ignore-case --pcre2 --quiet \
  '(?:@import|url\()[^;]*(?:https?:)?//' \
  "$site_root/styles.css"; then
  fail "external site dependency is forbidden"
fi

for forbidden in 'src="//' "src='//" 'href="//' "href='//"; do
  if rg --fixed-strings --quiet -- "$forbidden" \
    "$site_root/index.html" "$site_root/404.html"; then
    fail "external site dependency is forbidden"
  fi
done

python3 - "$site_root/index.html" <<'PY'
import html
import pathlib
import re
import sys

document = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def visible_text(fragment: str) -> str:
    without_tags = re.sub(r"<[^>]+>", " ", fragment)
    return " ".join(html.unescape(without_tags).split())


evidence_match = re.search(
    r"<section\b(?=[^>]*\bid=[\"']evidence[\"'])[^>]*>(.*?)</section>",
    document,
    re.IGNORECASE | re.DOTALL,
)
if evidence_match is None:
    fail("missing executable evidence section")

event_rows = {}
for row_match in re.finditer(
    r"<li\b[^>]*>(.*?)</li>",
    evidence_match.group(1),
    re.IGNORECASE | re.DOTALL,
):
    row_text = visible_text(row_match.group(1))
    for event in ("effect_requested", "run_completed"):
        if re.search(rf"\b{event}\b", row_text):
            event_rows[event] = (row_text, row_match.group(0))

for event in ("effect_requested", "run_completed"):
    if event not in event_rows:
        fail(f"missing replay evidence row: {event}")

completion_text, completion_markup = event_rows["run_completed"]
completion_lower = completion_text.lower()
if (
    "Recorded and replayed final outcomes agree" not in completion_text
    or "request" in completion_lower
    or "driver calls" in completion_lower
):
    fail(
        "run_completed must describe final outcome agreement, "
        "not request matching or driver calls"
    )

request_text, _ = event_rows["effect_requested"]
if "Replay matches each regenerated request" not in request_text:
    fail("effect_requested must describe replay request matching")

outside_completion = document.replace(completion_markup, "", 1)
if "driver calls 0" not in visible_text(outside_completion):
    fail("driver calls 0 must be a separate replay property")
PY

python3 - "$site_root/styles.css" <<'PY'
import pathlib
import re
import sys

stylesheet = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


root_match = re.search(r":root\s*\{([^}]*)\}", stylesheet, re.DOTALL)
if root_match is None:
    fail("missing :root color tokens in styles.css")

tokens = dict(
    re.findall(
        r"--([a-z0-9-]+)\s*:\s*(#[0-9a-f]{6})\s*;",
        root_match.group(1),
        re.IGNORECASE,
    )
)
for token in ("paper", "amber-on-paper"):
    if token not in tokens:
        fail(f"missing site color token: --{token}")


def relative_luminance(color: str) -> float:
    channels = [int(color[index : index + 2], 16) / 255 for index in (1, 3, 5)]
    linear = [
        channel / 12.92
        if channel <= 0.04045
        else ((channel + 0.055) / 1.055) ** 2.4
        for channel in channels
    ]
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]


foreground = relative_luminance(tokens["amber-on-paper"])
background = relative_luminance(tokens["paper"])
lighter, darker = max(foreground, background), min(foreground, background)
contrast = (lighter + 0.05) / (darker + 0.05)
if contrast < 4.5:
    fail(
        "light Why amber text contrast must be at least 4.5:1 "
        f"(got {contrast:.2f}:1)"
    )

required_selectors = {
    ".why .section-index",
    ".why .comparison-aster > span",
    ".why .comparison-aster strong",
}
accent_selectors = set()
for selectors, declarations in re.findall(r"([^{}]+)\{([^{}]*)\}", stylesheet):
    if re.search(
        r"(?:^|;)\s*color\s*:\s*var\(--amber-on-paper\)\s*(?:;|$)",
        declarations,
    ):
        accent_selectors.update(selector.strip() for selector in selectors.split(","))

missing = sorted(required_selectors - accent_selectors)
if missing:
    fail(
        "light Why amber text must use --amber-on-paper for selectors: "
        + ", ".join(missing)
    )
PY

echo "static site contract is valid"
