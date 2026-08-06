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
for section in hero boundary evidence download docs; do
  require_pattern index.html "<section\\s+[^>]*id=[\"']${section}[\"']"
done

require_pattern index.html "<a\\s+[^>]*href=[\"']#main-content[\"']"
require_text index.html 'styles.css'
require_text index.html 'site.js'
require_text index.html 'v0.2.0'
require_text index.html 'SHA256SUMS'

for asset in \
  aster-v0.2.0-x86_64-unknown-linux-musl.tar.gz \
  aster-v0.2.0-aarch64-apple-darwin.tar.gz \
  aster-v0.2.0-x86_64-apple-darwin.tar.gz \
  aster-v0.2.0-x86_64-pc-windows-msvc.zip; do
  require_text index.html "$asset"
done

require_text styles.css ':focus-visible'
require_text styles.css 'prefers-reduced-motion'
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

echo "static site contract is valid"
