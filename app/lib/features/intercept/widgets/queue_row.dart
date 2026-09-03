/// One line of the queue: state rail, method, host, path, countdown, and the
/// two ghost buttons the pointer uncovers.
///
/// The row is a projection of a [Flow]; it holds no decision logic. Clicking
/// a ghost button calls the same notifier the action bar calls.
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/flow_visual_state.dart';
import '../../../core/ui/middle_ellipsis.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../format.dart';
import '../providers/flows.dart';
import 'countdown_ring.dart';

/// Width of one monospace character at 12 px, used to decide how much of a
/// path fits before it is shortened in the middle.
const double monoCharWidth12 = 7.3;

/// A queue row.
class QueueRow extends ConsumerStatefulWidget {
  /// Creates the row for [flow].
  const QueueRow({
    required this.flow,
    required this.selected,
    required this.onSelect,
    super.key,
  });

  /// The flow this row shows.
  final Flow flow;

  /// Whether this is the selected row; a selected row is taller and carries a
  /// second line.
  final bool selected;

  /// Called when the row is clicked.
  final VoidCallback onSelect;

  @override
  ConsumerState<QueueRow> createState() => _QueueRowState();
}

class _QueueRowState extends ConsumerState<QueueRow> {
  bool _hovered = false;

  void _decide(Decision decision) {
    widget.onSelect();
    unawaited(
      ref
          .read(interceptDecisionProvider.notifier)
          .send(widget.flow.id, decision),
    );
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final Flow flow = widget.flow;
    final bool sending = ref.watch(interceptDecisionProvider).isSending;
    return HRow(
      state: flow.visualState,
      selected: widget.selected,
      onTap: widget.onSelect,
      onHover: (bool value) => setState(() => _hovered = value),
      semanticsLabel: l10n.interceptRowSemantics(
        flow.methodLabel,
        flow.host,
        flow.path,
      ),
      leading: HMethodBadge(method: flow.methodLabel),
      title: _Title(flow: flow),
      subtitle: _MetaLine(flow: flow),
      trailing: _hovered && flow.isHeld && !sending
          ? _RowActions(flow: flow, onDecide: _decide)
          : CountdownLabel(flow: flow),
    );
  }
}

/// Host and path of the row. The path is shortened in the middle: its end
/// carries the meaning (BACKLOG.md 5, Anti-Patterns).
class _Title extends StatelessWidget {
  const _Title({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
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
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          flex: 2,
          child: LayoutBuilder(
            builder: (BuildContext context, BoxConstraints constraints) => Text(
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
    );
  }
}

/// The second line of the selected row: size, content type, waiting time.
///
/// Only the selected row builds it, so the detail of every other row stays
/// unfetched; the card that shows next to it needs the same answer and shares
/// the provider.
class _MetaLine extends ConsumerWidget {
  const _MetaLine({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final AsyncValue<FlowDetail> detail = ref.watch(
      flowDetailProvider(flow.id),
    );
    final String contentType = detail.value?.request?.body.contentType ?? '';
    return HeldForLabel(
      flow: flow,
      builder: (BuildContext context, String elapsed) => Text(
        l10n.interceptRowMeta(
          formatBytes(flow.requestSize),
          contentType.isEmpty ? l10n.interceptContentTypeUnknown : contentType,
          elapsed,
        ),
      ),
    );
  }
}

/// The two ghost buttons the pointer uncovers, with the countdown between
/// them: Allow and Block are never next to each other (BACKLOG.md 5).
///
/// Glyphs, not labels: at the minimum width of the queue pane a pair of
/// labelled buttons leaves no room for the host, and a decision one cannot
/// read the target of is worse than one more click.
class _RowActions extends StatelessWidget {
  const _RowActions({required this.flow, required this.onDecide});

  final Flow flow;
  final void Function(Decision decision) onDecide;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HIconButton(
          key: Key('queue-block-${flow.id.value}'),
          glyph: HFlowState.blocked.glyph,
          color: tokens.state.blocked,
          semanticsLabel: l10n.interceptBlockButton,
          onPressed: () => onDecide(const Decision.block()),
        ),
        SizedBox(width: tokens.spacing.x1),
        CountdownLabel(flow: flow),
        SizedBox(width: tokens.spacing.x1),
        HIconButton(
          key: Key('queue-allow-${flow.id.value}'),
          glyph: HFlowState.allowed.glyph,
          color: tokens.state.allowed,
          semanticsLabel: l10n.interceptAllowButton,
          onPressed: () => onDecide(const Decision.allow()),
        ),
      ],
    );
  }
}
