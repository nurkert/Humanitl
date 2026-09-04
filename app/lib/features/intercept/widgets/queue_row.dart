/// One line of the queue: state rail, method, host, path, countdown, and the
/// block affordance the pointer uncovers.
///
/// The row is a projection of a [Flow]; it holds no decision logic. Clicking
/// the block affordance calls the same notifier the action bar calls.
///
/// A decided row keeps its place until its confirmation window is over and
/// swaps its content for the strip that says what happened -- same line, same
/// height, no second box (`docs/UX.md` 3.4 and 8). Out of a row only blocking
/// is possible: allowing cannot be taken back and needs the URL, and the URL
/// stands in the card (`docs/UX.md` 3.4).
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/flow_visual_state.dart';
import '../../../core/ui/hold_to_confirm.dart';
import '../../../core/ui/middle_ellipsis.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../format.dart';
import '../providers/decision.dart';
import 'countdown_ring.dart';

/// Width of one monospace character at 12 px, used to decide how much of a
/// path fits before it is shortened in the middle.
const double monoCharWidth12 = 7.3;

/// How often any queue row has run its `build`.
///
/// Rebuild scope is invisible to the compiler and to a golden, so it
/// regresses silently; a counter is the only way a test can hold it
/// (`docs/UX.md` 7). Tests reset it before the step they measure.
@visibleForTesting
int debugQueueRowBuilds = 0;

/// A queue row.
class QueueRow extends ConsumerStatefulWidget {
  /// Creates the row for [flow].
  const QueueRow({
    required this.flow,
    required this.selected,
    required this.onSelect,
    this.member = false,
    this.onToggleSelect,
    this.onRangeSelect,
    super.key,
  });

  /// The flow this row shows.
  final Flow flow;

  /// Whether the cursor stands on this row. Exactly one row of the pane does,
  /// and it is the only one that carries a fill (`docs/UX.md` 3.5).
  final bool selected;

  /// Called when the row is clicked.
  final VoidCallback onSelect;

  /// Whether this row belongs to a multi-selection. Membership is the rail,
  /// never a second fill (`docs/UX.md` 3.5).
  final bool member;

  /// `Ctrl` and a click: takes the row into the selection or out of it.
  final VoidCallback? onToggleSelect;

  /// `Shift` and a click: everything between the cursor and this row.
  final VoidCallback? onRangeSelect;

  @override
  ConsumerState<QueueRow> createState() => _QueueRowState();
}

class _QueueRowState extends ConsumerState<QueueRow> {
  /// A click, with whatever modifier was down when it happened.
  ///
  /// The modifiers are read here and not passed down from a gesture handler,
  /// because `HRow` reports a tap and nothing else; a pointer path without a
  /// modifier keeps behaving exactly as it did before the multi-selection
  /// existed (HUM-029).
  void _tap() {
    final Set<LogicalKeyboardKey> down =
        HardwareKeyboard.instance.logicalKeysPressed;
    final bool control =
        down.contains(LogicalKeyboardKey.controlLeft) ||
        down.contains(LogicalKeyboardKey.controlRight);
    final bool shift =
        down.contains(LogicalKeyboardKey.shiftLeft) ||
        down.contains(LogicalKeyboardKey.shiftRight);
    final VoidCallback? toggle = widget.onToggleSelect;
    final VoidCallback? range = widget.onRangeSelect;
    if (control && toggle != null) {
      toggle();
      return;
    }
    if (shift && range != null) {
      range();
      return;
    }
    widget.onSelect();
  }

  void _decide(Decision decision) {
    widget.onSelect();
    unawaited(
      ref
          .read(interceptDecisionProvider.notifier)
          .send(widget.flow.id, decision, flow: widget.flow),
    );
  }

  @override
  Widget build(BuildContext context) {
    debugQueueRowBuilds++;
    final AppLocalizations l10n = context.l10n;
    final Flow flow = widget.flow;
    // Keyed to this row: a decision on another flow rebuilds another row, not
    // this one (`docs/UX.md` 7).
    final bool sending = ref.watch(
      interceptDecisionProvider.select(
        (DecisionProgress progress) =>
            progress is DecisionSending && progress.flowId == flow.id,
      ),
    );
    if (flow.isDecided) {
      return _ConfirmationStrip(flow: flow, selected: widget.selected);
    }
    return HRow(
      state: flow.visualState,
      // Only `held` stands in this queue, by construction: fifteen full
      // amber rails repeat one fact and become the loudest thing on the
      // screen (`docs/UX.md` 3.3, rule 1).
      tintedRail: true,
      selected: widget.selected,
      inSelection: widget.member,
      onTap: _tap,
      semanticsLabel: l10n.interceptRowSemantics(
        flow.methodLabel,
        flow.host,
        flow.path,
      ),
      leading: HMethodBadge(method: flow.methodLabel),
      title: _Title(flow: flow),
      // No second line, in any state: it would only repeat what the card two
      // panes away shows, and the row would grow by twenty pixels on every
      // `J`, pushing everything under it down (`docs/UX.md` 3.4 and 8).
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (flow.findingCount > 0) ...<Widget>[
            _FindingsChip(count: flow.findingCount),
            SizedBox(width: HTheme.of(context).spacing.x2),
          ],
          CountdownLabel(flow: flow),
        ],
      ),
      // The action slot is always there and empty at rest; hover and focus
      // uncover it, and it moves nothing (`docs/UX.md` 3.4).
      actionSlot: flow.isHeld && !sending
          ? _BlockAffordance(
              flow: flow,
              onDecide: _decide,
              onTapShort: () => ref
                  .read(lastRefusalProvider.notifier)
                  .refuse(RefusalReason.holdIt),
            )
          : null,
    );
  }
}

/// The findings chip: the one chroma a resting queue line may carry.
///
/// A request that waits because of a finding must not look like a routine
/// request; the chip is one of the three deviations `docs/UX.md` 4.7 allows.
class _FindingsChip extends StatelessWidget {
  const _FindingsChip({required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return HBadge(
      text: '$count',
      color: tokens.state.error,
      semanticsLabel: context.l10n.interceptGroupFindings(count),
    );
  }
}

/// The decided row: the same line, carrying what happened.
///
/// The strip belongs to the row that produced it, not to the card two panes
/// away, where it would block for three seconds exactly the place the next
/// decision is taken (`docs/UX.md` 8). It observes no clock: what it says was
/// true at the moment of the decision and stays true.
class _ConfirmationStrip extends StatelessWidget {
  const _ConfirmationStrip({required this.flow, required this.selected});

  final Flow flow;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = flow.visualState;
    final String text = switch (flow.decision) {
      DecisionKind.allow || DecisionKind.allowEdited => l10n.interceptSentTo(
        flow.host,
        formatBytes(flow.requestSize),
      ),
      DecisionKind.timedOut => l10n.interceptBlockedTimedOut,
      _ => l10n.interceptBlockedRetry,
    };
    return HRow(
      key: Key('queue-strip-${flow.id.value}'),
      state: state,
      selected: selected,
      semanticsLabel: '${l10n.flowStateLabel(state)} $text',
      // The decided row carries its state colour in full: for three seconds
      // it is the only saturated area in the pane (`docs/UX.md` 3.3).
      stateGlyph: HStateGlyph(state: state),
      title: ExcludeSemantics(
        child: Text(
          text,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.mono12.tinted(tokens.colors.fg0),
        ),
      ),
    );
  }
}

/// Below this much room for host and path together, only the host stands.
///
/// What gives way first is decided here and not left to the layout: the path
/// goes, the host stays, because the host is the answer to "where to"
/// (`docs/UX.md` 3.4). At twice the text scale the slot can shrink to a few
/// pixels, and a fixed gap between two texts would overflow it.
const double titlePathFloor = 160;

/// Host and path of the row. The path is shortened in the middle: its end
/// carries the meaning (BACKLOG.md 5, Anti-Patterns).
class _Title extends StatelessWidget {
  const _Title({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (BuildContext context, BoxConstraints outer) =>
        _line(context, outer.maxWidth >= titlePathFloor),
  );

  Widget _line(BuildContext context, bool withPath) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      children: <Widget>[
        Flexible(
          child: Text(
            flow.host,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
        ),
        if (withPath) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            flex: 2,
            child: LayoutBuilder(
              builder: (BuildContext context, BoxConstraints constraints) =>
                  Text(
                    middleEllipsis(
                      flow.path,
                      (constraints.maxWidth / monoCharWidth12).floor(),
                    ),
                    maxLines: 1,
                    softWrap: false,
                    style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  ),
            ),
          ),
        ],
      ],
    );
  }
}

/// The action the pointer and the focus uncover: blocking, and nothing else.
///
/// Out of a row only blocking is possible. Allowing cannot be taken back and
/// needs the URL, and the URL stands in the card, two panes away; an allow
/// affordance in the row would also stand outside the arming that protects
/// every other way to allow (`docs/UX.md` 3.4 and 5.4). The 250 ms hold is
/// the same one the action bar asks for.
///
/// A glyph, not a label: at the minimum width of the queue pane a labelled
/// button leaves no room for the host, and a decision whose target one cannot
/// read is worse than one more click.
class _BlockAffordance extends StatelessWidget {
  const _BlockAffordance({
    required this.flow,
    required this.onDecide,
    required this.onTapShort,
  });

  final Flow flow;
  final void Function(Decision decision) onDecide;
  final VoidCallback onTapShort;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return HoldToConfirm(
      key: Key('queue-block-${flow.id.value}'),
      duration: HMotion.holdToBlock,
      fill: holdFill(tokens.state.blocked),
      token: flow.id,
      onConfirmed: () => onDecide(const Decision.block()),
      onTapShort: onTapShort,
      builder: (BuildContext context, double progress) => Semantics(
        container: true,
        button: true,
        label: l10n.interceptBlockButton,
        child: SizedBox(
          width: HSize.rowActionSlot,
          height: HSize.rowActionSlot,
          child: Center(
            child: HGlyphIcon(
              HFlowState.blocked.glyph,
              color: tokens.state.blocked,
            ),
          ),
        ),
      ),
    );
  }
}
