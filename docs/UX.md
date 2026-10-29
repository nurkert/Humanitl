# Humanitl — UX guidelines

Applies to: Humanitl 0.1

> Purpose: the interface should make a decision so easy that a human can make it for hours without tiring, and so hard that they cannot make it by accident. This document fixes how motion, colour, text and keyboard work together, so that every screen from HUM-028 to HUM-034 speaks the same language. It complements `BACKLOG.md` section 5 (direction "Airlock"), `docs/ARCHITECTURE.md` (layers) and `app/packages/ui` (values).

In case of conflict: `packages/ui` wins for values, this document for their use. Where a rule here would need a token that does not exist, it stands in section 9. Tokens are never changed in passing.

## 1. Why this document exists

A human decides here about someone else's network traffic, under a running clock, for hours. Every movement, every colour and every word on the screen costs attention that they need for the next decision. This document fixes what this attention may be spent on, and makes every such decision checkable.

It stands next to `backlog/CONVENTIONS.md` 4.13, which records why this program must radiate trust and how that is measured: never claim more than is proven, precision where it counts, restraint, predictability, no dark patterns in either direction, errors with why and fix, visible craft. What stands here is the design execution of that promise; where both documents touch the same thing, 4.13 wins on the statement and this document on the execution.

### 1.1 What the saved attention is spent on

Most rules here forbid something. That is half the work; the other half is to spend the freed budget in one place per screen instead of saving it. A program that only saves becomes a clean derivative without a hand of its own.

Every screen therefore has one named moment at which it may be beautiful. Exactly one:

| Screen | The moment | How you recognise it |
|---|---|---|
| Intercept | the airlock chamber at the instant of the decision | The rail of the decided row sweeps to full saturation and is, for 200 ms, the only saturated surface in the whole pane. Everything else stays neutral, so that this one sweep means something. |
| Rules | the order as a readable chain | The rules stand as sentences one below the other and are read from top to bottom like a text; the first one that matches wins, and you see that without knowing it. The dry run makes this chain visible on real traffic. |
| History | density as evidence | Ten thousand rows in an even 28 px cadence, without a gap, without a loading area. The quantity itself is the statement: nothing went through unobserved. |
| Domain panel | the answer in one line | "Known" or "unknown" stands as the first word, in the largest type of the pane, before any detail follows. |
| Setup and Sandbox | the verdict on the three guarantees | Three rows, three states, no explanation before them. The explanation stands below, for whoever wants it. |

Whatever does not occur in these five moments gets no saturation, no motion and no large type. That is the trade.

---

## 2. The motion language

### 2.1 The test question

Every animation answers exactly one of four questions: What has arrived? What has left? What has changed in place? What have I just triggered? An animation that answers none of these questions is deleted, not tuned. This question is the review gate for HUM-029 to HUM-034.

Every duration, every curve and every distance comes from `HMotion`. A `Duration`, a `Curve` or a pixel offset as a literal in a feature file is a defect, even if the number is right: one table is the only way in which five unbuilt screens keep the same language.

### 2.2 The table

| Change on the screen | What it explains | Duration | Curve | Token | Path |
|---|---|---|---|---|---|
| Row arrives in the queue | "A packet has arrived from outside" | 180 ms | `enter` | `HMotion.arrive` | 8 px from above, `HMotion.arriveOffset`, plus fade-in |
| Several rows in the same frame | "There are several, one after another" | +30 ms per row, at most 5 | `enter` | `HMotion.stagger`, `HMotion.staggerMax` | as above, staggered |
| The queue counter receives an arrival | "Something has come, even if you do not see it" | 2 × 120 ms, one there and back | `easeInOut` | `HMotion.press` | Tint of the `HBadge` in the queue header from `held` 10 % to `held` 20 % and back; exactly this one widget, even with the queue frozen. The digit inside it jumps, it never animates (2.9). |
| Control is pressed (mouse or key) | "Your grip has arrived" | 120 ms | `enter` | `HMotion.press` | Fill |
| Rail changes state | "The state has changed" | 200 ms | `enter` | `HMotion.sweep` | Sweep from top to bottom, never a cross-fade |
| Holding confirms a block | "That was intended" | 250 ms | linear | `HMotion.holdToBlock` | Fill from the left |
| Holding fills the release valve | "This applies to the whole session" | 400 ms | linear | `HMotion.holdToConfirm` | Fill from the left |
| Decision in transit | "I have acted, the answer is pending" | — | — | — | no motion. The rail keeps its swept state, the glyph is swapped, the strip is still missing. If the answer does not come within `HMotion.confirm`, the inline `Diagnostic` from 4.4 takes the place of the strip. |
| Rejected key press | "This key does not apply here right now" | 120 ms | `enter` | `HMotion.press` | The control that would have acted shows its fill and drains again; the reason stands next to it (5.3). Never a shake, never a red flash. |
| Decided row leaves | "This request is allowed or blocked outside" | 220 ms | `exit` | `HMotion.leave` | 12 px to the side (`HMotion.leaveOffset`), then height to zero |
| Selection changes | "I am now looking at this row" | one frame | — | — | Change of fill, no change of height |
| Section or group expands | "There is more below" | 180 ms | `enter` | `HMotion.arrive` | Height |
| Sidebar expands or collapses | "Here are the names" or "back to only the symbols" | 180 ms | `enter` open, `exit` closed | `HMotion.arrive` | Width of the icon rail from 48 to 200 px; the label is laid out at full width, only uncovered, and faded in from half width. Under reduced motion the width jumps, the fade-in keeps its duration (2.10, HUM-254). |
| Caution tape runs | "Here is the barrier" or "the search is running" | 1200 ms per period, endless, as long as the cause persists | linear | `HMotion.cautionTape`, `HMotion.cautionTapePeriod` | Stripe pattern sideways by 16 px per period; the widget itself stands still. Under reduced motion the stripes stand still (3.6, HUM-260). |
| Sheet slides in from the right | "This belongs to what stands on the left" | 180 ms | `enter` | `HMotion.arrive` | 8 px from the right (`HMotion.arriveOffset`) plus fade-in; the background stays put |
| Modal appears | "You have to answer here first" | 180 ms | `enter` | `HMotion.arrive` | no path, only fade-in of the modal and dimming of the background; a modal that flies in claims a direction it does not have |
| Command palette opens | "The manual is here" | one frame | — | — | no motion. Whoever calls it is already typing. |
| Section change (`Ctrl+1..5`) and tab change | "I am somewhere else now" | one frame | — | — | no cross-fade in the `IndexedStack`, no shift. The change is navigation, not an event. |
| Return banner appears | "Something happened while you were away" | 180 ms | `enter` | `HMotion.arrive` | like an arrival: 8 px from above plus fade-in |
| New rule appears in the list | "This rule exists now" | 240 ms | `enter` | `HMotion.ruleDraw` | Drawing of the row |
| Countdown below a threshold | "Look now" | 1200 ms per breath, 3 breaths | `easeInOut` | `HMotion.breathe`, `HMotion.breatheCycles` | Opacity between `HMotion.breatheMinOpacity` and 1.0 |
| Error | "Something has gone wrong here" | — | — | — | no motion; the Diagnostic fades in via `HMotion.arrive` into a reserved slot |

### 2.3 Direction is meaning

Allowed and allowed-after-editing glide to the right, blocked, timed out and error to the left. Otherwise no widget anywhere in the program moves horizontally — except the sheet, which comes from the edge it is attached to. The caution tape (3.6) is no exception: it stands still, only its stripe pattern runs, and a pattern is a fill, not a position (next paragraph).

The axis rule binds the **position** of a widget, not its **fill**. A fill that grows from the left is progress and not direction: it says "this far have I got", not "that is where it goes". That is why both hold fills may run from the left without breaking the rule, and that is why nothing in the program fills from the right, from the top or from the middle — a second fill direction would be a second meaning for the same gesture.

A single meaning for a single axis of motion is readable at the edge of the field of view, and that is where the queue usually stands.

### 2.4 The exit has two phases

Gliding and fading out run in the first sixty per cent of `HMotion.leave`, the collapse of the height in the last sixty (`HMotion.leaveGlideFraction`, the phases overlap). On a shared `easeIn` curve the row stands still for about 170 ms and only jumps to the side in the last frames; exactly the direction that carries the whole meaning is then invisible.

The collapse stays, and it costs: a list whose rows animate their height cannot know its row height in advance. The queue therefore keeps `AnimatedList` and does without `itemExtent`; the extent obligation from section 7 does not apply there, but in History and in the body views, where n is large and no row changes its height. `AnimatedList` and `SliverAnimatedList` know neither `itemExtent` nor `prototypeItem`; whoever demands both at once demands something Flutter does not offer. The sums work out because the queue is bounded by the hold budget, while History carries ten thousand rows (deviation in section 8).

A leaving row is a frozen image: it observes no provider, loads no detail, its countdown stops from the instant of the decision, and it keeps the height it had at that instant. A row that keeps counting during its exit claims something false. "Frozen" is meant literally and needs a widget of its own with resolved values instead of a `ref` (section 9).

### 2.5 The sweep belongs to the grip, the strip to the answer

The rail sweep starts with the decision itself and runs the full 200 ms, regardless of when the daemon answers. The confirmation strip appears only once the answer is there. Two expert opinions contradict each other here — "show the effect at once" against "claim nothing before the answer" — and both are right, because they speak of two different things: the sweep says "your grip has arrived", the strip says "this has happened". Over a Unix socket `Decide` takes a few milliseconds; coupled to `DecisionSending`, the rail fills about two per cent and drains again, so the one moment the product exists for takes place without visible motion.

Between sweep and strip lies the state that lasts longest when something goes wrong: the decision is in transit. It deliberately has no motion of its own. The rail stands swept, the glyph is swapped, the strip is missing. If no answer comes within `HMotion.confirm`, the `Diagnostic` from 4.4 takes the place the strip would have had, and the row does not leave.

### 2.6 The effect stands within 400 ms, not 3 s

What the app decides on its own stands within 400 ms of the key press: rail swept, state glyph swapped, countdown frozen. What depends on the answer stands within 400 ms of the answer: the confirmation sentence. The separation is not cosmetic — the app does not control the latency of the daemon and therefore must not promise a deadline it cannot keep.

The three seconds until the row leaves (`HMotion.confirm`) are a resting state and not a waiting state. Three seconds of dead time between grip and motion tear the connection apart: when the row drops, the human has already moved on, and the motion no longer has a visible cause.

### 2.7 The breath is a flag, not a scale

The remaining time stands in the arc length of the ring. The breath only says "look now": three breaths when the flow drops below 20 % of its budget, three more at 5 %, then quiet. It runs over `Curves.easeInOut`, not on a linear controller with `repeat(reverse: true)` — a triangle wave has a corner at both ends, and the eye reads that as blinking. It never takes the glyph below `HMotion.breatheMinOpacity` (0.72); today it drops to 0.45, which makes the most urgent request the least visible one.

The phase comes from the clock time modulo `HMotion.breathe`, so that group header and rows breathe in step without needing an application-wide ticker. A glyph that is just crossing the threshold does **not take over** this phase **at once**, though: it starts its first breath at opacity 1.0 and only then runs into the shared phase. Otherwise the flag begins, depending on the second, with a jump from 1.0 to 0.72 in one frame — exactly the blinking this section forbids, and on the most urgent row of all.

If a threshold falls while the section is invisible, the three breaths lapse. They are not made up for. The breath says "look now", and that now is over; what must remain remains in the arc, in the Semantics value and in the warning from 4.8.

### 2.8 Nothing moves under the reading eye

An arrival may never insert a row above the pointer or above the scroll anchor. As long as the pointer is in the queue pane, the last keyboard navigation was less than `HMotion.freezeAfterKey` (2 s) ago or a multiple selection exists, the order stays frozen; new flows only increase the pill "+n new". The pill lies over the topmost row, semi-transparent, and is never inserted into the column — a pill in the header would push every row down by its own height, which is exactly what freezing exists against. Merging happens on click or `HMotion.freezeAfterPointer` (500 ms) after leaving the pane.

The pill is reachable, not only visible: it is a focus stop and has a key (`Shift+J`, "to the new ones"). Without a key a keyboard user could never merge the arrivals without reaching for the mouse. While frozen, the selection stays on its row and does not move along when merging; the human put it there, not the list.

**Decided is not removed.** The two words name two different events, and only one of them frees space:

- **Decided** means: the row stays in its place in the snapshot until `HMotion.confirm` has elapsed. It does not count as removed, it pushes nothing, and during this time it carries the confirmation strip (3.4). The evidence belongs to the row that produced it, so the row has to stay that long.
- **Removed** means: the row leaves the snapshot at once and takes the exit from 2.4. That applies to the end of the confirmation window, to a timeout and to a decision from elsewhere (rule, notification, second window). Removed rows only free space below the pointer and push nothing in.

At most three decided rows rest at the same time. If a fourth decision arrives, the oldest leaves at once without sitting out its window. Without this limit a quickly worked-off burst would fill half the visible list with evidence for things already done. Resting evidence keeps its position; it sorts neither up nor down, because that would be a motion that answers none of the four questions from 2.1.

### 2.9 What never animates

- **Text someone is reading.** URL, path, header tables, body views, rule sentence, diff. They change at once or not at all. This program demands that a URL is read before it is decided on; moving text costs a second fixation.
- **Numbers.** Countdown, queue counter, tray counter, window title. Digits jump; they do not roll, flip or scale. The surface around a number may pulse (2.2), the number itself never.
- **Focus rings.** One frame. A ring that fades in reads as input lag.
- **Row heights.** A row does not change its height because its state changes: 36 px as the minimum height in every state, see 3.4. That the same row is taller at a larger text scale is not animation but layout.
- **Direct manipulation.** Splitter, scrolling, text selection and drag reorder follow the pointer one to one. Only the gap that has been freed animates. Any damping between hand and pixel reads as if the program were contradicting the hand.
- **Hover.** Hover tints (`HMotion.press`) and uncovers affordances in space that is already reserved. Hover moves nothing and changes no size. A filled control visibly steps away from its rest state on hover and once more from hover on press: into hover at least 0.04 in OKLab (`HColorDerivation.interactionStepMin`); into press, colour (at least 0.025, `interactionJumpMin`) and a ring carry the step together; the ring lies in the always-present one-pixel border and only changes its colour (HUM-272). This holds in both themes, without the word losing its 4.5:1 or the fill its 3:1 as a surface.
- **Idle.** There is no ambient, idle or continuous motion except the bounded breath from 2.7 and the caution tape from 3.6, which only runs as long as its cause persists. No spinner, anywhere (2.11).

### 2.9b The shared transition

A `Hero` is not forced. It answers exactly one question — "where did the thing I was just looking at go?" — and therefore belongs in the two places where the eye would otherwise jump:

| From | To | What flies |
|---|---|---|
| Row in the queue | Card in the detail pane | the host chip with its rail strip |
| decided card | Row in the History | the same chip, as soon as both screens are visible side by side |

Everything else makes do with the tokens from section 2.2. Three conditions, otherwise the transition is dropped without replacement: source and target are on the screen at the same time, the flying element carries the same meaning in both states, and the distance is shorter than a screen diagonal. Under reduced motion it is always dropped; the arrival at the target is then shown by the same sweep as any other selection.

A shared transition may never be the only sign of a state change: whoever looks at the screen only after the animation must still find the same statement in colour, glyph and text. An animation that answers no question of the user is a defect, not decoration.

### 2.10 Reduced motion

All distances and all loops run through `HReducedMotion` in `motion.dart`; no widget decides for itself whether it may move. Under reduced motion the path is dropped, not the feedback: `arriveOffset` and `leaveOffset` become zero, while fade-out, key fill and rail sweep keep their full durations.

The exit is the case in which "less path" accidentally takes the feedback along with it. A row whose height drops to zero at once has nothing left that could fade out; the fade-out would run for 220 ms over nothing. Under reduced motion the leaving row therefore keeps its full height and fades out in place via `HMotion.leave`; only then is it removed in one frame. The collapse is dropped, the fade-out stays — that is the rule, not its exception.

Every loop gets a resting substitute instead of nothing at all: the breathing glyph becomes a permanently doubled ring at `HMotion.reducedRingAlpha`. The 20 % threshold is information, and whoever has switched animations off must not lose it.

### 2.11 Waiting

Waiting is a state like any other and therefore gets a language, not a symbol. The rule applies the same everywhere:

- Below `HMotion.waitVisible` (150 ms) nothing happens. An indicator that is visible for less than a reaction time is read as flicker.
- From 150 ms the pane keeps its layout and draws the hairline skeletons of the expected rows: the height of the target density, `fg2`, no motion. The skeleton says how much is about to come and where it will stand. A spinner only says that something is running.
- Whatever has appeared once stays for at least `HMotion.waitMinVisible` (400 ms). Otherwise an answer shortly after the threshold produces exactly the flicker the threshold is meant to prevent.
- On arrival nothing is shifted: the skeleton is replaced by the row it described, in the same place, in one frame.
- A failure while waiting is an error and lands, per 4.4, in the same place where the skeleton stood — never as a toast, never full-screen, as long as the rest of the screen is valid.

Where that sits in detail:

| Waiting operation | Place | Skeleton |
|---|---|---|
| Body is loaded and parsed (up to 8 MiB, HUM-030) | in the body pane, below the switcher that is already visible | ten rows of the target density 24 px; with `tooLarge` the card with size and findings appears instead at once |
| Dry run of a rule (HUM-033) | in the dry-run panel, never global | three result rows of 28 px below the unchanged heading; the editor stays usable |
| History loads more (HUM-032) | below the last loaded row | as many 28 px skeletons as the page is large; the table does not jump on arrival |
| Export (HUM-032) | in the export menu, at the triggering entry | no skeleton, but progress in rows ("1,284 of 5,000"), because the number is known |
| Decision in transit | in the row and in the action bar | none. The rail stands swept; that is the indicator (2.5). |
| Daemon does not answer yet at start-up | full-screen | Splash after the same two thresholds (4.2) |

---

## 3. Hierarchy and colour

### 3.1 The one important thing per screen

A screen has exactly one largest text element, exactly one filled control and exactly one empty state. Everything else is ghost, tinted or neutral.

"Filled" means: the control carries the accent as a surface, not as an outline — either as a full accent surface with `onAccent` text (`HButtonVariant.primary`) or as an accent tint at the token cap (`HColors.tintAlpha`, 10 %), like the release valve. Everything else is ghost: border or text, no surface.

| Screen | The one important thing | Largest type | Only filled control |
|---|---|---|---|
| Intercept | the URL of the selected request | `mono14` in the card header | Release valve |
| History | the filtered list | `mono14` in the detail header | none; the accent belongs to the filter field in focus |
| Rules | the order of the rules | `ui14` for the rule as a sentence | `+ New rule`, as long as the list is empty, otherwise `Save` in the editor |
| Setup and Sandbox | the state of the three guarantees | `ui16` for the verdict | the action of the topmost Diagnostic |

The rule applies per **screen**, not per pane. A pane has no most important thing of its own that could compete with that of the screen: the first row of the context pane in Intercept carries the answer "known or unknown" in `ui14` and thus stays below the URL in the card header. In `ui16` it would stand above the subject of the decision, and that even if `mono14` and `ui16` cannot be compared by the number alone — in doubt the URL wins, because the screen is about it.

An empty screen is the exception the rule needs: as long as a screen has no content, its primary creation action is filled, so that a way forward is visible at all; as soon as it has content, it becomes ghost and the fill goes to the control of the decision. More precisely: **one filled control per decision context**. A modal, a sheet and the screen below are three contexts, but never active at the same time.

Today the scale collapses to 11/12/13: `ui20` stands only on placeholders, `ui14` and `mono14` nowhere, and the URL — the subject of the decision — runs in `mono13`. From now on `ui16` and `ui20` belong to the most important thing in each case, not to the most unfinished.

### 3.2 The density rhythm

- Base 4 px. Every spacing is a multiple of it; `x1/2 = 2 px` does not exist.
- Panel padding 12 px. Every right gutter ends at 12 px, including that of the queue header, so that counters and countdowns align in one column.
- Three row densities, no more: 36 px in Queue and Rules, 28 px in the History table, 24 px in body views. All three are minimum heights (section 6, text scaling), and all three need a token (section 9).
- **Measure at most 90 monospace characters** — for running text and for single-line URL and path fields. Surplus pane width becomes gutter, never line length. At 2560 px a URL would otherwise run over 137 characters, and the eye searches for the start of the next line.
- **Code, hex and tables never wrap.** Raw body, hex view and the History table scroll horizontally and get no measure. A wrap there produces visual lines that shift the byte offsets of the findings (HUM-030, pitfall "wrap off"); a misplaced finding is worse than a long line.
- Panes 28/44/28 up to 1800 px window width. Above that the inspector freezes at measure plus gutter, queue and context take the rest. Below 1280 px the context pane collapses and is reachable via `Ctrl+D`.
- The seam between two panes belongs to the splitter. A pane draws neither a left nor a right border; between two panes there is exactly one hairline, and the grip of the splitter lies invisibly over it (3.7).
- A screen shows one empty state, not one per pane. As long as the main pane is empty, the side panes stay quiet.

The measure is given as a character count, not as a pixel width, because the width depends on the installed font. For whoever calculates anyway: JetBrains Mono and every fallback of the chain run at about 0.6 em advance, so about 700 px at `mono13` and about 650 px at `mono12`; `queue_row.dart` calculates with 7.3 px per character at 12 px. These numbers are illustration; the character count is normative (section 9).

### 3.3 Colour means state

The accent marks what can take focus or be clicked: `HColors.accent` (#E8B84B) and `HColors.lAccent` (#A57A1C). Since HUM-260 it is the yellow of the brand — the app icon is a black fist on a yellow tile (#EBC05A to #D49E34) —, in the light theme as ochre, because yellow on white is unreadable. Whatever stands on the accent, text as well as glyph, is dark: `HColors.onAccent` (#14161C), in both themes, never white. The numbers: accent on `bg0` to `bg3` at least 7.95:1, ochre on `lBg0` to `lBg3` at least 3.29:1 (surface threshold), `onAccent` on the accent 9.81:1, on the ochre 4.65:1. A word in accent colour stands in `accentText`, never in `accent`. Inert text is never accent — an accent-coloured URL that nobody can open teaches the eye that accent means nothing, and then focus, selection and primary button break along with it. Hover and press of the primary button shift not only lightness, since the narrow window of the ochre leaves hardly any room for that, but also chroma and hue, always towards the stronger, never towards the paler: hover lighter and more saturated (`accentFillHover`), press close to the ochre, more saturated and, where the word allows it, darker (`accentFillPressed`), plus the ring `accentPressedRing` (`onAccent` at 75 percent, at least 3:1 against the pressed fill). In the light theme the hue turns towards orange, in the dark theme it stays yellow (HUM-272).

State colours stand only on state, from `HStateColors` via `HTheme.of(context).state` or `tokens.stateColor(state)`:

| State | Token | Dark | Light |
|---|---|---|---|
| `held` | `HColors.held` | #79C9EC (until HUM-260 #E0B24A) | derived, `HStateColors.light` (#1C91C5) |
| `allowed` | `HColors.allowed` | #4FBF8C | derived |
| `allowedEdited` | `HColors.allowedEdited` | #57B99F plus accent dot | derived |
| `blocked` | `HColors.blocked` | #E5646E | derived |
| `timedOut` | `HColors.timedOut` | #8A90A2 | derived |
| `autoRule` | `HColors.autoRule` | `allowed` at 60 % | derived |
| `passthroughLlm` | `HColors.passthrough` | #B48AF0 | derived |
| `error` and Secret | `HColors.secret` | #F0784F | derived |

Besides the states there are, since HUM-260, two tones that are not a state of a flow and therefore stand not in `HStateColors` but in `HSurfaceColors`:

| Meaning | Token | Dark | Light |
|---|---|---|---|
| Attention: warning, open findings, writable mount, an operation that is not settled yet | `colors.warning`, word `colors.warningText` | #E8B84B (equal to the accent) | #A57A1C (equal to the accent) |
| Information: Diagnostic of level `info` | `colors.info`, word `colors.infoText` | #7C9CF5 | #5B7FE6 |

Rules for this:

1. **Full saturation only where two states stand side by side.** In the Intercept queue, by construction, only `held` stands; fifteen full `held` rails repeat a fact and become the loudest thing on the screen, while the actual decision stays neutral. There the rail carries `tokens.tint(tokens.stateColor(state))` (10 % alpha). At the instant of the decision it sweeps to full saturation — then it is the only saturated surface in the pane. History keeps full saturation, because eight states stand mixed there.
2. **Colour is never the only channel.** Every row that carries state carries glyph and colour. In the light theme `allowed` and `blocked` measure 1.01:1 under deuteranopia; a 4 px wide colour alone says nothing there. The glyph table already exists in `flow_state.dart` and has not been used so far.
3. **No colour change by interpolation.** `held` #79C9EC lerped to `blocked` #E5646E would run through a pale mauve (#AF96AD) that lies between `timedOut` and `passthrough`, and would show a state that never existed. That is why the rail sweeps instead of cross-fading.
4. **Method badges are neutral in lists** (`fg1` on `bg2`). The method hues borrow state colours (PUT/PATCH is `held`, DELETE is `blocked` at 70 %, POST is `passthrough`); in a list the eye reads a reddish badge next to a red rail as two blocks, not as a verb and a state. Only the one badge in the card header gets a hue.
5. **Chrome that cannot change carries `fg1` or `fg2`.** A state indicator has at least two values, otherwise it is none. "Intercept on" and the connection dot are always green today, because the shell only builds in the connected state; two permanently green marks teach the eye to overlook green, and green is the colour of the one action that matters. Either wire them up or remove them.
6. **Red means blocked.** Errors and findings are orange (`HColors.secret`). Red as ambience does not exist.
7. **In body views findings are the only chroma.** JSON keys, values and hex stay on the `fg` ladder. A body is the place where a secret must stand out within seconds; three colours for data types make the finding compete with the syntax. This deviates from HUM-030, see section 8.
8. **A control that counts state is still accent.** `+n new`, the findings chip, the group counter and the batch button all carry a number about held requests — and they stay accent, because you can touch them. The state they count stands as a glyph inside, never as a fill. Otherwise the accent falls apart into as many meanings as there are counters.
9. **What only displays and triggers nothing carries state colour.** The tray icon is not a control but the only state indicator of the session as long as the window is not visible; its counter therefore stands in the colour of `held`, not in the accent (deviation from HUM-034, see section 8). Rule 8 and rule 9 together: a surface to touch is accent, a surface to read is state.
10. **The resting `held` rail is exempt from the 3:1 rule.** A 10 % tint of `held` on `bg1` measures 1.21:1; it would only reach 3:1 at 46 % alpha, and at 50 % it is no longer a tint but the saturated surface that rule 1 reserves for the decision. There the rail only groups; state and deadline are carried by glyph, label and semantics. The exception applies by name to the resting state rail in the queue and to nothing else — the swept rail, the selection rail and every rail in History reach 3:1. `HColorDerivation.tint` caps at `HColors.tintAlpha`; a stronger rail would not be a call but a new token (section 9).

11. **Accent and `held` are never the same colour family.** Until HUM-260 `held` was amber and the accent blue; since the accent is the yellow of the brand, `held` carries a cool sky cyan. A yellow counter that means "touch here" and a yellow rail that means "waiting" would be exactly the confusion that rules 8 and 9 rule out. The distance from `held` to every other state colour and to the accent is measured in `tokens_test.dart`, in OKLab, normal and under simulated protanopia and deuteranopia (Machado 2009), and is at least 0.08 everywhere; the old amber would be at 0.02 next to the new accent. Under deuteranopia cyan and the violet of `passthrough` fall onto the same blue; there only lightness separates them, and `held` is therefore lighter than `passthrough`.

12. **Yellow means attention, blue means information, cyan means held.** Warnings carry `warning`, the yellow of the brand, like the caution tape (3.6); Diagnostics of level `info` carry `info`, the blue that was the accent until HUM-260; `held` now stands only on a held request. Before, warnings carried the amber of `held`, and after the change of `held` to cyan a cyan warning next to a yellow piece of information would have read the wrong way round. Because `warning` coincides in value with the accent, a warning **never stands as colour alone**: always with a glyph (`triangleAlert`) or text, a finding in the text with the wave instead of the straight line of the replacement. A yellow control with a warning is still a control (rule 8); the release valve with an open finding is one, and label and glyph say why it looks different.

### 3.4 The queue row, written out

Because most rules converge here, it stands written out in full once:

```
| 4 Rail | 8 | 16 state glyph with countdown ring | 8 | Method neutral | 8 |
  Host ui13/500 | 8 | Path mono12, shortened in the middle | 8 | Findings chip | 8 | Action slot 28 | 12 |
```

- **Height: at least 36 px, in every state, without a second line.** A selection that grows from 36 to 56 px pushes everything below it down by 20 px on every `J`; the second line also repeats size, content type and waiting time, which the card two panes further on shows anyway. 36 px is a minimum height and not a fixed one: at `TextScaler.linear(2.0)` `ui13` alone measures 40 px line height, and a fixed height would silently swallow the overflow. So the row grows with the scaling, but never with its state.
- **What gives way when.** Fixed widths use up 108 px of chrome plus method badge (GET about 36 px, DELETE about 56 px) plus findings chip (about 24 px); at `HSize.paneMinQueue` (280 px) about 100 px remain for host and path together. That is why the order in which things are shortened stands here and not at the discretion of the layout: the host gets a lower bound of 12 em and is shortened in the middle beyond that; the path gives way first and disappears entirely below about 360 px pane width; the findings chip collapses below about 320 px to a 4 px dot in `HColors.secret`. The host is the last thing to go — it is the answer to "where to".
- **The rail:** `held` as a tint, decided in full colour, selected in accent across the full 4 px. The selection **replaces** the state rail, it does not overlay its left half; that is already stated in the documentation of `spacing.dart` and works as soon as the row carries its glyph.
- **The fill:** hover `bg2`, selection `bg3`. Never the same colour. `Enter` and `A` allow the selected row, and allowing is irreversible; if a row under the pointer looks exactly like the selected one, someone reads the one and sends the other. `bg2` stands for exactly this intermediate step in the ladder and has not been used in rows so far.
- **No `mm:ss` in the row.** The remaining time stands in the arc of the ring and in the Semantics value; the digits belong to the selected request in the card. Fifteen digits running at the same time are the opposite of a calm control room.
- **The action slot** is always 28 px wide and empty at rest. Hover **and** focus fade in blocking there; it replaces nothing, shifts nothing and makes no text re-wrap. Today the row swaps about 33 px of countdown for about 97 px of actions on hover, re-wraps the title line in doing so and puts "Allow" directly under the pointer.
- **From the row only blocking is possible, never allowing**, and blocking demands 250 ms of holding. Allowing is irreversible and needs the URL; that stands in the card. This also keeps the rule that allow and block never stand side by side — in the narrowest pane, 280 px would otherwise be too little for an honest spacing.
- **The confirmation strip is the row, not a box above it.** During the confirmation window (`HMotion.confirm`) the decided row swaps its content for the strip text, on the same baseline, at the same height: rail in full state colour, state glyph fully saturated on the left, then the sentence in `mono12`. No second line, no extra box, no change of height, no new wrap. Afterwards the row leaves per 2.4.

### 3.5 Selection, multiple selection, group header

The multiple selection is at once a freeze condition (2.8) and a protection level (5.4). It therefore needs an appearance of its own that is unambiguous, and it gets it via the rail, not via the fill:

- **The rail says what is selected.** Every member of a selection carries the accent rail across the full 4 px, whether one or twelve.
- **The fill says where the cursor is.** Hover `bg2`, the row last clicked or reached with `J`/`K` `bg3` — exactly one row in the whole pane. A member without the cursor stays unfilled; the rail alone carries the membership.
- **No second filled control appears.** With a selection larger than one, the release valve relabels itself ("Allow 5 selected" / "5 ausgewählte senden") and stays the one filled control of the screen (3.1). An extra batch button next to it would be the second.

The group header is a row and keeps the row rule: in the 28 px action slot it shows only `Block {n}`. `Allow {n}` does not stand there — the same reasoning as in 3.4, amplified by a factor of n. As soon as a group is selected, the action bar takes over allowing, and the card next to it shows what is being decided on: host, method mix, paths, findings total. This deviates from HUM-029, see section 8.

### 3.6 The caution tape

Humanitl is the barrier at which the traffic stops. Its sign is a yellow-and-black caution tape: diagonal stripes in brand yellow (`HColors.accent`) and `HColors.onAccent` that run evenly sideways (`HCautionTape`, HUM-260). It is the only element of the program that runs endlessly, and may therefore only stand where it means something.

**Where it stands.** In exactly two places:

1. **Above the queue, as long as at least one request is held.** Four pixels high (`HSize.cautionTape`) below the header of the queue pane. It comes with the first held request and goes with the last. The barrier stands as long as someone is waiting at it.
2. **As the indeterminate progress indicator of the network search in setup** ("search running"), as long as the search is running. There it replaces the hairline that used to run back and forth, and says the same thing: it is working, without claiming a progress level.

**Where it never stands.**

- Never in a row, on a card, in a body or over text someone is reading (2.8, 2.9). It is edge, never content.
- Never as a state indicator. The tape does not show *how many* are waiting or *which*; counters, rail and glyph do that. It may never be the only channel of a statement: whoever does not see it loses nothing that is not in the counter in the queue header.
- Never as a control. It takes no focus, has no semantics (`ExcludeSemantics`) and reacts to no pointer. That it is yellow like the accent is the brand, not an invitation to touch (3.3, rule 8); that is why it always carries the black stripes as well and never an accent surface alone.
- Never in a state colour and never in a colour that changes per theme. It is a tape, not a token of the ladder; in the light theme it is the same yellow and black as in the dark.
- Never in a third place without this section naming it. A tape on every corner is wallpaper.

**How it runs.** Linearly, one period (16 px, one yellow and one black stripe) in `HMotion.cautionTape` (1200 ms), so about 13 px per second: you see that it is running, and it does not pull at the eye. Seamless: a shift of one period draws the same image as none. Its place is always reserved, even without the tape — the hairline then stands there —, so that nothing below it jumps when it comes or goes (2.8). It does not rebuild while running, but only paints its own pixels behind a `RepaintBoundary`; without a cause it is not in the tree and has no ticker, and in an invisible section its ticker falls silent via `TickerMode`.

**Reduced motion** (2.10): the stripes stand still, the tape stays. The path is dropped, the statement stays.

**Tests and goldens** stop the tape via `HCautionTape.debugFreeze`, set in `app/test/flutter_test_config.dart`; an endless tape would otherwise run into every time limit of `pumpAndSettle`, and a golden shows it as a still image at phase zero.


### 3.7 Layout grid

One grid for the whole shell (HUM-286). Each surface used to invent its own geometry: the settings sheet header was taller than the queue header beside it, the action bar sat inset in a box of its own, the active rail item kept a gap to the line above it, and the rules list drew a second line a few pixels under the toolbar. The rules below are the reason none of that can come back.

**Rows.** Under the window header (`headerBar`, 40 px) every pane starts on the same line. The first row is the pane header, `paneHeaderHeight` (48 px) high, its bottom border included. Queue, request card, context pane, editor, History, Rules, Sandbox and every side sheet (settings, rule from request, history detail, audit record, command line) use it. The rail has the same row: its entries are `railItemHeight` (48 px, equal to the header) high and start directly under the window header, so "Intercept" in the rail and "Queue" in the pane stand on one line.

**One widget, one height.** A pane header is `HPaneHeader` and nothing else. It sets the height from the token and centres its content vertically; it has horizontal padding (12 px) and no vertical padding. A header that reached its height through padding of its own would change with the font, and the line under it would no longer continue into the header next door. The height is fixed, and it follows the text scale once for everybody: `HPaneHeader.rowHeight` scales the token with the ambient text scaler (clamped to 1 to 2, rounded to a whole pixel), every header and every rail entry takes that one value, and content that does not fit is clipped rather than pushing its line out of the row. A two-line URL in the request card fits at every scale up to twice. The divider under the header is part of it; in the queue that is the caution tape or, without one, the hairline in the same 4 px (3.6).

**Borders.** Every border and divider is `borderWidth` (1 px, the hairline). Two adjacent regions share one divider, and it belongs to the upper one (or, side by side, to the splitter). What stands below a header starts directly under its line and draws no top border of its own; a nested container never adds a line on top of the line it already stands under. A block that has a separator above it drops it when it moves to the top of its list (the bundled rules under the Rules header).

**Highlights and bars are flush.** The active rail item fills its whole row edge to edge, fill and accent mark from the line above to the next item, without a margin. The action bar of the inspector is a footer of its pane: full width from seam to seam, down to the status bar, separated from the card by one border, with no inner card and no inset. The seams between panes are one hairline, never a strip of background beside a line (3.2).

**Sheets hang between the bars.** A side sheet starts under the window header and ends at the status bar, like a pane; its header is the same `HPaneHeader`, so the divider continues across the window when the sheet is open. A screen mounts its sheet at its own top, never inside a region below its header (the narrow rule editor of Rules is mounted by the screen, not by the list).

**Views without a header.** Setup and Audit open with content, not with a pane header (a document and a status card). They start with the panel padding under the window header. When one of them gets a header, it is an `HPaneHeader`.

**Checked by** `app/test/features/shell/layout_grid_test.dart`: in Intercept (with and without the settings sheet), History, Rules and Sandbox, at text scale 1 and 2, every visible pane header starts under the window header and ends on the same line, and the active rail item fills its row; the settings sheet, the narrow rule editor sheet and the sandbox command sheet start under the window header and their header line continues the one beside them. Between two panes, Tab goes pane, splitter, pane (`app/test/core/ui/design_system_test.dart`).

---

## 4. Guiding the human

Principle 7 from `BACKLOG.md` applies literally: no state without a plain-language reason, no error without "why" and "what now". All texts come from ARB, none stands in the code. English is the source language, German the translation; every sentence this document fixes therefore stands in both languages, English first, in the order in which the ARB files carry it.

### 4.1 Empty states

There are two empty states, and confusing them means promising the human a future that does not exist.

**Empty because nothing has happened yet.** The sentence names the next event, never the absence: "`<subject>` appears here when `<trigger>`". The words "nothing", "no" and "yet" do not occur in it. An empty screen is the screen a new user sees for longest, and the only teaching surface the program gets for free.

**Empty because a filter matches nothing.** Here there is no coming future; the set is there and the filter cuts it away. The sentence names the filter, the number of matches and the way back, and the way back is an accent control, not part of the sentence: en "`host:foo` matches 0 of 1,284 requests · Reset filter", de "`host:foo` trifft 0 von 1.284 Anfragen · Filter zurücksetzen". This applies in History and in Rules.

**Empty as a fact about a finished object** is neither of the two. A request without query parameters never gets any; there "No parameters." / "Keine Parameter." is right and the catalogue of forbidden words does not apply. The distinction is checkable: if there is an event that would fill the surface, it is an empty state; if there is none, it is a statement.

The empty queue additionally shows the three keys, once and never again. The order begins with the reversible key, and `Enter` names its consequence: en "`B` block · `Enter` send (final) · `J`/`K` move", de "`B` blockieren · `Enter` senden (endgültig) · `J`/`K` bewegen". A teaching surface that begins with the irreversible key teaches exactly the wrong thing first.

Today five shipped keys violate the first formula, and they are replaced together with the screen that uses them:

<!-- lang: de -->
| Key | today | instead |
|---|---|---|
| `shellSectionPlaceholder` | "Nothing here yet." / "Hier ist noch nichts." | the screen says what it will do, or it is absent (4.2, last paragraph) |
| `interceptEmptyTitle` | "No request is waiting" / "Keine Anfrage wartet" | en "The queue is open", de "Die Schleuse ist offen" |
| `interceptEmptyHint` | "The agent works without a network." / "Der Agent arbeitet ohne Netz." | en "Every request the agent makes appears here.", de "Jede Anfrage des Agenten erscheint hier." — "ohne Netz" reads as "without a safety net" and moreover claims something false as long as no session is running (4.2) |
| `interceptCardEmptyTitle` | "Nothing selected" / "Nichts ausgewählt" | en "Pick a request", de "Eine Anfrage wählen" |
| `shellIsolationUnknown` | "Isolation: not checked yet" / "Isolation: noch nicht geprüft" | en "Isolation: check running", de "Isolation: Prüfung läuft" — and if none is running, the sentence names the action that starts one |
<!-- /lang -->

`interceptQueryEmpty`, `interceptHeadersEmpty` and `interceptBodyEmpty` stay as they are: they are statements about a finished request. So that the forbidden words do not come back with the next screen, an ARB lint checks them in both languages and knows these three exceptions by name (section 9).

### 4.2 Before the first request

The first screen is not the empty queue but the question whether anyone answers at all. Four states, each with text, place and exactly one action:

1. **The daemon does not answer yet.** Splash, full-screen, after the thresholds from 2.11: below `HMotion.waitVisible` nothing, after that visible and standing for at least `HMotion.waitMinVisible`. No spinner, no progress bar for something whose duration nobody knows. A splash that flashes up on a quick start is worse than no splash.
2. **The daemon is missing at start-up.** Setup screen. The `Diagnostic` is the one important thing of the screen (3.1): `why` from the daemon or from the transport error, `fix` as a filled control. Nothing else on this screen competes for it.
<!-- lang: de -->
3. **The daemon is running, but no session.** The queue does not say the agent is working without a network — nobody is working. It names the next event the human triggers: en "Once a session runs, every request the agent makes appears here.", de "Sobald eine Session läuft, erscheint hier jede Anfrage des Agenten." The only action is en "Start session" / de "Session starten", and it is filled because the screen is empty (3.1).
<!-- /lang -->
4. **The connection breaks while the queue is running.** The shell stays. The setup screen replaces it only on a cold start, never in the middle of work — whoever has twelve waiting requests on the screen would otherwise lose the screen and not learn what has happened to the agent. Instead: the queue is marked as a frozen snapshot (timestamp in the header, all countdowns stop, all decision keys still), above it a banner with reason, consequence for the agent and the one action en "Reconnect" / de "Erneut verbinden".

As soon as there is traffic but nothing is selected, the context pane shows the summary of the session that its section owns (hosts seen, the five most frequent, findings in total) — a pane whose only content is its own heading reads as broken.

Internal vocabulary never stands on the screen: no sprint numbers, no issue identifiers, no word "placeholder". A control that does nothing yet says what it will do, or is absent. The target audience is designers and consultants; "Available from sprint 2" tells them that the product is unfinished, not what it can do.

### 4.3 Why this request is waiting

The sentence is built from the hold reason of the flow, not from a constant:

<!-- lang: de -->
| Reason | en | de |
|---|---|---|
| no rule matched | "Held: no rule matches · default: ask" | "Angehalten: keine Regel · Vorgabe: fragen" (HUM-052: shorter, so that the sentence is not cut off in the action bar) |
| a rule says ask | "Held: rule `<rule sentence>` says ask" | "Angehalten: Regel `<Regelsatz>` sagt fragen" |
| a finding | "Held: an AWS access key was found in the body" | "Angehalten: im Body steht ein AWS-Zugangsschlüssel" |
| unknown | names what is known, and invents no reason | likewise |
<!-- /lang -->

The `kind` identifier of the finding never stands in the sentence. It is internal vocabulary (4.2) and belongs in the detail line and in the Semantics value; the sentence names the translated name and the location of the finding.

<!-- lang: de -->
Today the same text stands there for every flow, centred between Block and Allow, at the most attention-grabbing spot of the screen — and it becomes a lie as soon as HUM-022 delivers real `ask` rules. What belongs in this middle is what changes: the reason, and the remaining time with its consequence named — en "auto-blocks in 1:47", de "wird in 1:47 blockiert" —, never a bare `mm:ss` whose direction and outcome one has to guess, and never a participle ("blockiert in 1:47") that leaves open whether it has already happened.
<!-- /lang -->

### 4.4 Diagnostics

Every error path delivers a `Diagnostic`, never a bare string.

- The `why` slot carries the daemon's sentence. The generic text of the app is the title, not the reason. Today the real reason stands in the mono detail line and a constant in `why`; that breaks both halves of principle 7 at once.
- `fix` is always passed on when the `Diagnostic` carries a `FixAction`. A `Diagnostic` with a `FixAction` and without a visible action is a defect.
- It is anchored at the place of the failure: inline below the control for a single decision, as a banner above the list with `Reload` for a data error, in the place of the skeleton for a failed waiting operation (2.11), full-screen only when nothing at all can be displayed. Distance between error and cause is the reason why people repeat the failed action instead of fixing it.
- A documentation link is accent if it is clickable. A URL that nobody can open is `fg1` mono with a copy button next to it.

### 4.5 What "Undo" means

"Undo" always undoes the rule, never the request. The strip reads "Rule saved · Undo" for `HMotion.undoWindow` (10 s) and says in the same breath that the request is already out. Afterwards only the strip disappears; the rule stays deletable in the Rules screen. For allow and block themselves there is no undo — hence the hold protection and the arming from 5.4. An undo whose reach someone guesses wrongly is worse than none, because they will rely on it in the next burst.

### 4.6 Motion and text say the same thing

The strip that stands in the decided row names the consequence, not the action: en "Sent to api.github.com · 2.1 KB", de "Gesendet an api.github.com · 2,1 KB". Blocked reads en "Blocked. The agent may retry.", de "Blockiert. Der Agent kann es erneut versuchen." No toast accompanies a motion; toast plus motion are two announcements of the same event.

After the confirmation window the strip is gone, but the action was irreversible. The status bar therefore carries the last decision permanently as one line — state glyph, host, time of day — until the next one replaces it. Three seconds are too short for the only trace of an action that 4.5 cannot take back.

**The verbs.** Three places, three wordings, and they are not mixed:

| Place | en | de | why |
|---|---|---|---|
| Control | "Allow" | "Senden" | The control names the action on the object. The German user sends a request, they do not allow it. |
| Rule | "allow" | "Erlauben" | The rule names the policy, not the single action; it applies to everything in the future. |
| Strip | "Sent to {host} · {size}" | "Gesendet an {host} · {size}" | The strip names the consequence, in the perfect tense, with target and extent. |

This settles that the German button does not say "Erlauben", even though HUM-028 begins the rule sentence with "Erlauben · ∗ · …" (deviation in section 8). `BACKLOG.md` section 5 already fixes this split; here stands why it is not a mistake.

Before "Remember" the rule stands as a sentence so that it can be checked, the default duration is `session`, and a control labelled "Allow all" exists only as a palette command that opens a host list.

As long as the grid remembers, **both** sentences therefore stand below the controls, one below the other: the sentence of the rule that Allow creates, and below it the one that Block creates (HUM-249). Which sentence stands there depends neither on the pointer nor on the focus. A keyboard shortcut asks neither of them: with the mouse over "Block", `A` creates an allow rule; with the focus on the valve, `Ctrl+L` creates a block rule; and if only the sentence of the highlighted action stood there, the saved rule would be one that nobody has read. A shortcut that first shows the sentence and demands a second press was rejected: it breaks the rhythm of the queue and would need a state that pointer, focus and keys maintain together. A neutral sentence without an action likewise: the rule names the policy (table above), and a rule without a verb is not one that can be checked. The sentences wrap and are never shortened with an ellipsis, not even at 480 px pane width and double font size: a shortened sentence is one whose end nobody has read, and the duration stands at the end. The same applies to the rule sentence in the modal (5.4). The row becomes taller as a result, but only together with the grid, which takes a line of its own anyway; the pointer that wanders between the buttons no longer changes any text (2.8). With the grid closed, holding the valve creates a rule for the session; its sentence stands from the press onwards, as long as the finger is down, that is the 400 ms before saving, in the place of the hold reason. If what the rule is built from changes during the hold — duration, target, request, domain —, the hold saves nothing and says why; it never saves a rule other than the one whose sentence it showed. The sentences take at most a third of the pane; whatever goes beyond that scrolls, and a decision that would save one of these rules is refused until the end of the sentences has been in view. The same in the modal: it stays in the window, its body scrolls, and confirming only works once the body has been in view to the end. Exactly the rule whose sentence stood there is saved (`ShownRule`, HUM-058).

### 4.7 When a secret goes along

A request that is held because of a finding is the one case that must not be calm. It must not look like a routine request, and it therefore gets four deviations from the normal case — and only these four:

- **In the row** the findings chip stands in `HColors.secret` instead of `fg2`. That is the only chroma a resting queue row may carry; the rest of the row stays neutral.
<!-- lang: de -->
- **In the action bar** the hold reason names kind and location of the finding in plain language (4.3), and the release valve switches to `warning`, carries the warning glyph (`triangleAlert`) before its label and relabels itself. The glyph has been necessary since HUM-260, because `warning` equals the accent in value and the colour alone no longer distinguishes a valve with a finding from a routine request: en "Send with 2 findings", de "Senden mit 2 Funden" (as already planned in HUM-049).
<!-- /lang -->
- **Allowing demands the same hold confirmation as blocking**, as long as at least one finding is unresolved, plus a sentence that names the consequence: en "An AWS access key goes to api.example.com", de "Ein AWS-Zugangsschlüssel geht an api.example.com". Only someone who knows what goes where presses and holds.
- **A click, `Enter` or `A` open, for a single request, the pause with the open findings** (HUM-049), instead of only refusing. It replaces the lower part of the card, grows open in 200 ms and lists per finding the kind, the shortened value and the location; at the foot stand "Send anyway" (`S`), "Pseudonymize" (`P`, opens the editor with all findings replaced) and "Block" (`B`), `Esc` closes it. The hold confirmation stays the way without a pause. For a group there is no pause: there the list of findings is missing, and it stays with holding or a key.

Explicitly **no** modal. The rule "never a modal for a single decision" (5.4) continues to apply here; a modal would also be the wrong protection, because it can be clicked away without reading the sentence. The protection sits in time, not in surface (5.4). Deviation in section 8.

### 4.8 When time runs out

A timeout is not a decision by a human but the absence of one. It is therefore the event that must leave the most trace, and today it leaves the least.

<!-- lang: de -->
- **Before**, the app warns once, at the row and in the action bar of the flow concerned, never as a toast: host, consequence and remaining time in one sentence — en "registry.npmjs.org auto-blocks in 0:30", de "registry.npmjs.org wird in 0:30 blockiert". As soon as the daemon knows extending, exactly one extension stands next to it. Until then the warning with the consequence named is the minimum.
<!-- /lang -->
- **At the instant of expiry** the rail sweeps to `timedOut` as with any other decision, and the row stays for three seconds as a grey row with the strip en "Blocked (timed out)" / de "Blockiert (Zeit abgelaufen)". That already stands in HUM-029; here stands that it is the same choreography as for a human decision, because the consequence for the agent is the same.
- **Afterwards** the status bar permanently carries a counter: en "{n} timed out this session", de "{n} abgelaufen in dieser Session". A click filters History to it. Whoever was away for five minutes must see on coming back that six requests were refused, without opening History and without remembering a banner they never saw.

### 4.9 Tray, notification and return

This is the only surface the human sees while the agent is stuck. It therefore gets rules of its own and not a line at the margin.

**The notification** has the same hierarchy as a screen (3.1): the host is the one important thing and stands large; method and path are secondary; the remaining time stands as a word, never as `mm:ss` — en "about 4 minutes left", de "noch etwa 4 Minuten". A notification is a still image in someone else's window manager; a digit in it is wrong from the first second. With the window focused no notification appears: what stands on the screen is not announced in addition (the same rule as toast plus motion in 4.6). Several arrivals update the same message, they do not stack.

**The tray icon** is a state indicator and carries state colour (3.3, rule 9):

| State | Icon | Colour |
|---|---|---|
| nothing is waiting | `idle` | `fg2`, neutral |
| 1 to 9 waiting, 10 and more | `held_n`, `held_9plus` | colour of `held` with digit |
| a timeout has blocked since the last focus | `alert` | `blocked` |

**The window** carries no title bar of the toolkit (HUM-255). The header bar of the app is the title bar: dragging on its free surface, on the wordmark, section title or badges moves the window, a double click maximises or restores, and its controls stay controls. On the right stand minimise, maximise and close as ghost buttons with hover and semantics labels, also on splash and setup of a cold start (there in a narrow bar that is a grip as well), also under tiling window managers that may ignore the first two. Close does exactly what closing via the window manager does; it is not the "Quit" of the tray menu.

**The return** replays nothing. Whoever comes back after fifteen arrivals sees the queue as it is, without arrival animations for things that happened minutes ago — an animation answers "what has just arrived", and none of this has just arrived (2.1). What happened during the absence stands in two calm forms: the return banner names the longest waiting time and leads to the oldest flow, the timeout counter in the status bar names the refusals (4.8).

### 4.10 Setup as a short onboarding

Setup is a short onboarding and not a list of cards (HUM-287). The owner picked the design on 2026-10-24 from three rendered variants: V1, a vertical stepper whose current step opens in place, with two things taken from the others. From V3 comes the plain-words headline of the open step, from V2 the "Next: …" beside its primary action. The images are `v1-*.png`, `v2-*.png` and `v3-*.png` with their `README.md` in the handover of the issue, and `impl-*.png` shows what was built.

**Four steps, in the order of the work.** Background service, model server, project folder, sandbox checks. The agent the daemon found (HUM-278) is a line of the model step, because the agent and its model are what a session runs. It stays one of the machine's lines and counts there, so a model server that answered is never reported as unmeasured because of another check.

**Green is one line.** A green step shows mark, title, state word and its evidence on one line of the grid's row height (`HSize.paneHeader`), with a quiet "Details" that opens it. Every step that is not in front stands as one line too. When a blocking step has a remedy with a button of its own, that button stays on its line (the folder chooser of HUM-282), so a second fix is still one click.

**The first step that is not green is open.** The order alone decides which step that is; the screen judges no step. The open step is a raised panel with a headline in plain words ("Your model server wants a valid access key", "Choose the folder the agent works in"), the step's own controls, and its finding inside the step with code, cause, detail and fix (4.4), drawn as a state rail and not as a card. Its action is the only primary button on the screen and takes the focus: the fix where it has a button of its own (Open settings through HUM-275, Choose folder, Install service), otherwise Contact it, Reconnect or Check again. Beside it stands the step after it ("Next: Project folder"), or "Next: start a session".

**One folder, chosen once (HUM-293).** Choosing a folder in Setup, on the sandbox screen or in Settings writes `sandbox.work_dir` into the user's `config.toml` through `SetConfig`; the daemon checks the folder with the same rules as a start before it writes, and writes it with its links resolved, so a link that later points elsewhere does not move the project. A refused folder is not stored, its `SANDBOX_006` stands in the project step, and the folder stored before stays. The app, the desktop launcher and `humanitl sandbox run`/`humanitl run` all take the stored folder; `--work` overrides it for one call, and the current directory counts only while nothing is stored. No folder lives only in the daemon's memory, so the app and the command line never start in two different folders. A running session keeps its folder; the new one applies to the next start.

**One choice, one button.** While the project step asks for a folder (no folder, or one the daemon refuses), its one control for that is the finding's "Choose folder" as the primary button; the folder picker with its own "Choose folder…" appears only once a folder holds, to change it or its mode.

**The one contact Setup makes by itself.** When Setup opens, it asks the daemon once to contact the model server saved in the configuration (owner decision 2026-10-24). This is the same `ProbeLlm` as "Contact it", with the stored key, and the daemon sends the key only to the origin of `llm.endpoint` (HUM-288). It asks only while the field shows the saved endpoint and no result for that endpoint stands. It asks again on "Check again" and when the saved endpoint changes, after a settings change or a switch of the saved server. It never asks on a timer, and never contacts an address somebody typed into the field. Every result carries a fingerprint of what it measured: the address, the active saved server, the reference of its key, whether a key is stored, and how often this app changed the key. The model step counts a result only when its fingerprint is the one of what the field shows now. A result for another address, another server or another key is "not measured" there, whether the address was typed, saved later or removed while the probe ran, and whether the key was replaced or cleared in Settings. A change of any part makes the contact rule ask again. The saved endpoint is the one Settings writes (`llm.endpoint`), not a sandbox snapshot. All probes ("Contact it", an address taken from the search, and the contact Setup makes by itself) go through one controller that numbers its requests, and only the latest request may set the result, so a late answer never replaces a newer one. Nothing else goes out: no search of the network and no other address. While no endpoint is saved, opening Setup contacts nothing at all. This replaces the earlier promise of HUM-075 that opening Setup contacts nothing.

**Ready.** With four green lines the heading says "Humanitl is ready", and there is no finding on the screen. Because of the one contact above, a machine on which everything is set reaches this state without a click. The one primary action is "Start a session", or "Go to Intercept" while a session already runs and the start is shut.

**Nothing needs scrolling.** At 1280×800 the open step, its action and the lines under it stand above the status bar in the owner's state of 2026-10-23 (key refused, no folder). The screen still scrolls at text scale 2.

**Where the app opens.** The app opens Setup when the first settled answer has a step that blocks the start or one that is still being asked (`SetupState.needsSetup`), and Intercept otherwise. A first start has no project folder, and a missing folder blocks (`CONFIG_013`). The first start is therefore the first case of the second, and the client keeps no "first start" flag of its own. A model server nobody asked to contact does not block, so an ordinary second start opens the queue.

**Checked by** `app/test/features/setup/setup_onboarding_test.dart` and the goldens `setup_firstrun`, `setup_partial` and `setup_ready_*`.

---

## 5. Keyboard and mouse

### 5.1 Parity

- Every pointer gesture has a key equivalent. Press-and-hold too: the release valve is reachable via a key or a two-step confirmation, otherwise the signature element is unusable without a mouse. The pill "+n new" too (2.8).
- Every element that reacts to tapping is reachable with `Tab` and can be triggered with `Enter` and `Space`. A bare `GestureDetector` or `MouseRegion` is never the only input path.
- Nothing that executes an action appears only on hover. Row actions also appear on focus and stay in the Semantics tree when they are invisible.
- The shortcut stands on the control that executes it: permanently on the two main decisions, on hover on everything else, after the `HoverLabel` pattern of the icon rail. Otherwise a mouse user cannot discover `Enter`, `A`, `B`, `J` and `K` by clicking.
- The command palette is the manual: it lists every action reachable by keyboard with its shortcut in the row, not only the five navigation entries.

### 5.2 Who the key belongs to

No `Shortcuts` widget above focusable controls may bind `Enter`, `Space` or a single letter in such a way that the focused control goes empty-handed. Today a keyboard user who focuses on "Block" and presses `Enter` allows the request — with `Space` the same control blocks correctly, and the wrong one of the two keys is the one every desktop user reaches for first.

The mechanism that fixes this is exactly one, and it is written out here because the obvious one does not work. `Action.overridable` does not help: an overridable action is overridden by the nearest **ancestor**, never by a focused descendant, and the collision arises earlier anyway — `Shortcuts` maps `Enter` to `AllowIntent` before any action lookup runs.

Instead: the `AllowIntent` and `BlockIntent` actions of the screen override `isActionEnabled` and return `false` as long as `FocusManager.instance.primaryFocus` sits on a control that handles `ActivateIntent`. `ShortcutManager.handleKeypress` returns `KeyEventResult.ignored` for a disabled action; the key thereby falls through to the default bindings of `WidgetsApp`, where `Enter` triggers `ActivateIntent`, and the focused control wins. The cheaper variant, should this prove brittle in testing: do not bind `Enter` at screen level at all, only `A` and `B`, and let `Enter` mean "activate" everywhere.

The queue is a single focus stop with arrow-key navigation inside. As long as the focus is in the queue, `Enter` = allow; if it is on a control, `Enter` applies to that control. That does not contradict the rule from 3.4 that only blocking happens from the row: the row is not the control, the **selection** is the subject. `Enter` allows the selected flow, whose URL the card shows, and the row itself carries no allow affordance that one could hit with the pointer.

**In the terminal the same question applies, and the answer is reversed.** There
every key belongs to the agent -- backspace, arrows, Enter, `Ctrl+C` as `0x03`,
`Ctrl+P` as `0x10` --, with exactly seven exceptions: `Ctrl+1` to `Ctrl+6` and
`Ctrl+K`. Without them one could only get out of a full-screen TUI with the
mouse, and that would violate 5.1. The way there is not the
obvious one: the emulator translates `Ctrl+K` into `0x0b` and `Ctrl+6` into `0x1e`
and reports the key as handled, so that the event no longer reaches any `Shortcuts`
above it; `KeyEventResult.skipRemainingHandlers` would stop the
emulator, but would end the journey just the same. `TerminalPane._onKey` therefore looks up
the key itself in `shellShortcuts()` and triggers the intent via
`Actions`. Measured in `a_shortcut_of_the_application_survives_the_terminal`
for all three cases (`Ctrl+1`, `Ctrl+6`, `Ctrl+K`).

`Ctrl+Shift+Enter` (HUM-298, also with the `Enter` of the number block) is
one more exception and takes the same way: it shows the terminal over the
whole window, without rail, sections and status bar, and brings back the
section and layout that were there before. The button at the right end of the
strip above the terminal does the same and names the chord in its tooltip.
`Esc` is not an exception; it stays with the agent, because OpenCode and pi
cancel with it. Measured in
`the_shortcut_works_with_the_terminal_focused_and_esc_reaches_the_agent`.
The chord is free in the queue as well: `Enter` allows, `Ctrl+Enter` blocks
and `Shift+Enter` opens the remember grid, and each of them demands its
modifiers exactly, so `Ctrl+Shift+Enter` decides nothing
(`the_chord_decides_nothing_in_the_focused_queue`). Not `Ctrl+Shift+F`: that
is the group allow of the queue.

**The terminal takes the keyboard on hover and by shortcut (HUM-303).** The
pointer over the emulator hands it the keyboard at once, without a click, in
its pane, over the whole window and later floating (HUM-300). Two exceptions
keep the focus where it is. A layer above the work -- a dialog, a sheet, a
menu, the command palette -- holds the keyboard in a focus scope of its own,
and the terminal takes the focus only from a node in its own scope or one of
the scopes around it; that covers every such layer without a list, and the
terminal's own context menu, which opens under the pointer and lets it
through, is the case that needs it. Every such layer takes the keyboard
when it opens, also from the terminal, and gives it back when it closes
(`HLayerScope`); `autofocus` alone would yield to whatever already holds
the focus, and `Esc` in the terminal's menu would reach the agent. A
palette opened over a sheet hands the keyboard back to the sheet and does
not offer the terminal. And a text field that has the focus keeps
it while the pointer only crosses the terminal; whoever wants the terminal
then clicks it or presses the shortcut. There is deliberately no delay of
some 150 ms: it would leave a moment in which the pointer is on the terminal
and an `Enter` meant for the agent still allows the selected request in the
queue. Focus at once, and the decision keys never fire because focus moved by
hover (`enter_goes_to_the_terminal_only_once_the_focus_moved`).

``Ctrl+` `` gives the keyboard to the terminal that is shown and changes
nothing in the layout; with no terminal on the screen it does nothing for
now (HUM-300 will open the floating terminal). It asks by the physical key
left of `1`, because on many layouts the backquote is a dead key or lies
behind `AltGr`; on a German keyboard the chord is `Strg+^`. It is one more
exception in the terminal and takes the way of the others through
`TerminalPane._onKey`, so the agent never receives it. A layer
above the work keeps the focus against the shortcut too; a text field does
not, because the shortcut asks deliberately. The palette lists it while a
terminal is shown.

### 5.3 Every bound key does something

Every activator in `interceptShortcuts()` has an action in the same screen. A widget test compares the two key sets and fails if they differ; a binding without an action is deleted, not disabled. Today `/`, `Ctrl+D` and `N` are bound and mute.

Every accepted key press produces a visible frame within 100 ms; the key decision shows the same 120 ms fill on the matching control of the action bar as a mouse click. A rejected press gets its own quiet rejection — the control that would have acted shows its fill and drains again, and the reason stands next to it. Never silence, never a shake, never a red flash. For a blind user a purely visible rejection would be a mute one; the rejection is therefore announced politely (section 6). Today a second `A` during a running decision returns nothing at all, while at the same time the ghost buttons disappear, and a quick `A A` reads as if the program had frozen.

### 5.4 Care grows with reach and reversibility

Danger is not a measure: nobody knows in advance whether a `POST` to an unknown host is bad. Reversibility is one. That is why the protection has two axes, and that is why blocking is ghost, although it is the seemingly harsher action — the agent may try again. Allowing is filled, because it is the action that matters; its protection sits in time, not in surface.

**Reach:**

| Reach | Protection |
|---|---|
| one request | 250 ms hold (`HMotion.holdToBlock`) |
| two to five requests | 250 ms hold |
| more than five, or a rule with `forever` | Modal |

Never a modal for a single decision: a modal per click destroys the rhythm of the queue. A modal traps the focus within itself, focuses the non-destructive action, closes on `Escape` and silences the decision shortcuts of the screen as long as it is open.

**Reversibility.** Allowing is irreversible (4.5), `Enter` is a single key, and the selection jumps to the next held flow after every decision. Three provisions keep this apart:

1. **Arming.** `Enter` and `A` only fire once the URL of the selected flow has stood in the card for at least `HMotion.rearm` (350 ms). Every new selection re-arms — including the one the program itself sets after a decision. Blocking stays available at once. A saccade to a new row plus a fixation takes about 300 ms; whoever decides faster has not read.
2. **Key repeat never decides.** A `KeyDownEvent` with `repeat == true` is discarded. After an accepted decision, `Enter`, `A` and `B` stay locked until the matching `KeyUpEvent` has arrived.
3. **The selection waits.** It does not move on after a decision as long as a decision key is still pressed. Otherwise rule 1 arms against a finger that has not let go at all.

**When the recording does not write** (HUM-250), allowing additionally can no longer be looked up later. The daemon reports the disruption (`RECORDER_001`, `RECORDER_003`) and its end (`RECORDER_005`); as long as it lasts, the status bar shows "Recording disrupted" (de "Aufzeichnung gestört") in the error colour, every held card and every card that has arrived since carries "Not being recorded", and the first release of the session stops at a pause in the place of the decision bar, like the pause with open findings: no modal. "Send unrecorded" (`S`) only accepts after `HMotion.rearm` and then applies for the rest of the session; `Esc` closes the pause without sending.

Mandatory test: holding `Enter` down for 500 ms produces exactly one `Decide`, not several. Without these three provisions a held-down `Enter` allows half the queue unread — today the only protection is `isSending`, and that ends with the daemon's answer after a few milliseconds.

The two decisions are also larger than everything else: `HSize.hitMin` (28 px) is the lower bound for minor things, allow and block measure at least 32 px in height and 120 px in width, as HUM-028 names it for the release valve anyway. A 28 px target next to another 28 px target is exactly the geometry in which a hasty click misses — and what lies next to it here is the irreversible action.

The note field when blocking (HUM-072) is temporary: hidden until `N`, closed on `Escape` or on a decision, never a permanent line of the action bar. A permanently visible optional text field adds a focus stop, a border and a character counter to the bar and invites typing where the normal way is a key press.

---

## 6. Accessibility as numbers

> **As of 2026-09-04: deferred until 1.0.0.** The project owner has
> decided that the MVP should first prove that the thing holds up and can be
> good; accessibility as a programme comes afterwards. What stands in this
> section remains the goal and remains worth reading, but no longer blocks any
> issue: the screen reader announcements, the second channel in its full
> gradation, the check at `TextScaler.linear(2.0)` and the 4.5:1 as a
> test gate are dormant. Where one of these numbers is already reached today, it
> stays — deferred means "do not wait for it", not "tear out again
> what has been achieved".
>
> **Four things continue to apply**, because they only look like accessibility and
> are in truth security or plain usability:
>
> 1. A hold duration keeps its full time, even if the platform
>    reports `disableAnimations`. Flutter otherwise shortens it to five per cent, and
>    then an ordinary click sends where a hold was demanded; as measured,
>    60 milliseconds were enough.
> 2. The hit areas of the decision stay: at least 28 × 28 px
>    in general, at least 32 × 120 px for allow and block. Whoever slips
>    on an irreversible action does not have an
>    accessibility problem, but a sent request.
> 3. Text stays above about 3:1. That is no longer a standard, but
>    legibility: `shadcn_flutter` paints its destructive button at 1.97:1,
>    and that is unreadable for everyone, not only for some.
> 4. A click shows a visible reaction. A control that does nothing
>    feels broken.
>
> See ADR-0009, section "Revised on 2026-09-04 by the
> project owner".


- **Contrast.** Text reaches 4.5:1 against the surface it really stands on — that is, also against a 10 % tint and against a fill, on all four surfaces of both ladders. Surfaces and rails reach 3:1, with the one named exception of the resting `held` rail (3.3, rule 10). The test in `app/packages/ui/test/tokens_test.dart` today only checks 3:1 for state colours as a surface; it is extended to text-on-tint and text-on-fill and fails below 4.5:1. Measured violations today: DELETE label on its tint over `bg3` 2.65:1, `held` label light on its tint 2.75:1.
- **Secondary text.** `fg2` measures 3.02:1 on `bg3`, `lFg2` 3.24:1 on `lBg3`. Both are reserved for genuinely disabled controls. Every sentence someone is meant to read is `fg1` or better; today that concerns the empty state of the queue and the titles of the collapsible sections.
- **Focus.** 2 px accent ring outside its own border. Never by recolouring an existing border, never in the colour the control is filled with. On the primary button the focus border measures 1.00:1 against the fill today — a keyboard user sees no difference there.
- **Hit targets.** At least 28 × 28 logical pixels (`HSize.hitMin`), for allow and block at least 32 × 120 px (5.4), measured in the widget test, not claimed. Known violation: the eye toggle that uncovers a masked secret measures 28 × 19.2 px.
- **Text scaling.** Up to `TextScaler.linear(2.0)` without `RenderFlex` overflow and without a cut-off paragraph. No box that contains text has a fixed height; all row heights from 3.2 are minimum heights, and the action bar wraps into a column when the scaled contents no longer fit side by side. Widget test and golden at 2.0 for queue row, action bar, header and status bar — fixed heights swallow the overflow silently, so without this test there is no failure, only overlapping text.
- **Second channel, graded.** The glyph stands next to the state colour everywhere, without exception. The translated plain-language label stands wherever space is reserved for it: card, detail view, group header, action bar, confirmation strip. Where a column measures 28 px, like the state column of the History table, Semantics label and tooltip carry the word — and they carry it **always**, in every view. Without this grading one person widens the column and another ticks off the checklist wrongly. In the light theme `allowed` and `blocked` measure 1.01:1 under deuteranopia; the glyph is therefore mandatory and not decoration.
- **What is announced** — today there is neither `liveRegion` nor `SemanticsService.announce` in the program:
  - **Arrivals bundled, politely**, at most one announcement per two seconds: en "3 new, oldest GET api.github.com", de "3 neu, älteste GET api.github.com". Fifteen arrivals in twenty seconds would otherwise put fifteen full URLs in the ear.
  - **A self-triggered decision politely**, with host, extent and whether an undo window is running. Only what the human did not trigger is announced assertively: the timeout pre-warning and the timeout that has occurred.
  - **A rejected key press politely**, with the reason from 5.3.
  - **The hold budget** only at the thresholds 5 min, 1 min, 30 s and 10 s, and assertively only at 5 %.
- **No label changes more often than once per second**, and the countdown is not a live region. A label that changes four times per second is the audible form of nagging. The remaining time therefore stands not in the **label** of the row but in its **Semantics value**, and is only spoken again at the four thresholds named; a label with `mm:ss` changes once per second and is repeated in full by every screen reader on the focused row every time. At the same time the countdown is no longer hidden via `ExcludeSemantics`: the row carries state, index, total, method, host and path in the label and the remaining time in the value. A widget may never be the only carrier of a deadline and at the same time be excluded from semantics.
- **Reduced motion** per 2.10.
- **A timeout is not a decision by a human**, per 4.8.

---

## 7. Performance as a budget

A budget that no test can check is a declaration of intent. The table therefore separates two kinds. **Gates** run in `flutter test`, block CI and measure only what can be determined in a fake-async environment without a rasteriser: build counts, element counts, call counts. **Measurements** run in `integration_test` with `IntegrationTestWidgetsFlutterBinding.watchPerformance` under Xvfb or in DevTools, are reported in the PR and do not block: frame and raster times, memory. `AutomatedTestWidgetsFlutterBinding` never rasterises, so every `FrameTiming` there is meaningless.

| Case | Budget | Kind |
|---|---|---|
| 200 held rows, five arrivals in one frame | no frame over 16 ms; p95 over 120 frames in the profile build on the machine named in `CONTRIBUTING.md` | Measurement |
| one decision | two row builds, not two hundred | Gate |
| one second of clock | at most one build per visible row | Gate |
| one splitter frame | no build of the card, its header and its body preview | Gate |
| 8 MiB body, 10k History rows | no visible stutter | Measurement |
| 8 MiB body, 10k History rows | memory below 300 MB | Measurement, DevTools; `ProcessInfo.currentRss` does not see the GPU memory |

Rules that keep these budgets:

- **Two clocks, not one.** The **UI clock** (`nowProvider`, `HMotion.clockTick`, whole seconds) drives labels and rings and stands still as soon as its section is invisible. The **policy clock** is coarse (5 s), never stops and only fires what must also apply in another window: the timeout pre-warning (4.8), the tray counter and the announcements (section 6). Whoever makes both into one kills, by saving, exactly the two abilities the program exists for while the human is elsewhere.
- **The ring gets no controller of its own per row.** Over five minutes the arc end of a 16 px ring moves about 0.15 px per second, so about 0.003 px per frame; two hundred tickers for subpixels are not a budget but a bill without a return. The ring reads the same seconds clock as everything else. Only below `HMotion.ringSmoothBelow` (60 s) does it get an `AnimationController`, because the arc runs about 0.8 px per second there and the second steps become visible as stutter. Only what says something below one second needs a controller of its own anyway: the hold fill and the rail sweep.
- **Visibility is a provider, not a private field.** The Intercept screen today computes the flag in `_InterceptScreenState`, where nobody can read it; it becomes a provider that the UI clock observes. `nowProvider` loses `keepAlive` in the process — today its doc comment claims the timer stops with the last observer, and `@Riverpod(keepAlive: true)` makes sure it does not.
- **A provider that filters, sorts or groups never sees a clock.** Otherwise an O(n log n) projection runs over the entire flow map per tick, and n is the traffic of the whole session, not the length of the queue. Time-dependent rows get a one-off timer.
- **The flow map is bounded.** Decided flows drop out as soon as their exit window is over; History paginates from the daemon, not from this map. A growing map that is copied in full per event turns one event into an O(n) copy.
- **Every derived collection provider returns a type with value equality**, never a bare `List` or `Map`. The model is `QueueSnapshot` with `listEquals`.
- **Scalars with `.select`.** Never `.length` on an observed collection.
- **Two builds per decision are only reachable with memoisation.** `SliverChildBuilderDelegate.shouldRebuild` returns `true` unconditionally, and a `setState` of the pane per snapshot rebuilds every built row. The pane therefore holds `List<FlowId>` and remembers **one** `QueueRow` widget instance per `FlowId`; the row observes `flowProvider(id)` and `selectedFlowIdProvider.select(...)` itself. `Element.updateChild` stops at an identical child widget, and that is the only way to the number. `flowProvider(FlowId)` does not exist yet (section 9).
- **A row only observes providers keyed on its own `FlowId`.** Shared state such as the running decision is read where it is displayed.
- **Pointer presence and freeze state stay in the `State` of the pane.** The frozen order is view state and dies with the pane; only the counter of pending arrivals is a provider, because the pill and the announcement share it. Pointer movements never reach the provider graph. This deviates from HUM-029, see section 8.
- **Continuous drag state lies in a `ValueNotifier`** that only the layout widget listens to. A provider write per pointer movement rebuilds the whole screen 120 times per second.
- **Fixed row heights are known to the list where n is large and no row changes its height**: History via `FixedSpanExtent`, body and hex views via `itemExtent`. The Intercept queue is exempt (2.4), because it needs `AnimatedList` and because its row height is not a constant at larger text scales anyway. Without an extent, scrolling costs linear instead of constant time — at ten thousand rows that is the difference, at two hundred it is not.
- **Animation wrappers leave the tree** as soon as `animation.isCompleted` holds. A lazy list builds only its viewport, so a queue carries not six hundred superfluous render objects but about two per visible row — in a 660 px high pane about thirty-six. Exactly those are paid for per frame, and exactly that is why they disappear. `CurvedAnimation` is a `late final` field of a `State` and is disposed, never an expression in `build`.
- **`FadeTransition` and `SlideTransition` instead of `Opacity` plus `Transform` in an `AnimatedBuilder`.** The transitions do not rebuild the child tree per frame; the `AnimatedBuilder` detour does, and in the two places that are the most expensive anyway.
- **`RepaintBoundary` exactly around the islands with their own clock**: countdown ring, card sweep, breathing glyph — and around the arriving and the leaving row, because an opacity layer is paid for there per frame. Nowhere else; elsewhere it only costs a texture.
- **`Shortcuts` and `Actions` maps are `late final` fields**, not expressions in `build`. Children that a rebuilding parent passes through unchanged are `const` or fields.
- **No formatting, shortening or analysis in a layout callback.** Compute once against the constraints and cache against these constraints; `middleEllipsis` allocates a full rune list per call.
- **Everything that touches a body** runs in `Isolate.run` above 64 KiB and returns only simple Dart values.

How a test proves this:

1. **Rebuild counter (gate).** A static counter variable in the widget's `build`, a state change through the container, the exact number checked. Mandatory per screen: `QueueRow` per decision, `HeaderBar` per event, `InterceptScreen` per splitter frame, History row per live update. Rebuild extent is invisible to compiler and goldens and otherwise regresses silently.
2. **Load gate (gate).** The fake daemon knows `burst:200`. A standing widget test uses it to pin down the number of `QueueRow` builds per second of fake time and per decision.
3. **Frame budget (measurement).** For 10k History rows and an 8 MiB body, an `integration_test` under Xvfb measures frame and raster times; the number stands in the PR. HUM-030 today only measures UI thread times via `SchedulerBinding`, and HUM-032 lists the frames as a risk with a DevTools measurement — both are less than this table demands and are extended accordingly.

---

## 8. Where this document deviates from a specification

So that the deviation stays checkable, it stands here and not between the lines.

- **HUM-028:** The confirmation strip belongs to the decided row, not in the card, and it is not an extra 28 px box but the swapped content of the same row (3.4). In the card it would block, for three seconds, exactly the spot where the next decision is made.
- **HUM-028:** The release valve carries the accent tint at the token cap (10 %), not 12 %. `HColorDerivation.tint` caps at `HColors.tintAlpha`; a value the token layer cannot produce is no specification, and nobody sees two per cent of alpha.
- **HUM-028 and `BACKLOG.md` 5:** The German control says "Senden", not "Erlauben"; "Erlauben" belongs to the rule sentence (4.6).
- **HUM-020 and `BACKLOG.md` 5:** The queue row stays 36 px high in every state and has no second line, because this line only repeats what the card next to it shows, and makes the list wander under the eye in doing so. 36 px is a minimum height that grows with the text scaling.
- **HUM-020:** From the row only blocking is possible. Allowing is irreversible and demands the URL, and that stands in the card.
- **HUM-029:** The group header shows only `Block {n}`; the action bar takes over `Allow {n}` as soon as a group is selected (3.5).
- **HUM-029:** The frozen order lives in the `State` of the queue pane, not in a `queueFreezeProvider`; only the counter of pending arrivals is a provider (section 7).
- **HUM-029 and section 7:** The Intercept queue keeps `AnimatedList` and does without `itemExtent`; the extent obligation applies in History and in the body views (2.4).
- **HUM-030:** Values in the JSON tree stay on the `fg` ladder; findings are the only chroma.
- **2.9, idle:** The caution tape (3.6) runs endlessly as long as its cause persists. It is the only continuous motion besides the bounded breath; the project owner decided on it on 2026-10-20 as a sign of the brand (HUM-260). It stands only in the two places that 3.6 names.
- **HUM-034:** The tray counter stands in the colour of `held`, not as an accent dot. The tray icon is a state indicator, not a control (3.3, rule 9).
- **HUM-049:** With at least one unresolved finding, allowing demands the same hold confirmation as blocking, plus a sentence that names the secret type and target host — but no modal (4.7).
- **`BACKLOG.md` 5:** The glyph breathes in a bounded way (two thresholds, three breaths each) instead of permanently, it brightens instead of fading, and a glyph that crosses a threshold starts its first breath at full opacity (2.7).
- **`packages/ui`:** The resting `held` rail in the queue is exempt by name from the 3:1 rule (3.3, rule 10). Every other surface and every other rail reaches it.
- **2.9 and `HTextField`:** The focus ring of this **one** control fades in over 150 ms and reserves no space, but draws beyond its own edge. 2.9 demands a ring in one frame, because one that fades in reads as input lag. Since the revision of ADR-0009 the input field stands on the `TextField` of `shadcn_flutter`, and that builds in its `FocusOutline` firmly — without a switch. Two rings on top of each other would be one too many, so it is theirs. Its measurements are ours: two pixels of accent, two pixels of gap, set via `FocusOutlineTheme` from `HFocusRingMetrics`. Every other control continues to carry `HFocusRing`, which reserves its space and does not animate. Under reduced motion the fade-in of the library ring drops to 7.5 ms, because its animation primitive is built without `animationBehavior`; the visible result is then one frame again.

---

## 9. What the design system still needs

> **As of 2026-09-04.** A dedicated pass has closed most of this
> list: `HRow` with state glyph, focus and action slot,
> `HFocusRing`, `HSkeleton` and `HWait`, `HTextField`, `HSegmented`,
> `HCheckbox`, the missing size tokens, five glyphs, and above all the
> contrast defect: state colours now have a text variant of their own that reaches
> 4.5:1 on all four surfaces of both ladders and on tint, hover and press fill,
> while the surface colours stay as they were. Whatever still stands
> below and is not marked as closed continues to apply.
>
> **Addendum 2026-09-04, second pass.** Two reviews brought nineteen
> findings; they are fixed, and each has a test that turns red without
> the fix. The breath ends at full opacity again (`repeat(count:)`
> counts half cycles), the split pill paints word and chevron in the
> text variant of its surface, the accent has, with `accentText` and `accentFill`,
> the same separation of surface and word as every state colour, the focus ring
> keeps two pixels of surface clear where it would otherwise lie on a fill that cannot be
> told apart from it — that is, on the primary button, measured and
> not enumerated (`HFocusRing.needsGap`) —, the hold fill is a
> token (`HColors.fillHoldAlpha`) and stands in the list against which the
> text derivation calculates, `HSheet` traps the focus and closes on `Escape`,
> the header line of a collapsible section and the splitter of the panes are
> focus stops with keyboard operation, and `HMotion` has taken up the two deadlines
> that previously stood as literals in `core/ui`. The
> hold fill in the text derivation shifts the text palette slightly; the
> frozen tables in `tokens_test.dart` are new accordingly.
>
> **Addendum 2026-09-04, third pass.** Ten more findings, likewise
> each with a test that turns red without the safeguard. The hold fill of the
> pill lay on the pill's own tint and composited to 0.248 instead of 0.20;
> its alpha is now calculated back (`HColorDerivation.alphaOver`), and
> `holdFill` in `core/ui` takes the same route. A disabled checkbox and
> a disabled segment carry `fg2` instead of `fg0` and no accent any more;
> the checkbox fills with `accentFill` like the primary button. The key fill
> keeps its 120 ms even with animations switched off: `AnimatedContainer`
> builds its controller without `animationBehavior`, so
> `HAnimatedFill` takes over the colour of button, row and segment. The remembered
> variant of the pill has a keyboard path (`Enter` or Space held),
> the border of the pill carries the same tone as its word, the
> header line of a collapsible section has a minimum height instead of a height and
> its curve as a field instead of an expression in `build`, and the width of the
> splitter grip is a token (`HSize.splitter`, odd, so that the
> hairline lies on a whole pixel). Since then `core/ui` has a
> test of its own at double text scaling; it found the header of the
> diagnostic card, which now wraps instead of cutting off.
>
> One decision of this pass goes against first appearances: the
> state glyph keeps the **surface colour**. The doc comment of
> `stateTextColor` counted it among what takes the text variant, and the code
> did not; it was the comment that was corrected. A glyph is a
> graphic, its threshold is the 3:1 from section 6, and the
> surface palette keeps that on every surface of both ladders. With the text variant,
> `autoRule` of all things would lose its tone — the colour carries 60 % opacity,
> it reaches 4.5:1 only close to white (dark) or black (light) —, and the
> glyph is the channel that doubles the colour for the colour-blind (3.3).

The following have grown: `motion.dart` (`leaveOffset`, `stagger`, `staggerMax`, `leaveGlideFraction`, `holdToBlock`, `rearm`, `confirm`, `undoWindow`, `freezeAfterKey`, `freezeAfterPointer`, `clockTick`, `waitVisible`, `waitMinVisible`, `ringSmoothBelow`, `breatheBelowUrgent`, `breatheCycles`, `breatheMinOpacity`, `reducedRingAlpha`, `hoverLabel`, `copyFeedback` as well as `HReducedMotion`), `colors.dart` (`fillHoverAlpha`, `fillPressedAlpha`, `fillHoldAlpha`, `textMinContrast`, `areaMinContrast`, `fillAlphas`, `textBackgrounds`, `textVariant`, `readableFill`), `spacing.dart` (`rowHistory`, `rowBody`, `rowActionSlot`, `hitDecision`, `measureChars`, `splitterActive`, `splitterStep`), `flow_state.dart` (glyph and ARB key per state) and `tokens.dart` (`stateText`, `methodText`, `accentText`, `accentFill`, `stateTextOf`). With HUM-260 came `HColors.onAccent`, `HColors.warning`, `HColors.info` (together with light variants and `colors.warningText`, `colors.infoText`), `HMotion.cautionTape`, `HMotion.cautionTapePeriod`, `HSize.cautionTape` and the widget `HCautionTape`. Everything that follows is open and is decided in issues of its own, never in passing.

**Tokens:**

1. ~~`HSize.rowSelected` (56) is no longer used by any row per 3.4. Either delete it or repurpose it for the History detail row.~~ Closed on 2026-09-04: repurposed for the two-line detail row of History.
2. ~~`HSize.selectionRail` (2) is dropped as soon as the selection replaces the state rail instead of overlaying it.~~ Closed on 2026-09-04: the selection replaces the rail across all four pixels, and the token now marks the active section of the icon rail.
3. ~~The three row densities from 3.2 need three tokens: `HSize.rowHistory` (28), `HSize.rowBody` (24) and `HSize.rowActionSlot` (28).~~ Closed on 2026-09-04: all three stand in `spacing.dart`, the action slot no longer borrows `hitMin`.
4. ~~A lower bound of its own for the two decisions: `HSize.hitDecision` (32 px height, 120 px width) next to `HSize.hitMin` (5.4).~~ Closed on 2026-09-04.
5. ~~The method hues do not reach 4.5:1 as text on their own tint (DELETE dark 2.65:1). Either carry label and surface colour separately or adjust the hues.~~ Closed on 2026-09-04: `HMethodColors.darkText` and `lightText` carry the abbreviation separately from the surface, derived via `HColorDerivation.textVariant`.
6. ~~The countdown ring takes `tokens.colors.line` as its track; in the light theme the arc measures 2.84:1 against it.~~ Decided on 2026-09-04, in favour of the gap: the ring now only draws the remaining arc, the consumed distance stays surface. A track would have had to meet two conditions at once — be visible on every panel surface and leave the arc 3:1 —, and the derived light track did not meet the first: it measured 1.01:1 to 1.26:1 against the four light surfaces and could not be seen on any panel. The arc alone, as a state colour, reaches its 3:1 on every surface of both ladders. The token `ringTrack` is thus gone again.
7. ~~There is no token for the measure upper bound from 3.2. Proposal: a character count (90), not a pixel width.~~ Closed on 2026-09-04: `HSize.measureChars` (90) and `HSize.measureWidth`.
8. A rail that reaches 3:1 would need an alpha token of its own far above `HColors.tintAlpha`. This document does not want it (3.3, rule 10); whoever wants it anyway introduces the token and must not misuse `tint()` for it.

**Widgets in `packages/ui`:**

9. ~~A leaving row needs a widget of its own without `ref`: `QueueRowSnapshot` (or `HRow.frozen`).~~ Closed on 2026-09-04, and without a second widget: `HRow` observes nothing and takes exclusively resolved values — state, glyph, method, host, path, remaining time as a string. So the same class serves for the living and for the leaving row, and the exit draws a frozen image with a stopped countdown (2.4). The doc comment of `HRow` says the same; the two texts contradicted each other until this one was brought in line.
10. ~~`HRow` paints hover and selection both `bg3`. Hover must become `bg2` (3.4).~~ Closed on 2026-09-04.
11. ~~`HRow` uses `HMotion.sweep` for hover tint and height. Hover belongs to `HMotion.press`, the height does not animate.~~ Closed on 2026-09-04.
12. ~~`HRow` has no slot for a state glyph and is not focusable. Plus a slot for the multiple-selection rail (3.5).~~ Closed on 2026-09-04; the header of this section says the same.
13. ~~`HMethodBadge` needs a neutral variant (`fg1` on `bg2`) for lists; the coloured one stays reserved for the card header.~~ Closed on 2026-09-04.
14. ~~`HButton` draws focus by recolouring the existing 1 px border. What is needed is a 2 px accent ring outside; two `BoxDecoration`s are enough for that, a new token is not needed.~~ Closed on 2026-09-04: `HFocusRing` draws the ring outside, `HFocusRing.inline` on the edge of the split pill.
15. `HStateGlyph`: `easeInOut`, the lower bound `HMotion.breatheMinOpacity`, `HMotion.breatheCycles` per threshold, the start at full opacity and the resting substitute ring under reduced motion have been in place since 2026-09-04; the end of the breath as well (`repeat(reverse: true, count:)` counts half cycles, an odd number ends on the lowest opacity). Only the phase remains open: the glyph has a controller of its own instead of taking its phase from the clock time modulo `HMotion.breathe`. Two rows that drop below the threshold at the same moment therefore breathe in step, two from different moments do not.
16. ~~`HModal` has neither a `FocusScope` nor an `Escape` binding.~~ Closed on 2026-09-04: `FocusScope` with `autofocus`, `Escape` via `DismissIntent`. `HSheet` had the same gap and has had the same treatment since 2026-09-04; without it `Tab` ran through the screen that the sheet was covering.
17. ~~`HPill`, `HIconButton`, `HRow` and `HBadge` with `onTap` are not focusable; `HButton` is today the only focusable control in the system.~~ Closed on 2026-09-04: ten of the 21 widgets carry focus handling of their own, among them all four.
18. ~~`HBadge.chipHeight` and `HButtonSize` are fixed heights and overflow at `TextScaler` 2.0. Minimum heights instead of heights.~~ Closed on 2026-09-04; since the same day the test at double scaling also covers `HPill`, `HSegmented`, `HIconButton` and `HModal`.
19. ~~A skeleton widget for 2.11: hairlines at the height of a target density, `fg2`, without motion, with the two thresholds `waitVisible` and `waitMinVisible` built in.~~ Closed on 2026-09-04: `HSkeleton` and `HWait`.

**In `app/lib`, to be decided outside this document:**

20. `flowProvider(FlowId)` does not exist; section 7 needs it for the memoisation of the rows.
21. The visibility of a section needs a provider; today it is a private field of `_InterceptScreenState`.
22. `nowProvider` loses `keepAlive` and becomes the UI clock; the policy clock from section 7 comes next to it.
23. `ConnectionGate` today replaces the whole shell as soon as the connection fails — even in the middle of work. 4.2 item 4 demands that the setup screen only takes over on a cold start.

**Tests:**

24. ~~The contrast test checks colour-on-surface at 3:1. It needs text-on-tint and text-on-fill at 4.5:1.~~ Closed on 2026-09-04: `every state colour carries text at 4.5:1 in both themes`, `every method hue carries its verb at 4.5:1 in both themes` and `every fill of the system is a fill the derivation saw` in `packages/ui/test/tokens_test.dart`; the exception from 3.3 rule 10 stands by name in the test `the area palette is exactly what it claims`.
25. There is no test that proves every state colour is accompanied by glyph **and** label. (The second half of this item — a test at `TextScaler` 2.0 — is done with item 18: `packages/ui` has checked the scaling for ten controls since 2026-09-04, `app/lib/core/ui` for its three since the same day.)
26. An ARB lint checks the forbidden words from 4.1 in both languages and knows the three statement keys as exceptions.
27. A widget test holds `Enter` down for 500 ms and expects exactly one `Decide` (5.4).

**What the library delivers and what it does not (ADR-0009, "Revised on 2026-09-04 by the project owner"):**

`shadcn_flutter` has been adopted since 2026-09-04 and delivers Resizable,
Command Palette, Sheet and ContextMenu. Toast and Menubar are dropped nonetheless:
a toast contradicts 2.11, 4.6 and 4.8, and the two ways to every action
are the icon rail and the command palette (5.1), not a menu bar.

The library delivers the look, not the behaviour layer underneath. It
computes a contrast value nowhere, does not know `AnimationBehavior`,
has three `Semantics` nodes in the whole library, no
hold-to-confirm, no minimum hit area and, on the desktop, no
pressed state. What of this applies in the MVP anyway stands in the ADR: the
hold duration keeps its time, text stays above about 3:1, and a click shows
a reaction. The rest is deferred until 1.0.0.

28. ~~A context menu is missing.~~ Closed on 2026-09-04: `shadcn_flutter`
    brings one along. It is needed twice, "Copy value" and "Copy path" in the
    JSON tree and copy/paste in the terminal; it is wrapped in `packages/ui`,
    not in one of the two features, because both need the same keyboard handling and
    the same focus ring.
29. ~~A date-time picker is missing.~~ Closed on 2026-09-04: the library
    brings one along. When wrapping it, it still applies that the time of day in the
    time-range filter of the audit screen stands in monospace, because it is part of the
    evidence (`backlog/CONVENTIONS.md` 4.13).
30. ~~`HResizablePanes` only knows the horizontal axis.~~ Closed on
    2026-09-04: the library knows both axes. Its two open bugs concerning the
    vertical axis (#427, #428) were the reason why HUM-035 listed it as a risk;
    when wrapping, it has to be checked whether they affect us. Since
    HUM-305 our own `HResizablePanes` has the vertical axis as well (the
    Sandbox section stacks terminal and tabs); the library is still not
    wrapped.
31. `FocusRing` in `app/lib/core/ui` and `HFocusRing` in `packages/ui` are
    the same thing twice. The comment that justified this with a state
    that item 14 has removed was corrected on 2026-09-04; the file
    stays, because five controls of the action bar and of History use it
    (`block_button`, `note_field`, `release_valve`, `remember_grid`,
    `history_table`). Until the move it reads its geometry from `HFocusRing`,
    so that the two rings do not drift apart; a test keeps the two
    spacings equal. When the handoff widgets move, one of the two remains.
32. No screen has ever called `HPill` and `HPanel`; they stand only in
    the gallery. Either they get a caller or they are dropped.

---

## 10. Checklist before "done"

- [ ] Every animation answers one of the four questions from 2.1; every duration, curve and distance comes from `HMotion`, no literal in a feature file.
- [ ] Every overlay — sheet, modal, palette, banner, section and tab change — takes its row from 2.2 and invents no motion of its own.
- [ ] Nothing moves that is being read; no number, no focus ring and no row height animates because of a state change.
- [ ] Distances and loops run through `HReducedMotion`, every loop has a resting substitute, and the exit keeps its feedback under reduced motion (2.10).
- [ ] Every waiting operation follows 2.11: nothing below 150 ms, after that a skeleton in the target density, standing for at least 400 ms, no spinner, no shift on arrival.
- [ ] The screen has exactly one filled control per decision context, one empty state and one largest text element, and that is the most important thing on it.
- [ ] Every state colour is accompanied by the glyph, by the plain-language label wherever space is reserved, and by Semantics and tooltip always; accent only on what is focusable or touchable; chrome without a second value carries `fg1`.
- [ ] All spacings are multiples of 4, the right gutter measures 12 px, the measure at most 90 monospace characters — except in code, hex and tables, which scroll horizontally and never wrap.
- [ ] Every empty state names the next event or, for a filter, the number of matches and the way back; no "nothing", no "yet", no internal vocabulary, every text from ARB in both languages.
- [ ] The four states before the first request from 4.2 are built, and the setup screen replaces the shell only on a cold start.
- [ ] A timeout leaves a pre-warning at the flow, three seconds of grey row and a permanent counter in the status bar (4.8).
- [ ] A request with a finding carries the chip in `HColors.secret`, the reason in plain language and the hold confirmation on allow (4.7).
- [ ] Every error path shows a `Diagnostic` with the daemon's `why` and with `fix`, anchored at the place of the action.
- [ ] Every activator has an action, every pointer path a key, every accepted press a visible reaction within 100 ms, every rejected one a quiet and an announced one.
- [ ] Neither `Enter` nor `Space` nor a single letter wins against a focused control; one test compares both key sets, and a second holds `Enter` down for 500 ms and expects one `Decide`.
- [ ] After every new selection allowing is disarmed for `HMotion.rearm`, key repeat never decides, and the selection does not move on as long as a decision key is pressed.
- [ ] Contrast: text ≥ 4.5:1 also on tint and fill, surfaces ≥ 3:1 with the one named exception; hit targets ≥ 28 × 28 px, allow and block ≥ 32 × 120 px, measured in the test.
- [ ] Widget test and golden at `TextScaler` 2.0 without overflow; Semantics carry state, index and total in the label and the deadline in the value; no label changes more often than once per second; arrivals are bundled.
- [ ] A rebuild counter test pins down the builds per decision, per second of clock and per splitter frame; the frame and memory numbers stand as a measurement in the PR, not as a gate in the unit test.
- [ ] `make check` green, goldens green, and every deviation from a specification stands in section 8.
