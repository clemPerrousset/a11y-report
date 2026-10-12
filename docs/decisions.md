# Decisions

Choices made without explicit validation, as required by the specification
(date, context, choice, rejected alternative). Newest last.

## 2026-10-12 — The tool captures screens itself (adb), not only pre-captured runs

- **Context**: the spec's MVP input is a run folder captured beforehand. The product owner wants a
  single command run at the root of an Android project that "does everything".
- **Choice**: `a11y-report` (default command `scan`) detects the Gradle project, finds `adb`
  (PATH, `ANDROID_HOME`, `local.properties`, default SDK paths), and guides the user screen by
  screen: navigate on the phone, press Enter, the tool runs `uiautomator dump` + `screencap` and
  writes the run folder (`a11y-report/run/`) in the spec's format. The analysis engine is
  unchanged: it still only reads run folders, so `analyze <RUN_DIR>` keeps working for CI and
  for captures produced by other tools. A non-interactive `capture <name>` command exists for
  scripts and UI tests.
- **Rejected**: an automatic crawler that taps every clickable element. On a real account it can
  post, delete or send things, and it is out of the MVP scope (automated capture is roadmap #2).

## 2026-10-12 — Names: binary `a11y-report`, crates `a11y-report-*`

- **Context**: `a11y-engine` was a provisional name; the repository is `a11y-report`. The crate
  name `a11y-report` is already taken on crates.io by an unrelated project.
- **Choice**: binary `a11y-report`, published as the `a11y-report-cli` package. Library crates are
  `a11y-report-core`, `-android`, `-rules`, `-render` (the spec's `a11y-report` crate is named
  `a11y-report-render` to avoid confusion with the binary).

## 2026-10-12 — Third output: Markdown briefs for AI agents

- **Context**: the report must have a human first page and a per-screen detail meant to guide AI
  coding agents.
- **Choice**: the HTML report keeps the spec structure (summary first). In addition,
  `ai/README.md` (instructions for agents, summary, fix recipes per rule) and `ai/<screen>.md`
  (one block per issue: element, evidence, likely source, fix, acceptance test) are rendered from
  the same report object. JSON stays the source of truth.

## 2026-10-12 — Source hints

- **Context**: to be actionable for an agent, an issue should point at code.
- **Choice**: when the tool runs inside an Android project, a source index is built once
  (layouts, `strings.xml`, Kotlin/Java) and each finding gets up to 5 `source_hints`
  (`layout_id`, `compose_test_tag`, `view_binding`, `code_id_reference`, `layout_string`,
  `string_resource`, `literal_text`), ordered by reliability. Best effort, deterministic.
- **Rejected**: parsing Kotlin/Compose ASTs (heavy, slow, not needed for a hint).

## 2026-10-12 — Output language: English by default, `--lang fr`

- **Context**: the spec asks for French HTML; the product owner wants English by default with a
  French option.
- **Choice**: every human string (HTML, Markdown, stdout, criterion titles, manual checks,
  limitations) is localized; labels live in `a11y-report-render/src/i18n.rs` and in the criteria
  catalog. The JSON carries `lang` and localized titles; ids and enums are language-independent,
  so reports in different languages can be diffed.

## 2026-10-12 — One catalog entry per WCAG criterion, with its EN 301 549 clause

- **Context**: the spec example lists both `en301549-11.1.4.3` and `wcag-1.4.3` on a finding.
  Counting both would double every criterion in the table and in the rate.
- **Choice**: criteria ids are `wcag-x.y.z`; each entry carries its EN 301 549 v3.2.1 chapter 11
  clause (`reference`: `WCAG 1.4.3 · EN 301 549 11.1.4.3`). WCAG 2.5.8 has no EN clause.
  The non-normative Android 48 dp guideline is `android-touch-target`, shown in the table but not
  counted in the automatic conformity rate.

## 2026-10-12 — Rule trait returns applicability

- **Context**: "a criterion is applicable if at least one concerned element exists" cannot be
  derived from findings alone.
- **Choice**: `Rule::check(&self, ctx, out: &mut RuleOutput)` where `RuleOutput` holds the findings
  and the set of criteria for which elements were evaluated. A rule that needs a screenshot on a
  screen without one marks its criteria "not verified"; if no screen could verify a criterion, its
  status is `manual`.

## 2026-10-12 — Exit code precedence

- **Context**: a run can both contain an invalid screen and exceed `--fail-on`.
- **Choice**: invalid input wins: code 2 (the report is still written). Then 1 for the threshold.
  Low-confidence and `info` findings never count for `--fail-on`, consistently with criteria.

## 2026-10-12 — Negative bounds are an input error

- **Context**: the spec says out-of-screen bounds must be clamped, but its robustness tests expect
  negative bounds to produce code 2.
- **Choice**: negative coordinates make the screen invalid (listed in `run.errors`, code 2); bounds
  beyond the right/bottom edges are clamped and flagged `clipped_by_screen`; zero-area nodes are
  dropped with their subtree.

## 2026-10-12 — Element keys: runs of digits become a single `#`

- **Context**: the spec replaces each digit by `#`, so "9 items" and "10 items" get different keys.
- **Choice**: every run of digits becomes one `#` ("Delete 2 plants" ≡ "Delete 15 plants").

## 2026-10-12 — Target size: nodes cut by a scrolling container

- **Context**: uiautomator reports the visible part of a node; a list item half scrolled out looks
  small.
- **Choice**: a node touching an edge of a scrollable ancestor without spanning it on that axis is
  skipped (possible false negatives on items flush with a list edge, never false positives).

## 2026-10-12 — Contrast details

- Busy backgrounds (dominant ring bucket < 60 %) only produce a finding when the measured ratio is
  below the threshold; it is then `info` / `confidence: low`.
- Large text needs an estimated size ≥ 28 dp (3:1); 20–28 dp keeps 4.5:1 with `confidence: medium`.
- The suggested colour (same hue and saturation, lightness moved by 0.5 % steps) is stored in the
  evidence so the HTML adds no business logic.

## 2026-10-12 — Severity overrides and ignores

- `[rules.<id>] severity = "…"` replaces the severity of every non-`info` finding of that rule.
- `[ignore] resource_ids` globs match the full resource id (`com.app:id/debug_panel`).
- Ignored findings stay in the JSON with `ignored: true` and are excluded from counts, criteria
  and exit codes.

## 2026-10-12 — Other windows are filtered by package

- **Context**: dumps include the status bar or the keyboard, which are not the app.
- **Choice**: `run.json` may carry `package`; top-level windows of other packages are dropped.
  The capture command fills it from the Gradle `applicationId` (or `--package`).

## 2026-10-12 — Extra crates and tools

No dependency outside the spec's list was added. Release automation uses GitHub Actions only;
install scripts are plain shell/PowerShell.
