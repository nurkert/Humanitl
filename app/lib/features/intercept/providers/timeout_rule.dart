/// „Als Regel übernehmen" an einer Anfrage, deren Frist ablief (HUM-058,
/// Zeile 7).
///
/// Ein Timeout ist das Ausbleiben einer Entscheidung, und der Agent darf es
/// erneut versuchen (`docs/UX.md` 4.8). Wer bis dahin im Raster aus Dauer und
/// Ziel schon eine Freigabe vorbereitet hatte, verliert den Entwurf nicht: Er
/// bleibt stehen, solange die Anfrage ausgewählt ist (`RememberDraft`), und
/// dieser Notifier legt aus ihm die Regel an, die den nächsten Versuch
/// durchlässt. Die Anfrage selbst ist längst blockiert; `Decide` nähme sie
/// nicht mehr an (`IPC_003`). Deshalb geht der Weg über `Rules(add)`.
///
/// Mit geschlossenem Raster gilt dessen Vorgabe, die Session: Die kleinste
/// Regel, die über die eine Anfrage hinausreicht (BACKLOG.md 5). Eine Regel
/// „für immer" fragt erst im selben Modal wie die Aktionsleiste
/// (`docs/UX.md` 5.4), und je Anfrage entsteht höchstens eine Regel.
library;

import 'package:flutter/foundation.dart' show immutable;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../rule_sentence.dart';
import 'decision.dart';

/// Wo die Regeln der abgelaufenen Anfragen stehen, je Anfrage.
@immutable
class TimeoutRuleState {
  /// Erzeugt den Zustand.
  const TimeoutRuleState({
    this.saved = const <FlowId, Rule>{},
    this.saving,
    this.failure,
  });

  /// Die angelegten Regeln, je abgelaufener Anfrage. Eine Anfrage, die hier
  /// steht, bekommt keine zweite (Review).
  final Map<FlowId, Rule> saved;

  /// Die Anfrage, deren `Rules(add)` gerade unterwegs ist, oder null.
  final FlowId? saving;

  /// Die zuletzt abgelehnte Anfrage mit dem Befund des Daemons, oder null.
  final (FlowId, Diagnostic)? failure;

  /// Die Regel zu [id], wenn sie angelegt ist.
  Rule? savedFor(FlowId id) => saved[id];

  /// Der Befund zu [id], wenn ihr Anlegen scheiterte.
  Diagnostic? failureFor(FlowId id) {
    final (FlowId, Diagnostic)? failed = failure;
    return failed != null && failed.$1 == id ? failed.$2 : null;
  }
}

/// Die Regel aus dem Entwurf einer abgelaufenen Anfrage.
final NotifierProvider<TimeoutRule, TimeoutRuleState> timeoutRuleProvider =
    NotifierProvider<TimeoutRule, TimeoutRuleState>(TimeoutRule.new);

/// Notifier hinter [timeoutRuleProvider].
class TimeoutRule extends Notifier<TimeoutRuleState> {
  @override
  TimeoutRuleState build() => const TimeoutRuleState();

  /// Der Entwurf, der für [flow] gilt: das offene Raster, sonst die Session.
  RuleDraft draftFor(Flow flow) {
    final RememberState remember = ref.read(rememberDraftProvider);
    return RuleDraft(
      duration: remember.open ? remember.duration : RememberDuration.session,
      target: remember.target,
      flow: flow,
    );
  }

  /// Legt die Regel für die abgelaufene Anfrage [flow] an.
  ///
  /// Tut nichts für eine Anfrage, die nicht abgelaufen ist, nichts, solange
  /// schon eine Regel unterwegs ist, und nichts für eine Anfrage, deren Regel
  /// schon steht. Eine Regel „für immer" legt erst das bestätigte Modal an
  /// ([confirmed], über `InterceptDecision.confirmBatch`).
  Future<void> save(Flow flow, {bool confirmed = false}) async {
    if (flow.decision != DecisionKind.timedOut ||
        state.saving != null ||
        state.saved.containsKey(flow.id)) {
      return;
    }
    final RuleDraft draft = draftFor(flow);
    if (draft.duration == RememberDuration.forever && !confirmed) {
      // Eine Regel, die die Sitzung überdauert, ist die zweite Reichweite,
      // für die es das Modal gibt (`docs/UX.md` 5.4).
      ref
          .read(batchConfirmProvider.notifier)
          .ask(
            BatchRequest(
              kind: DecisionKind.allow,
              flows: <Flow>[flow],
              remember: true,
              reason: ConfirmReason.forever,
              ruleOnly: true,
            ),
          );
      return;
    }
    final Rule? rule = buildRule(
      draft,
      now: DateTime.now(),
      apexOf: ref.read(apexResolverProvider),
    );
    if (rule == null) {
      // Nur das Ziel „Domäne" ohne bekannte Domäne baut keine Regel; das
      // Raster lehnt es schon vorher ab (`RememberDraft.setTarget`).
      return;
    }
    state = TimeoutRuleState(saved: state.saved, saving: flow.id);
    try {
      await ref.read(daemonClientProvider).addRule(rule);
      if (ref.mounted) {
        state = TimeoutRuleState(
          saved: <FlowId, Rule>{...state.saved, flow.id: rule},
        );
      }
    } on DaemonException catch (error) {
      if (ref.mounted) {
        state = TimeoutRuleState(
          saved: state.saved,
          failure: (flow.id, error.diagnostic),
        );
      }
    } on Object catch (error) {
      if (ref.mounted) {
        state = TimeoutRuleState(
          saved: state.saved,
          failure: (
            flow.id,
            ClientDiagnostics.daemonUnreachable(
              socketPath: '?',
              detail: error.toString(),
            ),
          ),
        );
      }
    }
  }
}
