/// The middle pane: everything about the selected request.
///
/// Read-only in version 1. The header line carries the method, the whole URL
/// and the countdown; below it sit the three collapsible sections. The rail on
/// the left is the same four pixels the queue row uses, and it sweeps in the
/// colour of the decision while `Decide` is on its way.
library;

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/flow_visual_state.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../body/body_view.dart';
import '../format.dart';
import '../providers/decision.dart';
import '../providers/flows.dart';
import '../providers/now.dart';
import 'countdown_ring.dart';
import 'section_headers.dart';
import 'section_query.dart';
import 'selectable_mono_text.dart';

/// The request card.
class RequestCard extends ConsumerWidget {
  /// Creates the card for [flow].
  const RequestCard({required this.flow, super.key});

  /// The selected flow.
  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AsyncValue<FlowDetail> detail = ref.watch(
      flowDetailProvider(flow.id),
    );
    final DecisionProgress progress = ref.watch(interceptDecisionProvider);
    final bool sweeping =
        progress is DecisionSending && progress.flowId == flow.id;
    final HFlowState sweepState = progress is DecisionSending
        ? _stateOf(progress.kind)
        : flow.visualState;
    final HttpRequest? request = detail.value?.request;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _CardRail(state: sweepState, sweeping: sweeping),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Padding(
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: _CardHeader(flow: flow),
              ),
              _CardBanner(flow: flow),
              const HHairline(),
              Expanded(
                child: SingleChildScrollView(
                  padding: EdgeInsets.symmetric(
                    horizontal: tokens.spacing.x3,
                    vertical: tokens.spacing.x2,
                  ),
                  // Every section starts open: the detail arrives one frame
                  // after the card, and a section that decided whether to
                  // open while the answer was still missing would stay shut.
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      SectionQuery(
                        pathAndQuery: request?.pathAndQuery ?? flow.path,
                      ),
                      SectionHeaders(
                        headers: request?.headers ?? const <Header>[],
                      ),
                      BodyView(flowId: flow.id, body: request?.body),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  static HFlowState _stateOf(DecisionKind kind) => switch (kind) {
    DecisionKind.allow => HFlowState.allowed,
    DecisionKind.allowEdited => HFlowState.allowedEdited,
    DecisionKind.block => HFlowState.blocked,
    DecisionKind.timedOut => HFlowState.timedOut,
  };
}

/// Method, URL and countdown.
class _CardHeader extends ConsumerWidget {
  const _CardHeader({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final DateTime now = ref.watch(nowProvider);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        HMethodBadge(method: flow.methodLabel),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: SelectableMonoText(
            key: const Key('intercept-card-url'),
            text: flow.url,
            maxLines: 2,
            // The largest type of the screen belongs to what the screen is
            // about: the URL that is being decided (`docs/UX.md` 3.1).
            style: tokens.typography.mono14.tinted(tokens.colors.fg0),
          ),
        ),
        SizedBox(width: tokens.spacing.x3),
        CountdownRing(flow: flow),
        SizedBox(width: tokens.spacing.x2),
        Text(
          formatCountdown(flow.remainingAt(now)),
          style: tokens.typography.mono12.tinted(
            flow.isHeld ? tokens.colors.fg1 : tokens.colors.fg2,
          ),
        ),
      ],
    );
  }
}

/// The line that says what happened to a request that is no longer held.
class _CardBanner extends StatelessWidget {
  const _CardBanner({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
    final DecisionKind? decision = flow.decision;
    if (decision == null) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = flow.visualState;
    final Color color = tokens.stateColor(state);
    final String text = decision == DecisionKind.timedOut
        ? l10n.interceptTimedOutBanner
        : l10n.interceptDecidedBanner(l10n.flowStateLabel(state));
    return Padding(
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x3,
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.tint(color),
          borderRadius: BorderRadius.circular(tokens.radii.control),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x2,
          ),
          child: Text(
            text,
            key: const Key('intercept-card-banner'),
            style: tokens.typography.ui12.medium.tinted(color),
          ),
        ),
      ),
    );
  }
}

/// The four pixel rail of the card; it fills top to bottom while a decision
/// travels to the daemon.
class _CardRail extends StatelessWidget {
  const _CardRail({required this.state, required this.sweeping});

  final HFlowState state;
  final bool sweeping;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color color = tokens.stateColor(state);
    return SizedBox(
      width: HSize.stateRail,
      child: Stack(
        fit: StackFit.expand,
        children: <Widget>[
          ColoredBox(color: tokens.tint(color)),
          TweenAnimationBuilder<double>(
            tween: Tween<double>(begin: 0, end: sweeping ? 1 : 0),
            duration: HMotion.sweep,
            curve: HMotion.enter,
            builder: (BuildContext context, double value, Widget? child) =>
                Align(
                  alignment: Alignment.topCenter,
                  child: FractionallySizedBox(
                    heightFactor: value.clamp(0.0, 1.0),
                    widthFactor: 1,
                    child: child,
                  ),
                ),
            child: ColoredBox(color: color),
          ),
        ],
      ),
    );
  }
}
