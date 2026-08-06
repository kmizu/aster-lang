# README and GitHub Pages Enrichment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn ASTER's README and GitHub Pages site into one layered onboarding journey that explains the problem in 30 seconds, produces deterministic evidence in five minutes, and then exposes the authority and host boundaries without overstating the experimental runtime.

**Architecture:** Keep the current dependency-free Operational Amber site and make the repository README its denser executable companion. Both surfaces follow `Why -> Try -> See -> Understand -> Integrate -> Scope`; the README owns copy-ready commands and durable detail, while Pages owns visual orientation and points into the same checked-in example and normative documents. Existing shell contract checkers gain explicit onboarding assertions so fixture execution, replay, host grants, release links, accessibility, and local-only assets cannot silently drift.

**Tech Stack:** Markdown, semantic HTML5, dependency-free CSS, vanilla JavaScript, Bash contract tests, Rust 1.96 CLI fixtures, Firefox headless rendering.

## Global Constraints

- Preserve every invariant in `AGENTS.md`, especially Candidate opacity, the write path `intent -> propose -> authorize -> commit`, affine single-use permits, reconciliation, deterministic replay, and secret exclusion.
- ASTER v0.2.0 remains an experimental reference processor, not a production platform.
- `aster run` in the quickstart is fixture-backed and synthetic.
- `aster replay` receives no fixture or driver and performs no external effect.
- `effect_preview` is not authority; an external host may act only after the matching durable `execute_grant`.
- Do not claim that ASTER confines a malicious host or verifies an external provider's honesty.
- Do not add live providers, network, shell, MCP, native adapters, analytics, external fonts, external scripts, image CDNs, build-time site dependencies, or a documentation generator.
- Use only checked-in synthetic example data and actual locally reproduced output.
- Keep all site assets local and directly publishable as static files.
- Preserve keyboard focus, skip navigation, semantic headings, reduced motion, no-JavaScript readability, code overflow, and responsive layouts.
- Preserve all four v0.2.0 native downloads, `SHA256SUMS`, unsigned-binary guidance, and trace/snapshot sensitivity warnings.
- Run `./scripts/check.sh` after the final edit; do not claim completion from narrower checks.

## Visual Direction

**Visual thesis:** A calm black operations ledger cut by one amber authority line: editorial in scale, terminal-like in evidence, and precise rather than futuristic.

**Content plan:** Keep the full-bleed brand hero; follow with one problem comparison, one runnable proof, one trace explanation, the typed authority boundary, the host protocol, release downloads, honest scope, and a final documentation index.

**Interaction thesis:** Preserve the staged hero entrance, use the existing scroll reveal to move readers through proof and protocol, and keep amber row/focus transitions as the sole interactive accent. Do not add copy-button state or another JavaScript subsystem unless static selection proves inadequate during manual testing.

## File Map

- `README.md` — executable repository landing page, five-minute proof, authority explanation, install/release guidance, and durable documentation map.
- `scripts/check-docs.sh` — verifies the README keeps its approved onboarding and trust-boundary vocabulary in addition to existing local-link validation.
- `scripts/tests/check-docs.sh` — proves the documentation checker rejects a README that loses a required onboarding boundary.
- `site/index.html` — semantic single-page journey and all public copy, commands, protocol stages, release links, and documentation links.
- `site/styles.css` — Operational Amber composition, new why/quickstart/protocol layouts, and responsive/accessibility behavior.
- `site/site.js` — unchanged unless manual no-JavaScript/animation review exposes a real regression; existing hero, reveal, and trace motion are sufficient.
- `scripts/check-site.sh` — enforces the new anchors, deterministic proof language, protocol sequence, release artifacts, and existing local-only/accessibility contracts.
- `scripts/tests/check-site.sh` — proves the site checker rejects lost host-grant and quickstart contracts while retaining the external-dependency rejection.
- `docs/superpowers/specs/2026-08-07-readme-pages-enrichment-design.md` — approved design source; update only if implementation discovers a design-level contradiction.

---

### Task 1: Make the README a runnable first encounter

**Files:**
- Modify: `README.md`
- Modify: `scripts/check-docs.sh`
- Modify: `scripts/tests/check-docs.sh`

**Interfaces:**
- Consumes: `examples/meeting-scheduler/{main.aster,event.json,initial-state.json,capabilities.json,fixtures.json}`, `target/release/aster`, and the existing v0.2.0 release URLs.
- Produces: stable README headings `Why ASTER`, `Five-minute deterministic proof`, `What the proof establishes`, `Authority model`, `External host integration`, `Install a release archive`, `Project scope`, and `Documentation map`; exact commands reused conceptually by Task 2.

- [x] **Step 1: Add a failing README-boundary test**

Extend `scripts/tests/check-docs.sh` immediately after `contract_root` is copied
and before the existing host-protocol mutation test changes that fixture. Copy
the still-valid contract fixture, remove the driver-free wording from its
README, run the checker, and require this diagnostic:

```bash
readme_contract_root="$fixture_root/readme-contract"
cp -R "$contract_root" "$readme_contract_root"

python3 - "$readme_contract_root/README.md" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
text = path.read_text(encoding="utf-8")
path.write_text(text.replace("Driver-free replay", "Offline replay"), encoding="utf-8")
PY

set +e
output=$($readme_contract_root/scripts/check-docs.sh --allow-active-bootstrap "$readme_contract_root" 2>&1)
task_status=$?
set -e

if [[ $task_status -eq 0 ]]; then
  echo "documentation checker accepted a README without driver-free replay" >&2
  exit 1
fi

expected="README missing required onboarding term: Driver-free replay"
if [[ "$output" != *"$expected"* ]]; then
  echo "documentation checker did not identify the missing README contract" >&2
  echo "$output" >&2
  exit 1
fi
```

- [x] **Step 2: Run the focused test and verify the new case fails for the right reason**

Run:

```bash
./scripts/tests/check-docs.sh
```

Expected: non-zero with `documentation checker accepted a README without driver-free replay`, because `scripts/check-docs.sh` does not yet enforce the README onboarding terms.

- [x] **Step 3: Add the README onboarding contract to the checker**

In the Python block in `scripts/check-docs.sh`, after local README links are checked, require these exact strings:

```python
readme = root / "README.md"
if readme.is_file():
    readme_text = readme.read_text(encoding="utf-8")
    for term in (
        "## Why ASTER",
        "## Five-minute deterministic proof",
        "Fixture-backed record",
        "Driver-free replay",
        "## What the proof establishes",
        "## Authority model",
        "## External host integration",
        "effect_preview -> effect_admission -> execute_grant -> effect_resolution",
        "## Install a release archive",
        "## Project scope",
        "## Documentation map",
    ):
        if term not in readme_text:
            errors.append(f"README missing required onboarding term: {term}")
```

- [x] **Step 4: Rewrite the README around the approved six-part narrative**

Keep the title and central boundary quote, then place this reader-facing explanation before status detail:

```markdown
ASTER is an experimental language and deterministic runtime for AI agents that
need judgment without ambient authority. A model may propose typed data, but it
cannot execute a tool, mint a capability, or turn its output into an ordinary
value. The runtime owns effects, budgets, state publication, audit evidence,
and replay.

**Experimental reference processor · current release v0.2.0**

[Project site](https://kmizu.github.io/aster-lang/) ·
[Release v0.2.0](https://github.com/kmizu/aster-lang/releases/tag/v0.2.0) ·
[Language specification](docs/spec/aster-0.1.md) ·
[Host protocol](docs/spec/aster-host-protocol-0.2.md)
```

Add `## Why ASTER` with a compact comparison table that preserves the security model:

```markdown
| Concern | ASTER boundary |
| --- | --- |
| Model result | `Candidate<T>` is opaque until deterministic validation. |
| Read effect | A declared read tool runs only through `observe`. |
| Write effect | The program must cross `intent -> propose -> authorize -> commit`. |
| Authority | `Permit<A>` is runtime-issued, expiring, proposal-bound, and single-use. |
| Success | A receipt must match a later observation before state publishes. |
| Replay | The VM replays recorded resolutions without a driver parameter. |
```

Add `## Five-minute deterministic proof` with the following copy-ready Bash sequence. Explain above it that Rust 1.96, a repository checkout, and a Bash-compatible shell are required; the checked-in data is synthetic; and successful ASTER commands are intentionally quiet:

```bash
cargo build --release -p aster-cli --bin aster

ASTER_DEMO_DIR=$(mktemp -d)

./target/release/aster check examples/meeting-scheduler/main.aster

./target/release/aster run examples/meeting-scheduler/main.aster \
  --agent Scheduler \
  --event message \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --fixtures examples/meeting-scheduler/fixtures.json \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --snapshot-dir "$ASTER_DEMO_DIR/snapshots" \
  --output-state "$ASTER_DEMO_DIR/record-state.json"

./target/release/aster replay examples/meeting-scheduler/main.aster \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --output-state "$ASTER_DEMO_DIR/replay-state.json"

cmp "$ASTER_DEMO_DIR/record-state.json" "$ASTER_DEMO_DIR/replay-state.json" \
  && echo "record and replay states match"
wc -l < "$ASTER_DEMO_DIR/meeting.trace.jsonl"
cat "$ASTER_DEMO_DIR/record-state.json"
```

Document the reproduced output exactly:

```text
record and replay states match
34
{"schema_version":1,"state":{"last_event":{"some":{"id":"event-001"}},"profile":{"known_attendees":[]}}}
```

Under `## What the proof establishes`, label the two phases exactly `Fixture-backed record` and `Driver-free replay`. Explain that record mode consumes the five synthetic fixture effects, reserves/settles budget, writes snapshots and a 34-entry hash chain, and publishes state only after reconciliation. Explain that replay has no fixture/driver input, re-steps deterministic semantics, and rejects request or trace divergence.

Follow with:

- `## Authority model`: the four values and the full source-stage sequence;
- `## External host integration`: the exact four-frame sequence and the malicious-host limitation;
- `## Install a release archive`: all four v0.2.0 asset links, `SHA256SUMS`, unsigned/notarization caveat, and source-build alternative;
- `## Project scope`: implemented scope, explicit exclusions, workspace dependency direction, repository checks, and trace sensitivity;
- `## Documentation map`: language spec, host spec, architecture, core beliefs, runtime/replay, security model, diagnostics, both examples, and v0.2.0 release notes.

Remove the old duplicate `Governed external host`, `Meeting scheduler workflow`, prerequisites, release, and warning sections only after their unique content has moved into the new structure.

- [x] **Step 5: Run the documented proof exactly as written**

Run the command block from Step 4 in a fresh shell. Expected:

- `check`, `run`, and `replay` exit 0 with no success chatter;
- `cmp` prints `record and replay states match`;
- `wc -l` prints `34`;
- final state is byte-for-byte the JSON documented above;
- `git status --short` shows no generated `.aster` or example changes.

- [x] **Step 6: Run focused documentation validation**

Run:

```bash
./scripts/tests/check-docs.sh
./scripts/check-docs.sh
git diff --check
```

Expected: both documentation checks print their success messages and all commands exit 0.

- [x] **Step 7: Commit the runnable README**

```bash
git add README.md scripts/check-docs.sh scripts/tests/check-docs.sh
git commit -m "Enrich ASTER README onboarding"
```

### Task 2: Make Pages tell the same story semantically

**Files:**
- Modify: `site/index.html`
- Modify: `scripts/check-site.sh`
- Modify: `scripts/tests/check-site.sh`

**Interfaces:**
- Consumes: the exact quickstart command/result contract from Task 1, existing release URLs, and existing `[data-reveal]` behavior from `site/site.js`.
- Produces: stable section IDs `hero`, `why`, `quickstart`, `evidence`, `boundary`, `protocol`, `download`, and `docs`; CSS class hooks consumed by Task 3.

- [x] **Step 1: Extend the valid site fixture and add a failing protocol mutation test**

Update the valid HTML fixture in `scripts/tests/check-site.sh` to contain every new section and required term:

```html
<section id="hero"><h1>Authority before action.</h1></section>
<section id="why">Judgment without authority.</section>
<section id="quickstart">
  <h2>Prove it in five minutes.</h2>
  <p>Fixture-backed record</p>
  <p>Driver-free replay</p>
  <p>34 trace entries</p>
</section>
<section id="evidence">record and replay states match; driver calls 0</section>
<section id="boundary">Candidate Proposal Permit Reconciliation</section>
<section id="protocol">
  effect_preview effect_admission execute_grant effect_resolution
  A preview is not authority.
</section>
```

After the existing external-dependency assertion, add a second mutation:

```bash
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
```

- [x] **Step 2: Run the focused site test and verify the new case fails**

Run:

```bash
./scripts/tests/check-site.sh
```

Expected: non-zero with `site checker accepted a page without execute_grant`.

- [x] **Step 3: Strengthen the static-site contract**

In `scripts/check-site.sh`, replace the current section list with:

```bash
for section in hero why quickstart evidence boundary protocol download docs; do
  require_pattern index.html "<section\\s+[^>]*id=[\"']${section}[\"']"
done
```

Add exact public-boundary assertions:

```bash
for text in \
  'Judgment without authority.' \
  'Prove it in five minutes.' \
  'Fixture-backed record' \
  'Driver-free replay' \
  '34 trace entries' \
  'record and replay states match' \
  'effect_preview' \
  'effect_admission' \
  'execute_grant' \
  'effect_resolution' \
  'A preview is not authority.'; do
  require_text index.html "$text"
done
```

Keep all existing release-asset, checksum, focus, reduced-motion,
`IntersectionObserver`, 404, and external-dependency checks.

- [x] **Step 4: Recompose `site/index.html` without changing the brand core**

Update the header navigation to `Why`, `Quickstart`, `Protocol`, `Download`,
and `Docs`. Keep the brand and version marker. Change the primary hero action to:

```html
<a class="button button-primary" href="#quickstart">
  Run the proof <span aria-hidden="true">↓</span>
</a>
```

Keep the existing hero wordmark, headline, lede, and four-stage trace. Insert
the following semantic section skeletons in the approved order; every content
block that may animate must also be readable before JavaScript runs:

```html
<section class="why section-light" id="why" aria-labelledby="why-title">
  <div class="why-statement reveal" data-reveal>
    <p class="section-index">01 / Why ASTER</p>
    <h2 id="why-title">Judgment<br>without authority.</h2>
    <p>Models are useful where answers require judgment. That is exactly why their output should remain data.</p>
  </div>
  <div class="why-comparison reveal" data-reveal>
    <div class="comparison-row comparison-risk">
      <span>Integration risk</span>
      <div>
        <strong>Model response → host tool call</strong>
        <p>Without an enforced boundary, generated data can be consumed as an instruction beside ambient host authority.</p>
      </div>
    </div>
    <div class="comparison-row comparison-aster">
      <span>ASTER path</span>
      <div>
        <strong>Candidate → validation → governed effect</strong>
        <p>The runtime keeps data, intent, authority, execution, and observed reality as separate transitions.</p>
      </div>
    </div>
  </div>
</section>

<section class="quickstart section-dark" id="quickstart" aria-labelledby="quickstart-title">
  <div class="section-heading reveal" data-reveal>
    <p class="section-index">02 / Deterministic proof</p>
    <h2 id="quickstart-title">Prove it in five minutes.</h2>
    <p>Repository checkout · Bash · Rust 1.96 · synthetic fixtures only.</p>
  </div>
  <ol class="quickstart-steps">
    <li class="quickstart-step reveal" data-reveal>
      <span class="step-number">01</span>
      <div>
        <strong>Build and check</strong>
        <p>Compile the pinned reference processor and validate the program.</p>
      </div>
      <pre><code>cargo build --release -p aster-cli --bin aster
ASTER_DEMO_DIR=$(mktemp -d)
./target/release/aster check examples/meeting-scheduler/main.aster</code></pre>
    </li>
    <li class="quickstart-step reveal" data-reveal>
      <span class="step-number">02</span>
      <div>
        <strong>Fixture-backed record</strong>
        <p>Execute five synthetic effects and record every governed boundary.</p>
      </div>
      <pre><code>./target/release/aster run examples/meeting-scheduler/main.aster \
  --agent Scheduler --event message \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --fixtures examples/meeting-scheduler/fixtures.json \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --snapshot-dir "$ASTER_DEMO_DIR/snapshots" \
  --output-state "$ASTER_DEMO_DIR/record-state.json"</code></pre>
    </li>
    <li class="quickstart-step reveal" data-reveal>
      <span class="step-number">03</span>
      <div>
        <strong>Driver-free replay</strong>
        <p>Re-step the VM without fixtures, a host, or a driver.</p>
      </div>
      <pre><code>./target/release/aster replay examples/meeting-scheduler/main.aster \
  --trace "$ASTER_DEMO_DIR/meeting.trace.jsonl" \
  --input examples/meeting-scheduler/event.json \
  --state examples/meeting-scheduler/initial-state.json \
  --capabilities examples/meeting-scheduler/capabilities.json \
  --output-state "$ASTER_DEMO_DIR/replay-state.json"
cmp "$ASTER_DEMO_DIR/record-state.json" \
  "$ASTER_DEMO_DIR/replay-state.json"</code></pre>
    </li>
  </ol>
  <div class="proof-result reveal" data-reveal>
    <strong>record and replay states match</strong>
    <span>34 trace entries</span>
    <code>driver calls 0 on replay</code>
  </div>
</section>

<section class="protocol section-dark" id="protocol" aria-labelledby="protocol-title">
  <div class="section-heading reveal" data-reveal>
    <p class="section-index">05 / External host boundary</p>
    <h2 id="protocol-title">Durable before granted.</h2>
    <p>A preview is not authority. ASTER reserves budget and persists the continuation before issuing an execute_grant.</p>
  </div>
  <ol class="protocol-flow" aria-label="ASTER host protocol sequence">
    <li class="protocol-step reveal" data-reveal>
      <span>01</span>
      <strong>effect_preview</strong>
      <p>The host may inspect the exact typed request. A preview is not authority.</p>
    </li>
    <li class="protocol-step reveal" data-reveal>
      <span>02</span>
      <strong>effect_admission</strong>
      <p>ASTER accepts declared maximum usage, reserves budget, and seals the continuation.</p>
    </li>
    <li class="protocol-step reveal" data-reveal>
      <span>03</span>
      <strong>execute_grant</strong>
      <p>Only the matching grant permits execution after snapshot and trace persistence.</p>
    </li>
    <li class="protocol-step reveal" data-reveal>
      <span>04</span>
      <strong>effect_resolution</strong>
      <p>ASTER validates the bound typed result, settles usage, and resumes the machine.</p>
    </li>
  </ol>
  <p class="host-warning reveal" data-reveal>
    The host remains outside ASTER's trust boundary. Process isolation and least privilege remain deployment responsibilities.
  </p>
</section>
```

Keep each quickstart row's command complete and free of ellipses. The comparison
must not claim that ASTER removes the host's ambient OS authority. Keep the
protocol frame names in the normative order shown above.

Change the existing evidence section to use the meeting-scheduler source excerpt
and actual trace kinds: `effect_requested`, `budget_reserved`,
`policy_decision`, `permit_issued`, `proposal_committed`,
`reconciliation_decision`, `state_committed`, and `run_completed`. Retain the
record/replay split and `driver calls 0` result.

Keep and renumber the authority, download, scope, and docs sections. Add docs
links for the language specification, host specification, architecture,
security model, meeting-scheduler example, governed-note example, and release
notes. Do not remove any v0.2.0 download URL or limitation.

- [x] **Step 5: Run semantic and link checks before styling**

Run:

```bash
./scripts/tests/check-site.sh
./scripts/check-site.sh
./scripts/check-docs.sh
git diff --check
```

Expected: all commands exit 0. The page may look temporarily under-styled, but
its reading order, anchors, commands, claims, and release links are complete.

- [x] **Step 6: Commit the semantic Pages journey**

```bash
git add site/index.html scripts/check-site.sh scripts/tests/check-site.sh
git commit -m "Add guided ASTER Pages journey"
```

### Task 3: Extend Operational Amber across the new journey

**Files:**
- Modify: `site/styles.css`
- Verify unchanged: `site/site.js`

**Interfaces:**
- Consumes: `.why`, `.why-statement`, `.why-comparison`, `.quickstart`, `.quickstart-steps`, `.quickstart-step`, `.proof-result`, `.protocol`, `.protocol-flow`, `.protocol-step`, and `.host-warning` from Task 2.
- Produces: desktop, tablet, and mobile layouts that use the existing `--amber`, `--ink`, `--paper`, `--line`, `--display`, and `--mono` tokens and existing `.reveal` motion.

- [x] **Step 1: Capture the semantic baseline at desktop and mobile widths**

Run:

```bash
ASTER_SHOT_DIR=$(mktemp -d)
firefox --headless --screenshot "$ASTER_SHOT_DIR/before-desktop.png" \
  --window-size 1440,1200 "file://$PWD/site/index.html"
firefox --headless --screenshot "$ASTER_SHOT_DIR/before-mobile.png" \
  --window-size 390,844 "file://$PWD/site/index.html"
```

Expected: both screenshots are created. Record visible overflow, unstyled rows,
and heading collisions to compare after the CSS change.

- [x] **Step 2: Add the light Why composition**

Add a full-width editorial composition, not cards:

```css
.why {
  display: grid;
  grid-template-columns: minmax(0, .9fr) minmax(24rem, 1.1fr);
  gap: clamp(3rem, 8vw, 9rem);
  border-top: .5rem solid var(--amber);
}

.why-statement h2 {
  max-width: 8ch;
  margin: 1.5rem 0 2rem;
  font-size: clamp(4rem, 8vw, 8.5rem);
  line-height: .82;
  letter-spacing: -.075em;
}

.why-comparison {
  align-self: end;
  border-top: 2px solid var(--paper-ink);
}

.comparison-row {
  display: grid;
  grid-template-columns: 8rem 1fr;
  gap: 2rem;
  padding: 2rem 0;
  border-bottom: 1px solid #b8b1a5;
}
```

Use amber only for the ASTER path and small labels; keep the section primarily
paper and black so it resets the eye after the hero.

- [x] **Step 3: Build the terminal proof as a numbered process band**

Add:

```css
.quickstart {
  border-top: 1px solid var(--line);
  background: #0b0c0f;
}

.quickstart-steps {
  margin: 4rem 0 0;
  padding: 0;
  border-top: 1px solid var(--line-strong);
  list-style: none;
}

.quickstart-step {
  display: grid;
  grid-template-columns: 5rem minmax(12rem, .65fr) minmax(0, 1.35fr);
  gap: clamp(1.5rem, 4vw, 4rem);
  padding: 2rem 0;
  border-bottom: 1px solid var(--line);
}

.quickstart-step pre {
  min-width: 0;
  margin: 0;
  padding: 1.25rem;
  overflow-x: auto;
  border-left: 2px solid var(--amber);
  color: #c7c7c2;
  background: #07080a;
  font: 500 .72rem/1.7 var(--mono);
}

.proof-result {
  display: grid;
  grid-template-columns: 1.4fr .8fr .8fr;
  gap: 1px;
  margin-top: 1px;
  background: var(--amber);
}

.proof-result > * {
  padding: 1.5rem;
  color: var(--paper-ink);
  background: var(--amber);
  font-family: var(--mono);
}
```

Keep the command text selectable and complete. Do not add fake terminal chrome,
copy buttons, or decorative traffic-light dots.

- [x] **Step 4: Add the host protocol authority line**

Add:

```css
.protocol {
  border-top: 1px solid var(--line);
  background: #090a0c;
}

.protocol-flow {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  margin: 4rem 0 0;
  padding: 0;
  border-top: 1px solid var(--line-strong);
  border-bottom: 1px solid var(--line);
  list-style: none;
}

.protocol-step {
  position: relative;
  min-height: 18rem;
  padding: 2rem;
  border-right: 1px solid var(--line);
}

.protocol-step:nth-child(3) {
  background: linear-gradient(180deg, rgba(255, 194, 71, .12), transparent);
}

.protocol-step:nth-child(3)::before {
  position: absolute;
  top: -2px;
  right: 0;
  left: 0;
  height: 3px;
  background: var(--amber);
  content: "";
}

.host-warning {
  max-width: 62rem;
  margin: 2.5rem 0 0 auto;
  padding-left: 1.5rem;
  border-left: 2px solid #66696f;
  color: var(--muted);
}
```

Visually emphasize only `execute_grant`; the other phases remain necessary but
quiet. Reuse `.reveal`; do not add another animation observer.

- [x] **Step 5: Add tablet and mobile rules for every new composition**

Inside `@media (max-width: 980px)`, add:

```css
.why {
  grid-template-columns: 1fr;
}

.quickstart-step {
  grid-template-columns: 4rem 1fr;
}

.quickstart-step pre {
  grid-column: 2;
}

.protocol-flow {
  grid-template-columns: 1fr 1fr;
}

.protocol-step:nth-child(2) {
  border-right: 0;
}
```

Inside `@media (max-width: 680px)`, add:

```css
.why-statement h2 {
  font-size: clamp(3.4rem, 18vw, 5.5rem);
}

.comparison-row,
.quickstart-step,
.proof-result,
.protocol-flow {
  grid-template-columns: 1fr;
}

.quickstart-step pre {
  grid-column: 1;
  font-size: .64rem;
}

.protocol-step {
  min-height: 14rem;
  border-right: 0;
  border-bottom: 1px solid var(--line);
}
```

Also ensure every new code block has horizontal overflow rather than wrapping
flags into unreadable fragments, every touch target remains at least the
existing button/link height, and the first viewport still contains the ASTER
brand, complete hero headline, lede, and primary action at 390x844.

- [x] **Step 6: Render and inspect the final page with and without JavaScript**

Run the two Firefox screenshot commands from Step 1 using `after-desktop.png`
and `after-mobile.png`. Then temporarily render a copy of `site/index.html`
with the script tag removed, without modifying the repository file:

```bash
sed '/<script src="site.js" defer><\/script>/d' site/index.html \
  > "$ASTER_SHOT_DIR/no-js.html"
firefox --headless --screenshot "$ASTER_SHOT_DIR/no-js.png" \
  --window-size 1440,1200 "file://$ASTER_SHOT_DIR/no-js.html"
```

Inspect all three images. Expected:

- the first viewport remains a single poster-like composition;
- the Why section has one comparison, not a card grid;
- commands are readable and scroll rather than clipping;
- `execute_grant` is the protocol's sole visual emphasis;
- no section repeats the same dominant layout;
- no content disappears without JavaScript;
- mobile has no horizontal page overflow or overlapping text.

If inspection finds a defect, add the smallest CSS correction and rerun all
three renders before continuing.

- [x] **Step 7: Run focused site checks and commit the visual system**

Run:

```bash
./scripts/tests/check-site.sh
./scripts/check-site.sh
git diff --check
```

Expected: all exit 0. Then commit:

```bash
git add site/styles.css
git commit -m "Extend Operational Amber onboarding"
```

### Task 4: Audit the complete public onboarding contract

**Files:**
- Modify if evidence requires: `README.md`, `site/index.html`, `site/styles.css`, `scripts/check-docs.sh`, `scripts/tests/check-docs.sh`, `scripts/check-site.sh`, `scripts/tests/check-site.sh`
- Update: `docs/superpowers/plans/2026-08-07-readme-pages-enrichment.md`

**Interfaces:**
- Consumes: all Task 1-3 artifacts and the approved design specification.
- Produces: verified branch state ready for review/push; no deployment claim until the public site is byte-compared after merge and Pages publication.

- [x] **Step 1: Re-run the README quickstart from a clean temporary directory**

Execute the exact README commands without substitutions. Capture and compare:

```bash
cmp "$ASTER_DEMO_DIR/record-state.json" "$ASTER_DEMO_DIR/replay-state.json"
test "$(wc -l < "$ASTER_DEMO_DIR/meeting.trace.jsonl")" -eq 34
test "$(cat "$ASTER_DEMO_DIR/record-state.json")" = \
  '{"schema_version":1,"state":{"last_event":{"some":{"id":"event-001"}},"profile":{"known_attendees":[]}}}'
```

Expected: all exit 0. Confirm `replay` received no `--fixtures`, host transport,
or driver argument.

- [x] **Step 2: Audit every public claim against authoritative docs**

Use `rg` to locate all README and Pages claims about candidates, permits,
reconciliation, replay, host grants, and malicious hosts:

```bash
rg -n "Candidate|Permit|reconcil|driver-free|effect_preview|execute_grant|malicious host|production" \
  README.md site/index.html
```

Compare each result with `docs/spec/aster-0.1.md`,
`docs/spec/aster-host-protocol-0.2.md`, `docs/design-docs/runtime-and-replay.md`,
and `docs/design-docs/security-model.md`. Fix any wording that grants broader
guarantees than those documents, then rerun focused docs/site checks.

- [x] **Step 3: Audit links, release artifacts, and static dependencies**

Run:

```bash
./scripts/check-docs.sh
./scripts/check-site.sh
./scripts/check-release.sh
rg -n 'https?://' site/index.html site/styles.css site/site.js
```

Expected: checker commands exit 0. HTTP URLs in `site/index.html` are navigation
links only; no external `src`, stylesheet, font, media, `@import`, or CSS `url()`
dependency exists. All four v0.2.0 archives and `SHA256SUMS` remain present.

- [x] **Step 4: Run the full repository gate after the final edit**

Run:

```bash
./scripts/check.sh
```

Expected: formatting, Clippy with `-D warnings`, all workspace tests, checker
self-tests, architecture, production Rust, documentation, static site, and
release validation all pass.

- [x] **Step 5: Review the complete branch diff and worktree**

Run:

```bash
git diff --check main...HEAD
git diff --stat main...HEAD
git log --oneline --decorate main..HEAD
git status --short --branch
```

Expected: no whitespace errors; only the approved design, plan, README, site,
and checker files changed; no screenshots, generated traces, snapshots,
`.aster`, `.superdesign`, credentials, or temporary files are tracked; worktree
is clean after recording validation results.

- [x] **Step 6: Record validation evidence and commit any final corrections**

Update this plan's checklist, add a short `## Validation Results` section with
the actual commands, exit status, test count, screenshot widths, trace count,
and record/replay equality result. If that update or evidence-driven corrections
change files, run their focused check and commit:

```bash
git add docs/superpowers/plans/2026-08-07-readme-pages-enrichment.md \
  README.md site/index.html site/styles.css \
  scripts/check-docs.sh scripts/tests/check-docs.sh \
  scripts/check-site.sh scripts/tests/check-site.sh
git commit -m "Record README and Pages validation"
```

- [ ] **Step 7: Verify publication only after integration deploys Pages**

After the branch is reviewed, integrated, and the Pages deployment completes,
download the public HTML and compare it with the integrated source:

```bash
curl --fail --silent --show-error --location \
  https://kmizu.github.io/aster-lang/ -o /tmp/aster-public-index.html
sha256sum site/index.html /tmp/aster-public-index.html
```

Expected: both hashes are identical. Open the public page once at desktop and
mobile width and confirm Quickstart and Protocol anchors work. Until this step
passes, report the local/branch work as complete but do not claim the public
Pages deployment is updated.

## Validation Results

- README proof: the documented command block was executed literally from the
  repository with a fresh `ASTER_DEMO_DIR`; `check`, fixture-backed `run`,
  driver-free `replay`, both `cmp` invocations, and the exact-state assertions
  exited 0. The trace contained 34 entries, and record/replay state was
  byte-identical at
  `{"schema_version":1,"state":{"last_event":{"some":{"id":"event-001"}},"profile":{"known_attendees":[]}}}`.
- Public contract audit: the Candidate, Permit, reconciliation, replay, durable
  grant, production-scope, and malicious-host wording in `README.md` and
  `site/index.html` was checked against the two normative specifications and
  the runtime/replay and security design documents. One evidence-backed Pages
  correction removed budget and policy evidence from the `Proposal<A>` binding
  description and named its action, arguments, intent, risk, capability
  request, and program identity instead. `./scripts/check-docs.sh`,
  `./scripts/check-site.sh`, and
  `./scripts/check-release.sh` exited 0. No external site dependency was found;
  all HTTP(S) occurrences are navigation or download links.
- Release evidence: GitHub release `v0.2.0` is neither draft nor prerelease and
  contains the four documented platform archives plus `SHA256SUMS`.
- Render evidence: fresh Firefox 148 captures were inspected at 1440x1200
  desktop, 390x844 mobile, 1440x1200 without JavaScript, and 390x844 mobile
  Quickstart. The first viewport remained intact, no-JavaScript content stayed
  visible, and the mobile command surface retained horizontal scrolling with a
  thin graphite scrollbar instead of the bright native scrollbar. All images
  were temporary files outside the repository.
- Repository gate: `./scripts/check.sh` exited 0 after the implementation edits
  with 156 workspace tests passing and 0 failing, followed by successful
  checker self-tests, architecture, production-Rust, documentation, static-site,
  and release validation. `git diff --check main...HEAD` and `git diff --check`
  reported no whitespace errors; the branch contains only the approved design,
  plan, README, site, and checker paths and no generated or sensitive artifacts.
- Publication: public Pages byte comparison and anchor inspection remain
  pending external integration and deployment. No public-site update is claimed.
