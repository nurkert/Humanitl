/// One rule as one line of the chain.
///
/// The line is read as a sentence -- action, then what it matches, then how
/// long it holds -- and the number in front of it is the place it is
/// evaluated in, inside its own group (CONVENTIONS 4.5). Nothing here decides
/// anything: the row shows the values the daemon answered with and calls back
/// when somebody wants something changed (`docs/UX.md` 1.1, Rules).
///
/// The row itself is [HRow]: fill, rail, focus ring, action slot and the
/// semantics of a row all live in the design system, so the queue, the
/// history and this list cannot drift apart.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/flow_handoff.dart';
import '../../../core/text/rule_sentence.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/rules.dart';

/// Below this pane width the origin of a rule leaves the row. It is the
/// least load-bearing part of the line: it says where the rule came from,
/// which the semantics and the editor still carry.
///
/// One exception, and it is not about provenance: for a rule somebody
/// switched off the same slot carries the one word that says the rule decides
/// nothing, and that word stays at every width -- shortened, so the sentence
/// keeps its room (`docs/UX.md` 3.3 rule 2, 3.4).
const double ruleRowOriginBelow = 420;

/// Below this width the note leaves the row. It explains a rule; the sentence
/// *is* the rule, so the note goes second.
const double ruleRowNoteBelow = 520;

/// Below this width the lifetime leaves the row and stays in the semantics
/// value and in the tooltip only. It is the last thing to go before the
/// sentence itself, which never goes.
const double ruleRowExpiryBelow = 300;

/// Width of the column the evaluation position stands in: four digits of
/// `mono11`, so that rule 1 and rule 1000 keep the sentence on one axis.
const double ruleRowPositionWidth = 24;

/// Move the focused rule one place up or down.
///
/// Every pointer gesture has a key (`docs/UX.md` 5.1), and dragging is the
/// one gesture this screen adds.
class MoveRuleIntent extends Intent {
  /// Moves by [delta] places; negative is towards the front of the chain.
  const MoveRuleIntent(this.delta);

  /// How far and in which direction.
  final int delta;
}

/// The shortcuts of the rule list. `Alt` because the arrow keys alone move
/// the focus, and moving a rule is not moving the focus.
Map<ShortcutActivator, Intent> ruleListShortcuts() =>
    <ShortcutActivator, Intent>{
      const SingleActivator(LogicalKeyboardKey.arrowUp, alt: true):
          const MoveRuleIntent(-1),
      const SingleActivator(LogicalKeyboardKey.arrowDown, alt: true):
          const MoveRuleIntent(1),
    };

/// One row of the rule list.
class RuleRow extends ConsumerStatefulWidget {
  /// Creates the row for [rule].
  const RuleRow({
    required this.rule,
    required this.index,
    required this.total,
    required this.selected,
    required this.onOpen,
    this.onDelete,
    this.onToggleDisabled,
    this.togglingDisabled = false,
    this.onMove,
    this.onMoveRefused,
    this.dragHandle,
    super.key,
  }) : assert(
         onDelete == null || onToggleDisabled == null,
         'a row offers the bin or the switch, never both: a bundled rule '
         'cannot be deleted and a rule of the person cannot be switched off, '
         'and the daemon refuses whichever of the two is offered wrongly '
         'with RULES_010',
       );

  /// The rule this row shows.
  final Rule rule;

  /// Its place in the list of this section, zero-based. Used only when the
  /// daemon named no position; the position of the rule wins.
  final int index;

  /// How many rules the group of this rule holds.
  final int total;

  /// True while this rule is the one in the editor.
  final bool selected;

  /// Opens the rule in the editor.
  final VoidCallback onOpen;

  /// Deletes the rule. Null for a bundled rule, which nobody can delete.
  final VoidCallback? onDelete;

  /// Switches the bundled rule off, or back on. Null for every rule that is
  /// not bundled: those are deleted rather than switched off, and the daemon
  /// refuses anything else with `RULES_010` (`rules_store.rs`,
  /// `set_bundled_disabled`). The row offers whichever of the two it was
  /// given, never both.
  ///
  /// Die Zeile ruft und rechnet nicht: der Aufruf, sein laufender Zustand und
  /// der Befund einer Ablehnung gehören der Liste, die ihn überlebt -- der
  /// Aktionsslot wird nur bei Hover und Fokus gebaut.
  final VoidCallback? onToggleDisabled;

  /// True while the call behind [onToggleDisabled] is still out. The switch is
  /// visibly disabled then; nothing anticipates the answer of the daemon
  /// (CONVENTIONS 4.13).
  final bool togglingDisabled;

  /// Moves the rule by so many places. Null while the list cannot be
  /// reordered -- a filtered list, or the bundled block.
  final void Function(int delta)? onMove;

  /// Wird statt [onMove] gerufen, wenn die Taste gebunden ist, die Zeile aber
  /// gerade nicht bewegt werden kann. Eine gebundene Taste, die schweigt, ist
  /// keine Ablehnung, sondern ein Programm, das eingefroren wirkt; die
  /// Ablehnung ist leise und nennt ihren Grund (`docs/UX.md` 5.3).
  final VoidCallback? onMoveRefused;

  /// The drag handle, wrapped by the list so that the gesture belongs to the
  /// list that owns the order. Null when the row cannot be dragged.
  final Widget? dragHandle;

  /// The place the row shows: what the daemon counted, one-based inside the
  /// group. A rule the daemon gave no position -- a draft -- falls back to
  /// where it stands.
  int get position => rule.position > 0 ? rule.position : index + 1;

  @override
  ConsumerState<RuleRow> createState() => _RuleRowState();
}

class _RuleRowState extends ConsumerState<RuleRow> {
  /// Owned rather than left to [HRow], so that the row can be focused from
  /// outside -- which is what `Alt` plus an arrow key needs a handle on, and
  /// what a test needs to prove that the keyboard reaches everything the
  /// pointer reaches (`docs/UX.md` 5.1).
  final FocusNode _focus = FocusNode(debugLabel: 'rule row');

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Rule rule = widget.rule;
    // Only a rule with an end time watches a clock, and it is the only thing
    // on this screen that watches one at all (`docs/UX.md` 7).
    final DateTime now = rule.expires is RuleExpiryAt
        ? ref.watch(ruleClockProvider).value ?? DateTime.now()
        : DateTime.now();
    final bool expired = ruleExpiredAt(rule, now);
    // Zwei Gründe, aus denen eine Zeile nichts mehr entscheidet, und dieselbe
    // Dämpfung für beide: abgelaufen und ausgeschaltet. Welcher der beiden es
    // ist, sagt das Wort daneben, nie die Farbe allein (`docs/UX.md` 3.3,
    // Regel 2).
    final bool inert = expired || rule.disabled;
    final HFlowState state = ruleRowState(rule, expired: expired);

    return Actions(
      actions: <Type, Action<Intent>>{
        MoveRuleIntent: CallbackAction<MoveRuleIntent>(
          onInvoke: (MoveRuleIntent intent) {
            final void Function(int delta)? move = widget.onMove;
            if (move == null) {
              widget.onMoveRefused?.call();
              return null;
            }
            move(intent.delta);
            return null;
          },
        ),
      },
      child: HRow(
        state: state,
        minHeight: tokens.sizes.row,
        focusNode: _focus,
        selected: widget.selected,
        onTap: widget.onOpen,
        semanticsLabel: l10n.rulesRowSemantics(
          widget.position,
          widget.total,
          ruleActionWord(rule.action, l10n),
          ruleMatchSummary(rule, l10n),
        ),
        semanticsValue: ruleExpiryExact(rule.expires, l10n),
        stateGlyph: ruleActionGlyph(
          rule.action,
          tokens.stateTextColor(state),
          expired: expired,
          disabled: rule.disabled,
        ),
        leading: _Place(
          position: widget.position,
          dragHandle: widget.dragHandle,
        ),
        title: _Line(rule: rule, inert: inert, now: now),
        actionSlot: _actionSlot(tokens, l10n),
      ),
    );
  }

  /// Was am rechten Rand steht: bei einer eigenen Regel der Papierkorb, bei
  /// einer mitgelieferten der Schalter -- nie beides und nie das eine an der
  /// Stelle des anderen.
  ///
  /// Welche der beiden Handlungen es gibt, sagt der Aufrufer, indem er genau
  /// einen der beiden Rückrufe reicht; die Zeile prüft das nicht ein zweites
  /// Mal. Dieselbe Aufteilung hat der Papierkorb seit HUM-033, und zwei
  /// Wahrheiten über dieselbe Frage wären eine zu viel.
  Widget? _actionSlot(HTokens tokens, AppLocalizations l10n) {
    final VoidCallback? onToggle = widget.onToggleDisabled;
    if (onToggle != null) {
      return _DisableSwitch(
        rule: widget.rule,
        index: widget.index,
        onPressed: widget.togglingDisabled ? null : onToggle,
      );
    }
    final VoidCallback? onDelete = widget.onDelete;
    return onDelete == null
        ? null
        : HIconButton(
            key: ValueKey<String>('rule-delete-${widget.index}'),
            glyph: HGlyph.trash,
            size: 14,
            color: tokens.stateTextColor(HFlowState.blocked),
            semanticsLabel: l10n.rulesDelete,
            onPressed: onDelete,
          );
  }
}

/// Der Schalter einer mitgelieferten Regel: aus, und wieder an.
///
/// Er steht an der Stelle, an der eine eigene Regel den Papierkorb hat, weil
/// dieselbe Handlung immer an derselben Stelle liegt (CONVENTIONS 4.13) --
/// und weil er für die mitgelieferte Regel dasselbe ist wie der Papierkorb
/// für die eigene: der eine Weg, sie aufzuheben. Löschen kann man sie nicht;
/// abschalten schon, und ein Klick holt sie zurück.
///
/// Er zeichnet nur und ruft zurück. Ein null-[onPressed] heißt „der Aufruf
/// ist noch unterwegs": der Knopf steht dann sichtbar deaktiviert da, und
/// nichts nimmt die Antwort des Daemons vorweg (CONVENTIONS 4.13).
class _DisableSwitch extends StatelessWidget {
  const _DisableSwitch({
    required this.rule,
    required this.index,
    required this.onPressed,
  });

  final Rule rule;
  final int index;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final bool off = rule.disabled;
    final String host = rule.matcher.host;
    return HoverLabel(
      label: off ? l10n.rulesEnable : l10n.rulesDisable,
      child: HIconButton(
        key: ValueKey<String>('rule-disable-$index'),
        // Der Blitz heißt in diesem Vokabular „eine Regel hat entschieden"
        // (`HGlyph.bolt`), und genau das schaltet dieser Knopf an und aus. Ein
        // Kreuz an dieser Stelle läse sich als der Papierkorb, der bei einer
        // eigenen Regel im selben Slot steht; im Zustands-Glyph links, wo kein
        // Papierkorb steht, sagt dasselbe Kreuz dagegen genau das Richtige.
        // Die Richtung des Schalters sagen Beschriftung und Semantik.
        glyph: HGlyph.bolt,
        size: 14,
        semanticsLabel: off
            ? l10n.rulesEnableSemantics(host)
            : l10n.rulesDisableSemantics(host),
        onPressed: onPressed,
      ),
    );
  }
}

/// The handle and the place: where the rule stands, and what to grab to move
/// it. The slot keeps its width whether the handle is there or not, so a
/// filtered list -- which cannot be reordered -- does not shift.
class _Place extends StatelessWidget {
  const _Place({required this.position, required this.dragHandle});

  final int position;
  final Widget? dragHandle;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        SizedBox(
          width: tokens.sizes.hitMin,
          child: Center(child: dragHandle ?? const SizedBox.shrink()),
        ),
        SizedBox(
          width: ruleRowPositionWidth,
          child: Text(
            '$position',
            textAlign: TextAlign.right,
            // `fg1`, never `fg2`: the number is read, and `fg2` is reserved
            // for controls that are really disabled (`docs/UX.md` 6).
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
        ),
      ],
    );
  }
}

/// Everything the line says, in the room the pane leaves it.
///
/// The order in which parts give way stands here and not in the discretion of
/// the layout: origin first, then the note, then the lifetime. The sentence
/// never goes -- it is the rule (`docs/UX.md` 3.4, same reasoning as the
/// queue row).
class _Line extends StatelessWidget {
  const _Line({required this.rule, required this.inert, required this.now});

  final Rule rule;

  /// True while the rule decides nothing: its time has passed, or it is
  /// switched off. It is what the line is drawn in the quieter colour for.
  final bool inert;

  final DateTime now;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String note = rule.note ?? '';
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double width = constraints.maxWidth;
        // Das Herkunftswort weicht als Erstes, und bei einer ausgeschalteten
        // Regel weicht es nur bis auf das eine Wort, auf das es ankommt: dort
        // sagt es nicht mehr, woher die Regel kommt, sondern dass sie nichts
        // entscheidet. Ganz wegzulassen hieße, das allein der Farbe und dem
        // Kreuz zu überlassen; ganz stehen zu lassen quetschte den Satz, und
        // der Satz ist die Regel (`docs/UX.md` 3.3 Regel 2, 3.4).
        final bool wide = width >= ruleRowOriginBelow;
        return Row(
          children: <Widget>[
            Expanded(
              child: _Sentence(rule: rule, inert: inert),
            ),
            if (width >= ruleRowNoteBelow && note.isNotEmpty) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              Flexible(
                // Die Notiz wird gekürzt, und die Einschränkung einer Notiz
                // steht meistens im zweiten Halbsatz: gekürzt bliebe die
                // Behauptung stehen und der Vorbehalt fiele weg. Der ganze
                // Satz steht deshalb am Zeiger, wie am Griff daneben
                // (CONVENTIONS 4.13).
                child: HoverLabel(
                  label: note,
                  child: Text(
                    note,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ),
              ),
            ],
            if (width >= ruleRowExpiryBelow) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              Text(
                ruleExpiryLabel(rule.expires, l10n, now: now),
                maxLines: 1,
                style: tokens.typography.ui12.tinted(
                  inert
                      ? tokens.stateTextColor(HFlowState.timedOut)
                      : tokens.colors.fg1,
                ),
              ),
            ],
            if (wide || rule.disabled) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              _Origin(rule: rule, short: !wide),
            ],
          ],
        );
      },
    );
  }
}

/// The rule as a sentence: the action in the largest type of the screen, the
/// match in monospace beside it, on one baseline.
///
/// Two families in one line on purpose. The action is a word somebody reads;
/// the match is a pattern somebody compares with another pattern, and
/// comparing needs a fixed advance (CONVENTIONS 4.13). A rule that decides
/// nothing -- because its time has passed, or because somebody switched it
/// off -- says so by fading to the colour of something that ran out: a
/// full-strength line would claim it still decides.
class _Sentence extends StatelessWidget {
  const _Sentence({required this.rule, required this.inert});

  final Rule rule;
  final bool inert;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Color word = inert
        ? tokens.stateTextColor(HFlowState.timedOut)
        : tokens.colors.fg0;
    final Color match = inert
        ? tokens.stateTextColor(HFlowState.timedOut)
        : tokens.colors.fg1;
    return Text.rich(
      TextSpan(
        children: <InlineSpan>[
          TextSpan(
            text: ruleActionWord(rule.action, l10n),
            style: tokens.typography.ui14.medium.tinted(word),
          ),
          TextSpan(
            text: ' · ${ruleMatchSummary(rule, l10n)}',
            style: tokens.typography.mono13.tinted(match),
          ),
        ],
      ),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
    );
  }
}

/// Woher die Regel kommt: aus dem Produkt oder aus einer Entscheidung.
///
/// Eine mitgelieferte Regel sagt das mit einem Wort und einem Schloss, weil
/// „warum kann ich die nicht löschen" die erste Frage ist, die sie aufwirft.
/// Ist sie ausgeschaltet, sagt dasselbe Wort beides -- woher sie kommt und
/// dass sie nichts entscheidet -- und in einer schmalen Pane bleibt davon die
/// Hälfte stehen, die den Zustand trägt.
/// Eine Regel aus einer Anfrage trägt deren kurze Id, und zwar als Control:
/// der Klick bittet die Shell, die Anfrage zu zeigen, so wie es die History
/// für ihre gehaltenen Zeilen tut. Der Weg dorthin ist [flowHandoffProvider]
/// in `core`, damit kein Feature in ein anderes greift (ARCHITECTURE 5); die
/// Tastenentsprechung bringt der Knopf mit (`docs/UX.md` 5.1).
class _Origin extends ConsumerWidget {
  const _Origin({required this.rule, this.short = false});

  final Rule rule;

  /// True while the pane is too narrow for the full word. Only a switched-off
  /// rule is drawn at all then, and only with the part that says it is off.
  final bool short;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    if (rule.bundled) {
      // Ein eigener Schlüssel und keine Verkettung: `de` und `en` setzen die
      // Kommata verschieden.
      final Color color = rule.disabled
          ? tokens.stateTextColor(HFlowState.timedOut)
          : tokens.colors.fg1;
      final String word = rule.disabled
          ? (short ? l10n.rulesOriginOff : l10n.rulesOriginBundledOff)
          : l10n.rulesOriginBundled;
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (!short) ...<Widget>[
            HGlyphIcon(HGlyph.lock, size: 12, color: color),
            SizedBox(width: tokens.spacing.x1),
          ],
          Text(word, style: tokens.typography.ui11.tinted(color)),
        ],
      );
    }
    final FlowId? from = rule.createdFrom;
    if (from == null) {
      return const SizedBox.shrink();
    }
    final String tail = from.value.split('-').last;
    return HoverLabel(
      label: l10n.rulesOriginOpen,
      child: HButton(
        key: ValueKey<String>('rule-origin-${from.value}'),
        variant: HButtonVariant.ghost,
        semanticsLabel: l10n.rulesOriginFlowSemantics(from.value),
        onPressed: () => ref.read(flowHandoffProvider.notifier).request(from),
        child: Text(
          l10n.rulesOriginFlow(tail),
          style: tokens.typography.mono11.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}

/// The visual state of a rule row.
///
/// Rule actions borrow the state colours they mean: allow is what an allowed
/// request wears, block what a blocked one wears, ask what a held one wears,
/// redact the passthrough hue. A rule whose end time has passed wears
/// [HFlowState.timedOut], because that is exactly what happened to it and
/// because nothing that decides nothing may look saturated (CONVENTIONS
/// 4.13).
///
/// A bundled rule that somebody switched off wears the same damping for the
/// same reason. The two are not the same thing -- a bundled rule never runs
/// out, and a rule of the person is never switched off -- so the row says
/// which of them it is in its origin word, not in its colour.
HFlowState ruleRowState(Rule rule, {required bool expired}) =>
    expired || rule.disabled
    ? HFlowState.timedOut
    : ruleActionState(rule.action);

/// The state colour an action borrows.
HFlowState ruleActionState(RuleAction action) => switch (action) {
  RuleAction.allow => HFlowState.allowed,
  RuleAction.block => HFlowState.blocked,
  RuleAction.ask => HFlowState.held,
  RuleAction.redact => HFlowState.passthroughLlm,
};

/// The colour the word and the glyph of [action] are drawn in: the text
/// variant, which reaches 4,5:1 on every surface (`docs/UX.md` 6).
Color ruleActionTextColor(RuleAction action, HTokens tokens) =>
    tokens.stateTextColor(ruleActionState(action));

/// The glyph beside the action word. Colour is never the only channel
/// (`docs/UX.md` 3.3, rule 2); a rule that has run out carries the glyph of a
/// hold that ran out, and a rule somebody switched off carries a plain cross.
///
/// The cross is the shape of "does not apply", and it is what tells a
/// switched-off rule from an effective one at every pane width, before any
/// word does. It is deliberately not the clock, and it never collides with
/// the shape of an action: that is checked over every action rather than
/// assumed (`rules_a11y_test.dart`).
///
/// [disabled] wins over [expired]. Today a bundled rule never runs out and a
/// rule of the person is never switched off, so the two cannot meet; if a
/// later contract lets them, being switched off is the decision of a person
/// and running out is the absence of one, and the row names the decision.
Widget ruleActionGlyph(
  RuleAction action,
  Color color, {
  bool expired = false,
  bool disabled = false,
}) {
  if (disabled) {
    return HGlyphIcon(HGlyph.close, size: 14, color: color);
  }
  if (expired) {
    return HGlyphIcon(HGlyph.clockX, size: 14, color: color);
  }
  return switch (action) {
    RuleAction.allow => HGlyphIcon(HGlyph.arrowUpRight, size: 14, color: color),
    RuleAction.block => HGlyphIcon(HGlyph.shieldX, size: 14, color: color),
    RuleAction.ask => HGlyphIcon(HGlyph.hourglass, size: 14, color: color),
    RuleAction.redact => HGlyphIcon(HGlyph.redactBar, size: 14, color: color),
  };
}
