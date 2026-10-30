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

import '../../../core/body/body_view.dart';
import '../../../core/domain/domain.dart';
import '../../../core/recorder/recorder_health.dart';
import '../../../core/text/format.dart';
import '../../../core/ui/flow_visual_state.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/websocket_notice.dart';
import '../../../l10n/l10n.dart';
import '../providers/decision.dart';
import '../providers/flows.dart';
import '../providers/now.dart';
import '../providers/timeout_rule.dart';
import '../rule_sentence.dart';
import '../../../core/domain/websocket.dart';
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
    final RecorderOutage? outage = ref.watch(recorderHealthProvider);
    final List<Widget> notices = <Widget>[
      if (flow.decision != null) _CardBanner(flow: flow),
      // Während einer Störung der Aufzeichnung trägt jede Karte, deren
      // Anfrage oder Entscheidung jetzt nicht aufgezeichnet wird, den
      // Hinweis: jede gehaltene und jede, die seit Beginn der Störung kam
      // (HUM-250). Er geht mit `RECORDER_005`.
      if (outage != null &&
          (flow.isHeld || !flow.receivedAt.isBefore(outage.since)))
        const _NotRecordedNotice(),
      if (flow.decision == DecisionKind.timedOut) _TimeoutRule(flow: flow),
      if (request != null && requestOpensWebSocket(request.headers))
        _WebSocketNotice(flow: flow),
      if (flow.isHeld &&
          detail.value != null &&
          !requestIsReadable(detail.value))
        const _UnreadableNotice(),
    ];
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _CardRail(state: sweepState, sweeping: sweeping),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              // Die Kopfzeile des Panes (HUM-286): Ihre Höhe ist das Token,
              // ihre Linie die untere Kante, in derselben Reihe wie die der
              // Queue und des Kontext-Panes.
              HPaneHeader(
                key: const Key('intercept-card-header'),
                child: _CardHeader(flow: flow),
              ),
              // Die Hinweise stehen zwischen Kopf und Abschnitten und bringen
              // ihre Linie darunter nur mit, wenn es sie gibt: zwei Linien
              // übereinander wären eine zu viel ("Layout grid").
              if (notices.isNotEmpty) ...<Widget>[
                Padding(
                  padding: EdgeInsets.only(top: tokens.spacing.x3),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: notices,
                  ),
                ),
                const HHairline(),
              ],
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
                      BodyView(
                        flowId: flow.id,
                        body: request?.body,
                        headers: request?.headers ?? const <Header>[],
                        findings: detail.value?.findings ?? const <Finding>[],
                        // Solange kein Detail da ist, ist der fehlende Rumpf
                        // Warten und keine Aussage (HUM-154). Gefragt wird
                        // nach dem Wert, nicht nach `isLoading`: riverpod
                        // wiederholt einen gescheiterten Provider von selbst,
                        // und die Karte spränge sonst zwischen Skelett und
                        // „kein Rumpf" hin und her. Ein Detail, das dauerhaft
                        // scheitert, wartet hier weiter, statt zu behaupten;
                        // die Meldung dazu gehört an die Stelle des Skeletts
                        // und fehlt noch, so wie den Abschnitten daneben.
                        pending: detail.value == null,
                      ),
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
        ? _timedOut(l10n)
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

extension on _CardBanner {
  /// Der Satz zu einer abgelaufenen Anfrage: wie lange sie wartete und was der
  /// Agent jetzt tun kann (HUM-058, Zeile 7).
  ///
  /// Die Frist steht am Flow, solange die Warteschlange ihn hält; die Spanne
  /// vom Eintreffen bis zu ihr ist die Wartezeit, nach der der Daemon
  /// blockiert hat. Fehlt eine der beiden, sagt der Satz nur, was geschah, und
  /// erfindet keine Zahl.
  String _timedOut(AppLocalizations l10n) {
    final DateTime? deadline = flow.deadline;
    if (deadline == null || !deadline.isAfter(flow.receivedAt)) {
      return l10n.interceptTimedOutBanner;
    }
    return l10n.interceptTimedOutAfter(
      deadline.difference(flow.receivedAt).inSeconds,
    );
  }
}

/// „Als Regel übernehmen" unter dem Band einer abgelaufenen Anfrage (HUM-058,
/// Zeile 7): der Satz der Regel, die der Entwurf ergäbe, und der Knopf, der
/// sie anlegt. Vor „Merken" steht die Regel als Satz, damit sie geprüft
/// werden kann (`docs/UX.md` 4.6).
class _TimeoutRule extends ConsumerWidget {
  const _TimeoutRule({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Der Entwurf wird beobachtet, damit Satz und Regel dem Raster folgen.
    ref.watch(rememberDraftProvider);
    final TimeoutRuleState saving = ref.watch(timeoutRuleProvider);
    final TimeoutRule notifier = ref.read(timeoutRuleProvider.notifier);
    final String sentence = ruleSentence(
      notifier.draftFor(flow),
      l10n,
      apexOf: ref.watch(apexResolverProvider),
    );
    if (sentence.isEmpty) {
      return const SizedBox.shrink();
    }
    final TextStyle note = tokens.typography.ui12.tinted(tokens.colors.fg1);
    return Padding(
      key: const Key('intercept-timeout-rule'),
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x3,
      ),
      child: Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: tokens.spacing.x3,
        runSpacing: tokens.spacing.x1,
        children: <Widget>[
          if (saving.savedFor(flow.id) != null)
            Text(
              l10n.interceptTimedOutRuleSaved,
              key: const Key('intercept-timeout-rule-saved'),
              style: note,
            )
          else
            HButton(
              key: const Key('intercept-timeout-rule-save'),
              size: HButtonSize.sm,
              onPressed: saving.saving != null
                  ? null
                  : () => notifier.save(flow),
              child: Text(l10n.interceptTimedOutSaveRule),
            ),
          Text(
            sentence,
            style: tokens.typography.mono12.tinted(tokens.colors.fg1),
          ),
          if (saving.failureFor(flow.id) case final Diagnostic diagnostic)
            Text(
              l10n.interceptTimedOutRuleFailed(diagnostic.why),
              key: const Key('intercept-timeout-rule-failed'),
              style: tokens.typography.ui12.tinted(
                tokens.stateTextColor(HFlowState.error),
              ),
            ),
        ],
      ),
    );
  }
}

/// Die Zeile über einer gehaltenen Anfrage, deren Detail ohne Kopfzeilen
/// kam (HUM-058, Review): Es sagt nichts darüber, ob die Anfrage eine
/// WebSocket-Verbindung öffnet, also ist die Freigabe aus, und hier steht,
/// warum.
class _UnreadableNotice extends StatelessWidget {
  const _UnreadableNotice();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      key: const Key('intercept-card-unreadable'),
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x3,
      ),
      child: Text(
        context.l10n.interceptCardUnreadable,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
    );
  }
}

/// Die Zeile auf einer Karte, deren Anfrage gerade nicht aufgezeichnet wird
/// (HUM-250): Sie und ihre Entscheidung fehlen später in der History.
///
/// In der Farbe des Fehlers, wie die Statusleiste und die Pause vor der
/// ersten Freigabe; Rot heißt blockiert (`docs/UX.md` 3.3).
class _NotRecordedNotice extends StatelessWidget {
  const _NotRecordedNotice();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      key: const Key('intercept-card-not-recorded'),
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x3,
      ),
      child: Text(
        context.l10n.interceptCardNotRecorded,
        style: tokens.typography.ui12.medium.tinted(
          tokens.stateTextColor(HFlowState.error),
        ),
      ),
    );
  }
}

/// The line under the header of a request that opens a WebSocket (HUM-058,
/// row 11): a chip that names the protocol and the sentence that says what an
/// allow lets through after it.
class _WebSocketNotice extends StatelessWidget {
  const _WebSocketNotice({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      key: const Key('intercept-card-websocket'),
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x3,
      ),
      child: WebSocketNotice(host: flow.authority.shownHost),
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
