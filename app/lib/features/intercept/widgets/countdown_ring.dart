/// The countdown: a ring around the state glyph and the remaining time as
/// `mm:ss`.
///
/// Both listen to `nowProvider`, the one clock of the queue; a timer per row
/// does not scale to two hundred rows (HUM-020 Fallstricke). The ring is
/// `HStateGlyph`, which draws the arc and breathes below twenty percent of
/// the budget without ever turning red.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/flow_visual_state.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../format.dart';
import '../providers/now.dart';

/// The remaining fraction of the hold budget of [flow] at [now], or null when
/// the flow carries no deadline.
double? remainingFraction(Flow flow, DateTime now) {
  final Duration budget = flow.holdBudget;
  if (budget <= Duration.zero) {
    return null;
  }
  final double left =
      flow.remainingAt(now).inMilliseconds / budget.inMilliseconds;
  return left.clamp(0.0, 1.0);
}

/// The state glyph of [flow] inside its countdown ring.
class CountdownRing extends ConsumerWidget {
  /// Creates a ring for [flow].
  const CountdownRing({required this.flow, this.size = 20, super.key});

  /// The flow whose deadline is drawn.
  final Flow flow;

  /// Diameter of the ring.
  final double size;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final DateTime now = ref.watch(nowProvider);
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = flow.visualState;
    final double? progress = flow.isHeld ? remainingFraction(flow, now) : null;
    return HStateGlyph(
      state: state,
      size: size,
      progress: progress,
      semanticsLabel: progress == null
          ? l10n.flowStateLabel(state)
          : l10n.interceptRemaining(formatCountdown(flow.remainingAt(now))),
    );
  }
}

/// The remaining time of [flow] as `mm:ss`.
class CountdownLabel extends ConsumerWidget {
  /// Creates the label for [flow].
  const CountdownLabel({required this.flow, super.key});

  /// The flow whose deadline is counted down.
  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final DateTime now = ref.watch(nowProvider);
    return ExcludeSemantics(
      child: Text(
        formatCountdown(flow.remainingAt(now)),
        style: tokens.typography.mono11.tinted(
          flow.isHeld ? tokens.colors.fg1 : tokens.colors.fg2,
        ),
      ),
    );
  }
}

/// How long [flow] has been waiting, as `mm:ss`.
class HeldForLabel extends ConsumerWidget {
  /// Creates the label for [flow]; [builder] wraps the formatted value in the
  /// sentence the row shows.
  const HeldForLabel({required this.flow, required this.builder, super.key});

  /// The flow that is waiting.
  final Flow flow;

  /// Turns the formatted duration into the finished line.
  final Widget Function(BuildContext context, String elapsed) builder;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final DateTime now = ref.watch(nowProvider);
    return builder(context, formatCountdown(flow.heldFor(now)));
  }
}
