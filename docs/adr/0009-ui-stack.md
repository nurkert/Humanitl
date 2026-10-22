# ADR-0009 · UI stack: Flutter with its own wrapper layer `packages/ui`

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02, revised twice on 2026-09-04 (HUM-035, then the project owner)

## Context

The interface of Humanitl is a control room, not a form. It shows a queue of
held requests with running countdowns, a virtualised history over tens of
thousands of rows, a diff editor for pseudonymisation, an embedded terminal,
JSON trees, hex views and a panel with live isolation checks. That is clearly
more than a widget library for business applications covers.

At the same time the result should look calm and not like a hacker tool
(principle 4): colour means state, never decoration; decisions happen inline
and never in a modal. The target platform is the Linux desktop under X11 and
Wayland, shipped as `.deb` and AppImage.

Two risks are visible from the start: a young widget library can break or be
left unmaintained, and a wrong choice in Sprint 0 would be expensive if it
only shows in the third sprint.

## Decision

> **Revised twice on 2026-09-04.** HUM-035 had replaced this section and
> rejected `shadcn_flutter`; the project owner reversed that on the same
> day. The section "Revised on 2026-09-04 by the project owner" at the very
> bottom applies: `shadcn_flutter` is adopted, pinned to 0.0.54 and only in
> `app/packages/ui`. Everything else in this section applies unchanged. The
> original wording stays in place so that the chain can be followed — including
> the last paragraph, which announces the review at the end of Sprint 2: it
> took place, twice.

The interface is Flutter (desktop/GTK). For the chrome — resizable panes,
command palette, sheet, toast, ContextMenu, menubar —
[shadcn_flutter](https://pub.dev/packages/shadcn_flutter) is used, with an
**exactly pinned version** and **fully encapsulated behind the local package
`app/packages/ui`**. No feature imports `shadcn_flutter` directly; all screens
use the wrappers `HButton`, `HPill`, `HBadge`, `HPanel`, `HRow`, `HModal`,
`HSheet` and the design tokens `HTokens`. An upgrade of the library is a
separate, planned undertaking: one upgrade day per sprint.

The data-heavy widgets explicitly do **not** come from shadcn but from
specialised packages:

| Task | Package |
|---|---|
| Virtualised table, tree | `two_dimensional_scrollables` |
| Code and body editor | `re_editor` |
| Terminal | `xterm2` |
| Diff for the pseudonymisation editor | `diff_match_patch` |

State: `flutter_riverpod` 3 with generator (`riverpod_annotation`,
`riverpod_generator`), models with `freezed` 4, `FlowEvent` as a sealed class.
Data flows in one direction: event stream → provider → widget. Widgets call
the `DaemonClient` only through provider methods, never directly.

**No WebView on Linux.** A domain preview is either a card from the bundled
catalogue or an image delivered by the daemon.

**One window with docked panes**, no multi-window. Multi-window is still
experimental in Flutter.

The choice of shadcn_flutter over [forui](https://pub.dev/packages/forui) is
**reviewed at the end of Sprint 2** (HUM-035), once the first real intercept
screen exists: confirm or switch.

## Rationale

> **Revised twice on 2026-09-04.** The paragraphs on `shadcn_flutter` and on
> the review at the end of Sprint 2 are answered. The rationale for the
> current state is in the section "Revised on 2026-09-04 by the project
> owner"; the section "Decision after Sprint 2" is history. Everything else
> applies unchanged.

For the Linux desktop, Flutter delivers complete rendering without a zoo of
GTK widgets, a very good animation layer (arrival and departure animations of
the queue cards are part of the design) and a golden-test tool with which a
visual concept can be secured against regressions. One window with docked
panes can be built without a fight.

shadcn_flutter covers exactly the chrome elements one would otherwise build
oneself and do worse: command palette, resizable, sheet, toast, menubar. The
value lies in the consistency of these elements with each other, not in
individual widgets.

The encapsulation behind `packages/ui` is the actual core of this decision.
It costs one day of work and makes the choice of library reversible: a switch
to forui, to another library or to our own widgets then touches one package,
not eight features. That is exactly why the review at the end of Sprint 2
(HUM-035) is a cheap promise and not an empty gesture.

Taking the data-heavy widgets from specialised packages follows the same logic
as ADR-0001: nothing of our own where something proven exists, but also no
contortions to get everything from one source. A virtualised table over tens
of thousands of rows is a problem of its own and is solved by a package that
does only that.

No WebView, because under Linux it would be the largest dependency and the
largest attack surface of the whole program — and because the UI would render
exactly the page the user is deciding about. This is the same consideration as
in ADR-0006 on the automatic domain preview.

riverpod with generator instead of hand-written providers, because the
provider names are fixed bindingly (`backlog/CONVENTIONS.md` 3.9) and the
generator turns typos and forgotten invalidations into compile errors. freezed
for models, because the Dart side is a mirror of the Rust core types and needs
value semantics plus exhaustive pattern matching.

## Rejected alternatives

> **Revised twice on 2026-09-04.** The item "forui instead of
> shadcn_flutter" is decided: `shadcn_flutter`. The list stands as history;
> the section "Revised on 2026-09-04 by the project owner" is authoritative.

- **GTK4 directly (Rust `gtk4-rs` or Vala).** Native, lightweight, good
  system integration. Loses on the number of specialised widgets: virtualised
  table, diff editor, terminal and animations would largely be built in-house,
  and golden tests are missing.
- **Tauri or Electron.** Brings a browser engine as a dependency — the same
  attack surface that doing without the WebView avoids. The wrong direction
  for a security tool, regardless of package size.
- **Qt/QML.** Technically viable, but licensing and packaging questions and a
  third ecosystem next to Rust and Dart.
- **forui instead of shadcn_flutter.** Seriously considered and not ruled out:
  the decision is reviewed at the end of Sprint 2 with a real screen as
  evidence. The wrapper layer makes the switch local.
- **Build all widgets ourselves.** Maximum control, but the effort flows into
  chrome instead of the domain. Contradicts principle 2.
- **Multi-window (queue and history separate).** Would be pleasant for the
  workflow, but is still experimental in Flutter. Postponed, not rejected.
- **Terminal as an external window.** Would have spared `xterm2`, but the
  status line "a request is being held" in the agent's terminal (ADR-0014) and
  reading along in the UI would then not be possible.

## Consequences

> **Revised twice on 2026-09-04.** The four points that concern
> `shadcn_flutter` — pinning, upgrade day, import ban, residual risk — apply
> again, in the version of the section "Revised on 2026-09-04 by the project
> owner": pin to 0.0.54, import only in `app/packages/ui`, enforced by
> `tools/check-deps.sh`. A successor ADR for forui is dropped because forui
> was not chosen. Goldens, the `DaemonClient` binding and `HTokens` apply
> unchanged.

- `app/packages/ui` is binding: a direct `shadcn_flutter` import in a feature
  is an architecture violation and is objected to in review.
- The version of shadcn_flutter is pinned exactly (for example `0.0.54`); an
  upgrade is a separate, tested step per sprint, not a side effect of a
  `pub upgrade`.
- Golden tests (`alchemist`) secure the visual states: queue row in three
  states, request card, action bar, domain panel known and unknown, isolation
  panel — per language, because DE and EN have texts of different length.
- Every screen is built against the interface `DaemonClient`, not against
  gRPC. Widget tests run against `FakeDaemonClient`, without a daemon.
- The design tokens (`HTokens`) are Dart constants from `BACKLOG.md` 5; state
  colours come exclusively through `FlowStateColor.of(state)`, so that colour
  is never used decoratively.
- HUM-035 is a real decision issue with two possible outcomes. If it comes out
  in favour of forui, this ADR is not overwritten but replaced by a successor
  ADR (status then `Superseded by`).
- A residual risk remains: shadcn_flutter is young. The buffer in HUM-061 is
  explicitly reserved for shadcn breakage too.

## Decision after Sprint 2 (2026-09-04)

> **This section is history.** It records how HUM-035 decided and what it
> measured against. Its result — no library — no longer applies; the project
> owner reversed it on the same day. The section "Revised on 2026-09-04 by
> the project owner" is authoritative. The measurements here remain valid and
> worth reading, because they describe what the library costs.

HUM-035 was to confirm or revise the choice between `shadcn_flutter` and
forui. On inspection the situation was a different one from the one the issue
was written for: `shadcn_flutter` was never entered in `app/pubspec.yaml`.
`app/packages/ui` has stood on plain `package:flutter/widgets.dart` since
Sprint 0 and has grown into a layer of its own in Sprint 1 and 2. Three
screens run on it — Intercept with action bar, Rules, History. The question is
therefore no longer "shadcn or forui" but "stay, adopt `shadcn_flutter` or
adopt forui".

**Decision: we stay with our own wrapper layer `app/packages/ui` on
`package:flutter/widgets.dart` and adopt neither `shadcn_flutter` nor forui.**
The encapsulation rule from the section "Decision" continues to apply
unchanged; only the name of the library behind the wrapper goes away.

### Rationale

A switch costs 8.5 to 11.5 person-days and saves 3.5 days of building it
ourselves. Both libraries require Flutter 3.47.0 at their head, our pin is at
3.44.0; adoption would therefore start with an upgrade that makes all 49
goldens need re-approval. In the last six releases there are 18 breaking items
for `shadcn_flutter` and 27 for forui, most of them on our surface. Our own
layer has zero workarounds, zero overrides in `HTokens`, around 30 contrast
assertions and around 200 key references; it reaches 88.3 % of the weighted
points, `shadcn_flutter` 48.3 % and forui 51.7 %. What is missing —
ContextMenu, date-time picker, vertical resizable — is three widgets, not a
library. The encapsulation stays so that the decision stays reversible.

### Decided without a prototype

The issue provided for a branch `spike/forui`, time box one day, and an
acceptance criterion for it. The branch was not built; that is a deliberate
deviation, recorded in `backlog/CONVENTIONS.md` 4.20 and brought into line in
`backlog/sprint-2.md` under HUM-035. The reason is the one condition that is
settled without a prototype and that a prototype would not have changed
either: both libraries require Flutter 3.47.0 at their head, the pin is at
3.44.0. The time box would have been used up by the upgrade before the first
line of the port was written.

**What that costs.** Four criteria — 2 (bugs), 4 (theming), 5 (keyboard and
focus) and 6 (performance), together 11 of the 20 weight points — remain an
estimate rather than a measurement for both libraries. The section "How
robust the result is" works out how far they could carry the result.

**How a misjudgement would show.** By three observations, each without new
effort: the in-house build needs more than three new chrome elements at once
in one sprint; maintaining the layer costs more than a fifth of the UI time
measured over a sprint; or one of the two libraries reaches 1.0 with a
stability promise and thereby cancels criterion 3. If one of these happens,
the spike is to be done after all, and then it measures exactly the four
criteria above.

### Where the numbers come from

All numbers from the repository were measured on 2026-09-04 in the working
tree, at the state of commit `2c26ad9` plus the not yet committed work on
Sprint 2 (the three screens and `packages/ui` are working tree at this point,
not commit; a pure commit anchor would not have captured them). This tree is
being built on in parallel, and the counts moved by a few percent within one
day. Where a quantity moves, it is therefore given as a rounded order of
magnitude; only what is a countable inventory is exact. Next to every
quantity is how it is counted:

- **Lines:** `wc -l` over `*.dart`, without `.dart_tool`.
- **Widgets:** classes in `app/packages/ui/lib/src/widgets/` that begin with
  `class H` at column 0, without the pure data class `HSegmentOption`.
- **Use per screen:** occurrences of the identifier with a word boundary in
  `app/lib/features/<screen>/`, distinct names counted, not calls.
- **Goldens:** PNG under `app/test/goldens/goldens/ci`. The directory
  `app/test/goldens/failures/` is in `app/.gitignore` and contains artefacts
  of failed runs; it does not count.
- **Test cases:** occurrences of `test(` and `testWidgets(` in `app/test` and
  `app/packages/ui/test`, and separately the `goldenTest(` calls.
- **Key references:** occurrences of `LogicalKeyboardKey` in the same two
  directories.
- **Token fields and read sites:** matches of the pattern
  `tokens\.[a-zA-Z]*\.[a-zA-Z0-9]*` in
  `app/packages/ui/lib/src/widgets/*.dart`; distinct matches are the fields,
  all matches the read sites.
- **Contrast assertions:** lines in `app/packages/ui/test/tokens_test.dart`
  with `greaterThanOrEqualTo(3.0)`, `greaterThanOrEqualTo(4.5)`,
  `lessThan(3.0)`, `lessThan(4.5)` or `closeTo(`.
- **Breaking items:** for `shadcn_flutter` the top-level bullet points under a
  heading `### Breaking` or `### Breaking Changes`, for forui the lines with
  the marker `**Breaking**`. Where one item ends and the next begins is a
  judgement call: if nested sub-items are counted too, `shadcn_flutter` comes
  to 26 instead of 18 (0.0.50 alone to 16 instead of 8) and forui 0.24.0 to
  about 23 instead of 13. That does not change the ranking.
- **External numbers** (stars, issues, commits, versions, licences, downloads)
  come from the GitHub and pub.dev APIs, retrieved on 2026-09-04.

The state on the day of measurement: `app/packages/ui` has around 9,900 lines
of Dart, of which around 6,400 are in `lib/` (18 widget files with around
3,300 lines, around 1,900 lines of tokens, 1,044 lines of gallery, 78 lines of
theme) and around 3,500 in `test/`.

### What the three screens really needed

The layer offers 21 widgets in `app/packages/ui` (plus `HTheme` as theme host
and `HGalleryPage`) and nine more in `app/lib/core/ui` that are earmarked as
handoff for the package.

| Screen | distinct widgets of the layer | the most frequent |
|---|---|---|
| Intercept | 15 | `HButton`, `HBadge`, `HGlyphIcon`, `HHairline`, `HRow`, `FocusRing` |
| Rules | 14 | `HButton`, `HGlyphIcon`, `HTextField`, `HHairline`, `HSegmented` |
| History | 17 | `HButton`, `HRow`, `HHairline`, `HDiagnosticCard`, `HWait` |

Together that is 24 distinct widgets from both directories. Exactly two have no
caller outside the gallery: `HPill` and `HPanel`. Both are wrappers that this
ADR originally named as the core of the layer; the action bar built the split
pill (`release_valve.dart`) itself instead, because it needs holding with a
fill. `HAnimatedFill` and `HSegment` do not appear in the table either, but
they are not dead entries: they are building blocks of `HButton`, `HSegmented`
and `HRow`.

### What is still missing

From `docs/UX.md` 9 and the handoff notes in `app/lib/core/ui`, against the
seven elements this ADR expected from a library. The day figures are orders of
magnitude from the scope of each task, not measured values.

| Element | State on 2026-09-04 | Effort to build ourselves |
|---|---|---|
| Resizable | `HResizablePanes` (`app/lib/core/ui`), horizontal, one caller | vertical axis 0.5 d |
| Command Palette | `app/lib/features/shell/widgets/command_palette.dart`, 243 lines | done |
| Sheet | `HSheet`, three calls | done |
| Segmented | `HSegmented` (six calls), `HChoiceChips` (one) | done |
| ContextMenu | missing; needed for the JSON tree ("Copy value", "Copy path", HUM-030) and the terminal (right-click Copy/Paste, HUM-042) | 1 d |
| Toast | is not built: `docs/UX.md` 2.11, 4.6 and 4.8 forbid it, BACKLOG.md 5 lists "toast spam" as an anti-pattern | not applicable |
| Menubar | is not needed: icon rail and command palette are the two ways, no document asks for a menu bar | not applicable |

That leaves **five needed** of the seven elements of the ADR: Resizable,
Command Palette, Sheet, Segmented, ContextMenu. Toast and Menubar drop out
equally for all candidates, not only for the in-house build; they therefore
count in no column in criterion 1. A data table is not in the ADR's list at
all and counts nowhere either: `history_table.dart` solves it with around 890
lines — `ListView.builder` with a known `itemExtent`, a memoised build per
flow, one `Semantics` node per row instead of eleven.

That leaves 3.5 person-days of building it ourselves: ContextMenu 1 d,
date-time picker 2 d (time-range filter of the audit screen, Sprint 4),
vertical axis of the resizable 0.5 d. This number is the reference value for
everything that follows.

### The points scale

The issue prescribes 0 to 3 points per criterion, but no meaning for them.
Without thresholds every single rating would be a prose judgement, which is
why they are given here. They apply equally to all three columns.

| Criterion | 3 points | 2 | 1 | 0 |
|---|---|---|---|---|
| 1 Components (of the five needed, supplied itself) | five | four | two or three | at most one |
| 2 Bugs hit | none, and backed by our own tests | none recorded, but unproven | at least one open defect in a component we rely on | more than three, or one without a workaround |
| 3 Breaking items in six releases | zero | 1 to 5 | 6 to 20 | more than 20 |
| 4 Theming fit | zero overrides, `HTokens` the only colour source, bound by tests | individual overrides, one source | a second colour source needed, or not decidable without a prototype | the library forces its own colour scale |
| 5 Keyboard and focus | measured in the repository, overlays covered by tests | documented, not measured | gaps documented | no focus handling |
| 6 Performance history | measured with DevTools over 10,000 rows, under 16 ms | method proven, not measured | would be a step back from today | measurably over 16 ms |
| 7 Community and maintenance | over 50 commits in 90 days from at least three accounts | at least 20 commits from at least two accounts | fewer, or everything on one account | no commit in 90 days |
| 8 Licence | OSI-approved and compatible with GPL-3.0-only | compatible with conditions | unclear | incompatible |
| 9 Switching effort | zero days | up to 3 days | up to 8 days | over 8 days |

### The matrix

Criteria and weights from `backlog/sprint-2.md`, HUM-035. Maximum score 60.
Versions as of 2026-09-04: `shadcn_flutter` 0.0.54 (released 2026-08-27),
forui 0.26.0 (released 2026-08-24), our own layer as described above.

| Criterion | Wt. | In-house | shadcn 0.0.54 | forui 0.26.0 |
|---|---:|---:|---:|---:|
| 1 Components (five needed) | 3 | 2 | 3 | 2 |
| 2 Bugs hit in Sprint 1–2 | 3 | 3 | 1 | 2 |
| 3 Breaking changes in the last six releases | 2 | 3 | 1 | 0 |
| 4 Theming fit to BACKLOG.md 5 | 3 | 3 | 1 | 1 |
| 5 Keyboard and focus | 3 | 3 | 2 | 2 |
| 6 Performance history | 2 | 2 | 1 | 2 |
| 7 Community and maintenance | 1 | 1 | 1 | 3 |
| 8 Licence | 1 | 3 | 3 | 3 |
| 9 Switching effort | 2 | 3 | 0 | 0 |
| **Weighted sum** | **20** | **53** | **29** | **31** |
| **Share** | | **88.3 %** | **48.3 %** | **51.7 %** |

The evidence, criterion by criterion:

**1 Components (weight 3).** The five needed elements are counted, for every
column by the same rule: what the column supplies itself counts; what it does
not supply would remain an in-house build. `shadcn_flutter` supplies all five
— `control/command.dart`, `layout/resizable.dart` (ten occurrences of
`Axis.vertical`), `overlay/drawer.dart`, `form/multiple_choice.dart`,
`menu/context_menu.dart` — 5 of 5, 3 points. forui supplies Resizable, Sheet,
ContextMenu and, with `tabs`/`select_group`, a substitute for Segmented, but
no command palette — 4 of 5, 2 points. The in-house build has Command Palette,
Sheet, Segmented and the horizontal Resizable, the ContextMenu is missing — 4
of 5, 2 points. A more generous reading that lets forui off for the missing
Command would bring 3 points there and 34 of 60; the ranking stays.

**2 Bugs hit (weight 3).** `grep -rn "WORKAROUND\|TODO\|FIXME\|HACK"
app/packages/ui` finds zero matches. This is backed by over 500 calls of
`test(` and `testWidgets(`, plus 26 `goldenTest(` in six files with 49 stored
golden images — 3 points. For both libraries the number of bugs hit is zero,
because neither was ever integrated; that is no evidence of quality, and the
score here is an estimate, not a measurement. The only recorded evidence is in
`backlog/sprint-1.md`, HUM-020: `ResizablePane` in `shadcn_flutter` 0.0.54 has
the open defects #427 and #428, with `multi_split_view` as a workaround — and
that hits exactly the component that carries the intercept layout. Hence 1
point for `shadcn_flutter` and 2 for forui, against which nothing comparable
is noted.

**3 Breaking changes (weight 2).** From the changelogs, as of 2026-09-04, by
the counting rule given above. `shadcn_flutter` 0.0.49 to 0.0.54: 18 breaking
items, eight of them in 0.0.50 (the selection model of `NavigationBar`
switches from index to value, several parameters are dropped) and ten in
0.0.54 (Material and Cupertino removed from the package, `ShadcnApp` no longer
sets Material ancestors, lower bound Flutter 3.47.0). Eleven of these would
affect us: the navigation is our icon rail, and the Material removal hits
every app — 1 point by the threshold "6 to 20". forui 0.24.0 to 0.26.0,
including the three patch versions exactly six releases: 27 breaking items,
13 of them in 0.24.0 (`FThemes` removed, all predefined colour schemes except
`FColors.neutralLight` and `FColors.neutralDark` removed,
`FBadgeContentStyle`, `FCardContentStyle`, `FDialogContentStyle` removed,
`FCard.raw` and `FDialog.raw` renamed), ten in 0.25.0 and four in 0.26.0
(switch to `material_ui` and `cupertino_ui`, `forui_assets` is now called
`forui_lucide`, lower bound Flutter 3.47.0). That hits badge, card, dialog,
theme and text field, exactly our surface — 0 points by the threshold "more
than 20". The in-house build has no upstream and therefore zero — 3 points.

In addition, a finding that holds independently of the score: **both
libraries require Flutter ≥ 3.47.0 and Dart ≥ 3.13.0 in their current
version.** The only pin the CI job reads is `app/.fvmrc` with `3.44.0`
(`.github/actions/setup-flutter`); `app/pubspec.yaml` carries the lower bound
additionally as a constraint of its own. The last version of each that runs on
our state is `shadcn_flutter` 0.0.53 (2026-07-14) and forui 0.25.0
(2026-08-02). Adoption at the head therefore starts with a Flutter upgrade
that makes all 49 goldens need re-approval.

**4 Theming fit (weight 3).** The criterion measures `!important`-like
overrides in `HTokens`: zero. The widget bodies read 30 distinct token fields
at around 100 places through around 20 `HTheme.of(context)` calls; the only
seven colour literals in the bodies are `Color(0x00000000)`, that is,
transparent. State colours come exclusively from `FlowStateColor` and
`HStateColors`, with separate surface and text variants, and around 30
contrast assertions in `tokens_test.dart` bind them to 3:1 for surfaces and
4.5:1 for text on both scales — 3 points. In 0.24.0 forui removed all
predefined colour schemes except two and points to a generator
(`dart run forui theme create`); a generated theme would be a second colour
source next to `HTokens`, to be kept in sync by hand — 1 point, and this one
can be justified without a prototype. `shadcn_flutter` brings a `ColorScheme`
with radius and typography; our 30 fields would have to be laid over it as an
adapter, and what that does to the measured contrasts is **not** decidable
without a prototype — the one point there falls under the second half of the
threshold and is an estimate.

**5 Keyboard and focus (weight 3).** Around 200 occurrences of
`LogicalKeyboardKey` in 18 test files. `HModal` carries `FocusScope` and binds
`Escape` via `DismissIntent`; ten of the 21 widgets have their own focus
handling; `HFocusRing` draws the two-pixel accent ring outside the control,
`HFocusRing.inline` on the edge of the split pill. This closes items 14, 16
and 17 from `docs/UX.md` 9 — 3 points. For both libraries there is no
measurement, because the prototype branch was not built; 2 points each by the
threshold "documented, not measured".

**6 Performance history (weight 2).** `history_table.dart` is a
`ListView.builder` with a known `itemExtent`, a memoised build per flow and
one `Semantics` node per row instead of eleven. The DevTools measurement over
10,000 rows that the criterion requires is still outstanding — 2 points
instead of 3. `shadcn_flutter` gets 1 point: its `layout/table.dart` would be
a step back in the `Semantics` nodes compared with what runs today. forui
likewise gets 2 points, not 1: it brings no table, our method would remain
unchanged, and where nothing changes, the number must not fall.

**7 Community and maintenance (weight 1).** Numbers from the GitHub and
pub.dev APIs of 2026-09-04. `sunarya-thito/shadcn_flutter`: 936 stars, 26 open
issues, last push 2026-08-27, 28 commits since 2026-06-06, all 28 from a
single account; around 12,300 downloads in 30 days, 469 likes, 160 of 160
points — 1 point by the threshold "everything on one account".
`duobaseio/forui`: 2,327 stars, 48 open issues, last push 2026-09-03, 95
commits since 2026-06-06 from five accounts (54, 23 by Renovate, 16, twice 1);
around 23,900 downloads in 30 days, 432 likes, 160 of 160 points — 3 points.
The in-house build has no outside community: one account, so 1 point by the
same threshold. This is the only row in which the in-house build clearly
loses.

**8 Licence (weight 1).** `shadcn_flutter` is under BSD-3-Clause, forui under
MIT with OFL-1.1 for the bundled fonts; both are OSI-approved and compatible
with GPL-3.0-only. Our own layer is already GPL-3.0-only. 3 points each; the
criterion separates nothing.

**9 Switching effort (weight 2).** For the in-house build zero days — 3
points. For both libraries 8.5 to 11.5 person-days according to the breakdown
below, so above the threshold of eight days — 0 points.

### The decision rule

The issue says: "stay if shadcn ≥ 75 % of the weighted points and no
criterion with weight 3 below 1 point; otherwise switch." The rule was written
when `shadcn_flutter` was supposed to be the incumbent. It comes out the same
in both readings:

- Applied to the letter, to `shadcn_flutter`: 48.3 % is below 75 %, so do not
  confirm.
- Applied to the actual incumbent, our own layer: 88.3 % is above 75 %, and
  the weakest criterion with weight 3 has 2 points (components), so stay.

### How robust the result is

Four of the nine criteria are **not measured** for the libraries, because the
prototype branch was not built: 2 (bugs), 4 (theming), 5 (keyboard and focus)
and 6 (performance). Together they weigh 11 of the 20 weight points. That has
to be stated openly, otherwise the matrix checks nothing and only justifies.

If these four are set to the full score for the libraries — that is, the
unproven case most favourable to them —, `shadcn_flutter` comes to 48 of 60
(80.0 %) and forui to 45 of 60 (75.0 %). **Both thereby reach the threshold of
the issue.** The in-house build stays at 53 of 60 and remains ahead, but the
rule alone would no longer carry the decision in this case.

What carries it then are the rows that are settled without a prototype:
criterion 3 from the changelogs (18 and 27 breaking items in six releases),
criteria 7 and 8 from the public registries, criterion 9 from the scope of our
own code, and the Flutter lower bound 3.47.0, which has no score and
nevertheless makes every switch two to three days more expensive. For
criterion 4 one can additionally argue for forui without a prototype — a
generated theme would be a second colour source next to `HTokens` —, for
`shadcn_flutter` one cannot; there the 1 remains an estimate.

The cross-check in the other direction: if criterion 9 is struck entirely,
because an incumbent is systematically at an advantage there, 54 possible
points remain, and the in-house build comes to 47 (87.0 %) against 29
(53.7 %) and 31 (57.4 %). So the decision tips neither without the incumbent
bonus nor in the most favourable unproven case to a clear result for a
library. It tips into a situation in which a challenger narrowly reaches the
threshold and demands 8.5 to 11.5 days for it, while the existing layer costs
3.5 days of remaining work. That is the sentence on which the decision really
hangs — not the percentage.

### What a switch would have cost

Should the decision fall the other way later after all, this is the
breakdown. No issue is created from it; HUM-035b is dropped. The day figures
are orders of magnitude from the scope of the affected files, not measured
values; the largest item assumes around 1,100 lines of widget body per day
with unchanged signatures.

1. Raise the Flutter pin from 3.44.0 to 3.47.0 (`app/.fvmrc`,
   `app/pubspec.yaml`), switch to `material_ui` and `cupertino_ui`, re-approve
   49 goldens — 2 to 3 days.
2. Switch `HTheme` to the library's theme, keep `HTokens` as the only colour
   source — 1 to 2 days.
3. Convert the 18 widget bodies (around 3,300 lines), signatures unchanged, so
   that no screen notices — 3 days.
4. Re-approve goldens and check the contrast assertions against the new
   surfaces — 1 to 2 days.
5. Bring keyboard and focus in overlays into line with the key tests — 1 day.
6. Register the library's localisation (`ShadcnLocalizations`; otherwise the
   DatePicker, for example, crashes under `de`, `backlog/sprint-4.md` HUM-052)
   — 0.5 days.

Total 8.5 to 11.5 person-days, without the recurring upgrade day per sprint.

### Consequences of this decision

- `app/packages/ui` stays on `package:flutter/widgets.dart`. Neither
  `shadcn_flutter` nor forui is in `app/pubspec.yaml`; an entry would be an
  architecture violation and is objected to in review.
- The encapsulation remains and keeps its purpose: features import
  `lib/core/ui/ui.dart`, never a third-party package. It no longer protects
  against a library, but keeps the door open for a later one.
- The upgrade day per sprint is dropped. The buffer HUM-061 is no longer
  reserved for shadcn breakage.
- Three issues build the rest, each separately, numbers when Sprint 3 is cut:
  a ContextMenu for JSON tree and terminal (1 d), a date-time picker for the
  time-range filter (2 d), the vertical axis for `HResizablePanes` (0.5 d).
- The nine widgets from `app/lib/core/ui` that are earmarked as handoff
  (`HResizablePanes`, `HCollapsible`, `HDiagnosticCard`, `HoverLabel`,
  `HoldToConfirm`, `FixControl`, `FocusRing`, `ShellGlyphIcon`,
  `SectionPlaceholder`) move to `app/packages/ui`. `FocusRing` there and
  `HFocusRing` in the package are the same thing twice; one stays after the
  move.
- `HPill` and `HPanel` have no caller outside the gallery. Either they get one
  or they go; a wrapper without users is ballast that moves along with every
  rebuild.
- The DevTools measurement of the history over 10,000 rows is still
  outstanding and belongs to HUM-054.
- The data-heavy packages from the section "Decision" remain untouched;
  `history_table.dart` shows, however, that `two_dimensional_scrollables` is
  not needed for the history. Whether it comes for the JSON tree is decided by
  HUM-030, not by this ADR.
- The sentence in the section "Consequences" that announces a successor ADR
  in case of a decision in favour of forui is thereby settled: the decision
  favours no library, and the rationale for that is below under "Rejected
  alternatives of this revision".
- This decision is reopened if one of the three conditions from "Decided
  without a prototype" occurs.

**Places that still contradict this decision.** They are listed here so that
no one takes them as valid:

- `backlog/sprint-0.md` 1749 (HUM-008) and `backlog/sprint-4.md` 1476
  (HUM-054) were acceptance criteria that required the opposite of this rule
  ("shadcn_flutter pinned exactly", "shadcn_flutter bump to the next
  version"). Both have been brought into line with this ADR, minimally and
  without touching the rest of the boxes.
- `backlog/sprint-2.md` (HUM-035) has been brought into line in its approach,
  its steps and the acceptance criterion on the spike branch; the wrong path
  `docs/adr/0009-ui-kit.md` is corrected to `0009-ui-stack.md`.
- `backlog/sprint-0.md` (HUM-008, plus the Flutter version 3.47.0 in lines
  115 and 208) and `backlog/sprint-1.md` (HUM-019, HUM-020) continue to
  describe `shadcn_flutter` as settled in context, specification and pitfalls.
  That stays as history: these are the assignments under which the work was
  done, and this ADR is the answer to them. The pitfall about `ResizablePane`
  in HUM-020 additionally stays as evidence for criterion 2.
- `backlog/sprint-4.md` (HUM-052 pitfall on localisation, HUM-054 reference to
  shadcn upgrades) and `backlog/sprint-5.md` (risk table with shadcn breakage,
  lines 709, 728, 748, 751; `find.byType(Dialog)` in HUM-058, lines 398 and
  415) concern sprints that have not been cut yet. They are brought into line
  when the respective sprint is cut, not before; `BACKLOG.md` 10 has already
  been switched over and takes precedence over the sprint files.

### Rejected alternatives of this revision

- **Confirm `shadcn_flutter`.** 48.3 % of the weighted points, thus below the
  threshold the issue itself set; 80.0 % in the most favourable unproven case.
  The broadest set of components of all, but maintenance hangs on one person,
  adoption starts with a Flutter upgrade, and the component that carries our
  intercept layout has two open defects.
- **Switch to forui.** 51.7 %, 75.0 % in the most favourable unproven case:
  the best maintenance in the field, but 27 breaking items in six releases, no
  substitute for the command palette, and a generated theme as a second colour
  source next to `HTokens`.
- **Pull individual components from a library** (for example only Command and
  ContextMenu). Brings the whole dependency, the whole theme and the whole
  Flutter lower bound for two widgets that together cost 1 day to build
  ourselves.
- **One specialised package per gap** (`multi_split_view` for the resizable
  and similar). Remains allowed and is no contradiction: the section
  "Decision" takes the data-heavy widgets from exactly such packages. For the
  three open gaps it is not worth it, because each is under two days and each
  has to take state colour and focus ring from our tokens.
- **Build the spike after all and then decide.** The path the issue provided
  for. Rejected with the rationale in the section "Decided without a
  prototype": the time box of one day would have been used up by the Flutter
  upgrade before the first line of the port, and the condition on which the
  decision hangs is settled without it. The price for this is in the same
  line: four criteria remain an estimate.
- **Replace this ADR with a successor.** The section "Consequences" provides
  for that in case HUM-035 comes out in favour of forui. It comes out in
  favour of no library, and everything else in this ADR — Flutter, the
  encapsulation behind `packages/ui`, no WebView, one window with docked
  panes, riverpod with generator, freezed, the specialised packages — stands
  unchanged. A successor ADR would have repeated 90 % of the text to strike a
  package name.

## Revised on 2026-09-04 by the project owner

The section "Decision after Sprint 2" no longer applies. It decided not to
adopt `shadcn_flutter` and to keep our own widget layer. The project owner
overruled that on the same day: **Flutter runs on the latest stable version at
any time, and `shadcn_flutter` is adopted.**

Why the previous decision was wrong, regardless of the result: in its original
version this ADR named `shadcn_flutter` as settled. That was a requirement of
the project owner, not a suggestion. HUM-035 revised it without asking — and
thereby passed off as a review what was in truth the reversal of someone
else's decision. A decision that reverses something the project owner set goes
to them beforehand as a question, not afterwards as a result.

### What applies now

Flutter is at **3.47.2** with Dart **3.13.2**, pinned in `app/.fvmrc`, and
from now on follows the latest stable state. If a package blocks an upgrade,
the package is named and resolved instead of postponing the upgrade. The
switch cost: `intl` from 0.20.2 to 0.20.3, because `flutter_localizations` in
3.47.2 replaces its exact pin with `^0.20.3`; `freezed` to 4.0.1,
`riverpod_generator` to 4.0.9, `json_serializable` to 6.14.1 and
`build_runner` to 2.16.1. The caps in detail, measured on the pubspecs in the
cache: `riverpod_generator` 4.0.3 takes `analyzer ^9.0.0`, and `analyzer`
9.0.0 knows language version 3.11, so two behind Dart 3.13; `freezed` 3.2.5
and `json_serializable` 6.13.0 take `analyzer >=9 <11`, and 10.1.0 knows 3.12,
so one behind. `build_runner` 2.15.1 would have sufficed with
`analyzer >=8 <14`; its upgrade is incidental, not forced. Also carried along
were the runtime packages `flutter_riverpod` to 3.4.3 and
`riverpod_annotation` to 4.0.7, as well as `json_annotation` to 4.12.0, which
`json_serializable` prescribes exactly. The rest of the code
was untouched: 3.45 and 3.46 carry no breaking changes, and the three from
3.47 do not affect us.

`shadcn_flutter` is pinned exactly to 0.0.54 and is in
`app/packages/ui/pubspec.yaml`, not in `app/pubspec.yaml`. So the seam
remains: no feature imports the library, `tools/check-deps.sh` objects to
every `package:shadcn_flutter` outside `app/packages/ui`, and what a screen
sees are still the `H*` widgets.

It brings thirteen further packages, and it bundles **8.3 MB of its own
assets** into every build: 4.4 MB of Geist fonts in 33 weights, 1.25 MB of
icon fonts, and 3.3 MB of country flags from `country_flags`. Flutter takes a
package's fonts along regardless of whether anyone imports them, so the weight
is incurred even if we use only part of the library. For a desktop
application that is acceptable; it still belongs on record here, because
`backlog/CONVENTIONS.md` 4.11 said until today "fonts are not
bundled". The lock files have been versioned since this
upgrade: the thirteen packages come with caret ranges, and without a lock a
release of one of them would change the build without a single commit.

### What the library does not bring

Before adoption, the 165 files of the package were read in source, not the
README. The result belongs here, because it draws the line up to which one can
rely on it:

- It checks a contrast ratio nowhere. There is a heuristic over brightness
  (`getContrastColor` in `lib/src/util.dart`) that picks a light or dark
  counter-colour for a colour, but does not compute a ratio. The destructive
  button in the light theme therefore paints a fixed `Colors.white` on a red at
  half opacity, that is 1.97:1, with a comment next to it that admits this.
- The entire library has three `Semantics` nodes, none of them on button,
  checkbox, tabs or badge.
- `AnimationBehavior` does not occur. Every one of its animation durations
  collapses to five percent as soon as the system reports reduced motion; what
  hangs on a `Timer`, such as the delay of the hover card, keeps its time.
- There is no hold-to-confirm, no minimum hit area, and on the desktop no
  pressed state: the only feedback hangs on a switch that is turned off under
  Linux.

It is therefore no Radix: it supplies appearance and chrome, not the
behaviour layer underneath. That is no objection to adopting it, but the
description of what `packages/ui` still has to do itself.

### What applies in the MVP and what comes back for 1.0.0

The project owner took accessibility as a programme out of the MVP: first
prove that it works and can be good. Screen reader semantics, WCAG numbers as
a test gate and the check at double text scaling are thereby postponed until
1.0.0.

Four things stay nevertheless, because they only look like accessibility:

1. **The hold duration keeps its time.** If the system reports reduced motion,
   Flutter cuts every animation duration to five percent. For us, the safeguard
   that prevents a click from sending a request out hangs on it; measured, 60
   milliseconds sufficed. That is a security defect that happens to run through
   the same setting.
2. **A lower bound for legibility.** 1.97:1 is not "not accessible" but
   illegible. The bound stays, but as a design question and considerably
   looser than the 4.5:1 that applied until today.
3. **The hit areas of the decision.** At least 28 × 28 px in general, at least
   32 × 120 px for Allow and Block. The library knows no minimum hit area;
   whoever slips on an irreversible action does not have an accessibility
   problem but a sent request.
4. **A visible pressed state.** A control that does nothing on a click feels
   broken.

### What adoption cost

Exactly one assertion has been relaxed, and it is named in `docs/UX.md` 8:

**The focus ring of `HTextField` animates and reserves no space.**
`docs/UX.md` 2.9 requires a focus ring within one frame, because one that
fades in reads as input lag. The library's `TextField` hard-wires its
`FocusOutline`, without a switch; it runs a value builder over 150 ms, scales
the border width and draws beyond its own edge. Two rings on top of each other
would be one too many, so it is theirs. Its dimensions stay ours — two pixels
of accent, two pixels of gap, set through their `FocusOutlineTheme` from
`HFocusRingMetrics` —, and every other control still carries `HFocusRing`,
which reserves its space and does not animate. No contrast value has been
lowered.

Four traps have been caught instead of taken over, each with a test that turns
red without the safeguard:

- Their `TextField` reports **every** change of the `TextEditingController` to
  `onChanged`, including one that a widget assigns itself. Flutter documents
  the opposite for `onChanged`, and the screens are written under that
  contract: the rule editor fills its fields on opening, and the report would
  run back into the provider in the middle of the build.
- Their `Clickable` binds four arrow keys to `DirectionalFocusIntent` under
  every control. The innermost `Shortcuts` wins, so `ArrowUp` and `ArrowDown`
  no longer reached the screen as long as a row or a button held the focus —
  and the queue is a single focus stop with navigation inside it
  (`docs/UX.md` 5.2).
- Their actions exist as `CallbackAction<Intent>`.
  `Actions.maybeFind<ActivateIntent>` cannot cast them: in the debug build it
  aborts in an assertion, in the release build it silently returns `null`.
  Exactly this question decides whether a screen key fires while a control
  holds the focus (flutter/flutter#180871, `docs/UX.md` 5.2).
- Their `Checkbox` computes `enabled ?? onChanged != null`. Whoever, like us,
  makes the decision outside the component and therefore leaves `onChanged`
  empty gets `false` — and with it the branch that paints the border in
  `muted`: **every** unticked box would stand in the colour of a dead one. The
  `enabled` is therefore fixed to `true`, and the four colours come from the
  tokens.

## Affected issues

`HUM-008` (design tokens and `packages/ui`), `HUM-019` (Flutter shell),
`HUM-020` (intercept screen v1), `HUM-030` (body views), `HUM-032` (history
screen with virtualised table), `HUM-042` (terminal with xterm2), `HUM-035`
(confirmation or revision of this decision), `HUM-054` (golden and widget
tests), `HUM-061` (buffer).
