# ASTER README and GitHub Pages Enrichment Design

**Date:** 2026-08-07  
**Status:** Approved direction; implementation pending  
**Audience:** AI-agent developers first, language/runtime implementers second

## Context

ASTER v0.2.0 has a strong public identity and a complete experimental vertical
slice, but its two main entry points currently emphasize different incomplete
parts of the story:

- the project site explains the authority boundary clearly and presents the
  release well, but does not give a visitor a short path to a successful run;
- the README contains exact commands and normative links, but starts with dense
  scope and release detail before showing why ASTER is useful or what a
  successful execution proves.

The bundled meeting-scheduler example already supports a compelling first-run
experience. `check`, fixture-backed `run`, and driver-free `replay` complete
successfully, produce a 34-entry hash-chained trace, and publish byte-identical
record and replay state. The documentation should make that evidence legible
without implying live-provider support or production readiness.

## Goals

The enriched README and Pages site must let a new visitor:

1. understand ASTER's value proposition within roughly 30 seconds;
2. complete a deterministic demonstration within roughly five minutes after
   obtaining or building the CLI;
3. understand what the demonstration did and what its artifacts prove;
4. distinguish fixture-backed execution from the external-host protocol;
5. follow the authority path from model output through reconciliation;
6. find the specifications, architecture, security model, examples, and release
   artifacts without searching the repository tree;
7. see ASTER's experimental scope and residual host risks before mistaking it
   for a production platform.

The two surfaces will tell the same story at different densities. Pages is the
visual orientation and launch point. README is the executable, repository-local
guide and durable reference map.

## Non-goals

- No source-language, host-protocol, runtime, or security invariant changes.
- No live provider, network, shell, MCP, or native adapter integration.
- No claim that ASTER confines a malicious external host.
- No invented benchmark, adoption, security-certification, or production-use
  claims.
- No external fonts, scripts, image CDNs, analytics, or build-time site
  dependencies.
- No general documentation-site generator or multi-page documentation system.

## Shared narrative

Both surfaces follow the same six-part narrative:

1. **Why** — model judgment is useful, but model output must not be authority.
2. **Try** — run `check`, fixture-backed `run`, and driver-free `replay`.
3. **See** — inspect the final state and trace, then verify record/replay state
   equality.
4. **Understand** — follow `Candidate -> Proposal -> Permit ->
   Reconciliation` and the read/write effect paths.
5. **Integrate** — learn where a bounded external host attaches and when an
   `execute_grant` permits action.
6. **Scope** — understand the implemented vertical slice, exclusions, and
   residual risks.

This is a layered design rather than a choice between tutorial and manifesto:
the first layer gives an immediate, reproducible proof; the second explains the
language and runtime principles that make the proof meaningful.

## README information architecture

The README will be reorganized in this order:

1. **Identity and concise status**
   - Keep the central boundary statement.
   - Add a plain-language paragraph naming the problem ASTER solves.
   - Put project site, release, and specification links near the top.
   - State `experimental reference processor` before any setup instructions.

2. **Why ASTER**
   - Contrast a conventional direct model-to-tool path with ASTER's typed
     transitions.
   - Explain that the model is an inference oracle, not a security principal.
   - Use concrete consequences: opaque model output, explicit write authority,
     budget reservation before effects, reconciliation before state publication,
     and driver-free replay.

3. **Five-minute deterministic proof**
   - Offer a build-from-source path that works from a repository checkout.
   - Run the bundled meeting-scheduler with checked-in synthetic fixtures.
   - Use a temporary output directory so the quickstart does not dirty the
     repository.
   - Show how to inspect the final state and trace count.
   - Replay without fixtures or a driver and compare output states.
   - State the expected evidence and explain that successful CLI commands are
     intentionally quiet.

4. **What just happened**
   - Map the example stages to `infer`, `validate`, `observe`, `intent`,
     `propose`, `authorize`, `commit`, and `reconcile`.
   - Explain the record/replay equality and hash-chained trace in reader-facing
     terms without weakening the normative definitions.

5. **Authority model**
   - Preserve the precise `Candidate`, `Proposal`, `Permit`, and
     `Reconciliation` story.
   - Add a compact table for value, owner, allowed transition, and forbidden
     shortcut.

6. **External host integration**
   - Explain fixture mode before host mode so the two are not conflated.
   - Keep the `effect_preview -> effect_admission -> execute_grant ->
     effect_resolution` sequence visible.
   - Make clear that a preview is not authority and that host execution requires
     a matching durable grant.
   - Link to the governed-note walkthrough and normative host specification.

7. **Install, architecture, scope, and security**
   - Keep native archive links and checksum guidance, but move them after the
     first conceptual and runnable path.
   - Retain the workspace dependency direction and repository checks.
   - Keep exclusions, unsigned-binary warning, trace sensitivity warning, and
     residual malicious-host risk explicit.

The README remains English because the normative repository documentation and
existing public surface are English.

## GitHub Pages information architecture

The current Operational Amber visual system and `Authority before action.` hero
remain the brand core. The page gains orientation and execution sections rather
than being replaced by a generic documentation layout.

The page order becomes:

1. **Hero** — identity, release status, primary `Run the proof` action, secondary
   specification action, and the four-stage authority trace.
2. **Why ASTER** — a concise before/after comparison between an implicit
   model-to-tool path and ASTER's explicit governed path.
3. **Five-minute proof** — three numbered terminal steps for check, record, and
   replay, followed by the exact evidence a visitor should observe.
4. **What the trace proves** — retain the source/ledger visual, but connect each
   recorded event to the quickstart artifacts rather than presenting a purely
   illustrative ledger.
5. **Authority boundary** — retain and refine the four typed transitions.
6. **External host boundary** — visually distinguish preview, admission, grant,
   and resolution, including the durable-before-grant condition and host trust
   limitation.
7. **Release and download** — retain native downloads, checksum guidance, and
   release metadata.
8. **Honest scope and documentation** — retain implemented/excluded lists and
   improve links to the language spec, host spec, architecture, security model,
   examples, and release notes.

The navigation will expose `Why`, `Quickstart`, `Protocol`, `Download`, and
`Docs`. The primary hero action will move from immediate download to the
quickstart because understanding and proving the runtime is a stronger first
success than merely obtaining an archive. Download remains prominent in the
header flow and its own light section.

## Visual and interaction design

- Preserve the black, warm-white, graphite, and amber palette, large editorial
  typography, faint grid, and operational-console details.
- Introduce no stock imagery or decorative illustration; the trace, commands,
  hashes, and state transitions are the visual material.
- Use horizontal process bands and asymmetric terminal/evidence layouts rather
  than a grid of generic feature cards.
- Keep JavaScript progressive and optional. Reveal motion may continue, but all
  content must be visible and usable without JavaScript.
- A small copy control may be added to self-contained commands only if its
  no-JavaScript fallback remains the selectable command text and its accessible
  label/state are tested manually.
- Maintain clear focus indicators, skip navigation, semantic headings, reduced
  motion behavior, readable code overflow, and responsive layouts at existing
  desktop, tablet, and mobile breakpoints.
- Keep all assets local and the site directly publishable as static files.

## Accuracy and trust-boundary rules

Public wording must preserve these distinctions:

- `aster run` in the quickstart is fixture-backed and synthetic.
- `aster replay` receives no fixture or driver and performs no external effect.
- `aster host` exposes a bounded protocol to an external process; it does not
  give the model ambient authority.
- An `effect_preview` is not authorization to act.
- ASTER emits `execute_grant` only after admission, budget reservation, and
  durable continuation persistence.
- ASTER validates the returned resolution, but cannot prevent a malicious host
  from abusing authority it already owns or lying about provider behavior.
- ASTER v0.2.0 is an experimental reference processor, not a production
  platform.

Any example output must be generated from checked-in synthetic fixtures or
derived from an actual local run. No trace payload containing real personal or
secret data may be embedded in the README or site.

## Validation design

Implementation will update the static-site contract tests so the new structure
cannot silently regress. At minimum, automated checks will verify:

- stable anchors for `why`, `quickstart`, `protocol`, `download`, and `docs`;
- the three quickstart stages and fixture/replay distinction;
- the host protocol sequence and `execute_grant` wording;
- local-only site dependencies;
- keyboard focus and reduced-motion CSS contracts;
- all release asset names and checksum link;
- README links and repository documentation checks through the existing scripts.

The final verification will run:

```bash
./scripts/check.sh
```

It will also render the page locally at desktop and mobile widths, inspect it
with JavaScript enabled and disabled, run the documented quickstart from a clean
temporary output directory, compare record/replay state byte-for-byte, and
verify that the deployed Pages content matches the committed static site after
publication.

## Acceptance criteria

The work is complete when:

1. README and Pages share the approved six-part narrative without duplicating
   the same density of prose.
2. A repository user can copy the README quickstart and obtain the documented
   deterministic evidence without dirtying tracked files.
3. A Pages visitor can identify ASTER's problem, proof, authority path, host
   boundary, limitations, and next documentation link in one linear read.
4. Fixture-backed execution, replay, and external-host execution are visibly
   distinct.
5. No public claim exceeds the normative language, runtime, or host-protocol
   documentation.
6. Accessibility, responsive behavior, static-site independence, download
   integrity guidance, and full repository checks remain intact.
