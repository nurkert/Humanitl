// Die Wege, auf denen eine Freigabe hinausgeht, nach Runde 3 des Reviews von
// HUM-058.
//
// - Eine einzelne Anfrage mit WebSocket-Aufbau, deren Karte mit Warnung auf
//   dem Schirm stand, geht auch durch das Modal für eine Regel „für immer"
//   hinaus (`BatchRequest.cardSeen`); in einem Stapel bleibt sie gehalten
//   (`batch_websocket_test.dart`).
// - Ein Detail ohne Kopfzeilen sagt nichts über einen Protokollwechsel; die
//   Anfrage bleibt einzeln und im Stapel gehalten, und die Karte sagt warum.
// - Ein Stapel überspringt eine Anfrage, die die Warteschlange inzwischen
//   verlassen hat, und sendet den Rest; ein Detail, das nicht kommt, hält die
//   Leiste nicht auf.
//
// Gegen welche Umkehrung jeder Test gerichtet ist:
//
// - `cardSeen: flow.id` im Einzelweg weg: `a_seen_websocket_goes_through_the_
//   forever_modal` wird rot, weil `_many` sie zurückhält.
// - `requestIsReadable` in `detailSufficesForAllow` weg: beide Tests zur
//   unlesbaren Anfrage werden rot.
// - Das Überspringen bei `FLOW_NOT_HELD` weg: `a_batch_skips_a_request_that_
//   left_the_queue` wird rot, weil der Stapel abbricht.
// - Die Frist in `_allowable` weg: `a_detail_that_never_comes_holds_only_its_
//   request` wird rot, weil die Leiste auf „sending" stehen bleibt.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';

import 'fixtures.dart';
import 'harness.dart';

Future<void> pressChord(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Eine gehaltene Anfrage an `registry.npmjs.org` mit [headers].
FlowDetail npm(int n, {List<Header>? headers}) {
  final FlowDetail base = held(n, path: '/pkg/$n');
  return headers == null
      ? base
      : base.copyWith(request: base.request!.copyWith(headers: headers));
}

/// [details], jede empfangen und angehalten.
List<ScriptedEvent> scriptOf(List<FlowDetail> details) => <ScriptedEvent>[
  for (int i = 0; i < details.length; i++)
    ...arriveAt(details[i], Duration(milliseconds: 10 * i)),
];

final List<Header> upgrade = <Header>[
  header('host', 'registry.npmjs.org'),
  header('connection', 'Upgrade'),
  header('upgrade', 'websocket'),
];

/// Ein Fake, dessen Entscheidung über [gone] mit `FLOW_NOT_HELD` scheitert,
/// und über jede Anfrage, wenn [allGone] gilt.
class GoneClient extends FakeDaemonClient {
  GoneClient(List<ScriptedEvent> script, this.gone, {this.allGone = false})
    : super(script: script, clock: () => testStart);

  final FlowId gone;
  final bool allGone;

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    if (allGone || id == gone) {
      throw const DaemonException(
        Diagnostic(
          code: DiagnosticCodes.flowNotHeld,
          severity: Severity.warning,
          why: 'the flow timed out meanwhile',
        ),
      );
    }
    return super.decide(id, decision, remember: remember);
  }
}

/// Ein Fake, dessen Detail zu [silent] nie kommt, sobald [mute] gilt.
class SilentClient extends FakeDaemonClient {
  SilentClient(List<ScriptedEvent> script, this.silent)
    : super(script: script, clock: () => testStart);

  final FlowId silent;
  bool mute = false;

  @override
  Future<FlowDetail> getFlow(FlowId id) {
    if (mute && id == silent) {
      return Completer<FlowDetail>().future;
    }
    return super.getFlow(id);
  }
}

/// Öffnet über die Anfragen [ns] das Modal für eine Regel „für immer" auf
/// dem Weg, den die Aktionsleiste nimmt (`allowMany`), und liefert die Frage.
Future<BatchRequest> openForever(WidgetTester tester, List<int> ns) async {
  final container = containerOf(tester);
  container
      .read(rememberDraftProvider.notifier)
      .setDuration(RememberDuration.forever);
  await tester.pump();
  final Map<FlowId, Flow> queue = container.read(flowsProvider);
  await container.read(interceptDecisionProvider.notifier).allowMany(<Flow>[
    for (final int n in ns) queue[testFlowId(n)]!,
  ], confirmed: true);
  await tester.pump();
  final BatchRequest? asked = container.read(batchConfirmProvider);
  expect(asked?.reason, ConfirmReason.forever);
  return asked!;
}

void main() {
  testWidgets('a_seen_websocket_goes_through_the_forever_modal', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1, headers: upgrade)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));
    expect(find.byKey(const Key('intercept-card-websocket')), findsOneWidget);

    containerOf(tester)
        .read(rememberDraftProvider.notifier)
        .setDuration(RememberDuration.forever);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    final BatchRequest? asked = containerOf(tester).read(batchConfirmProvider);
    expect(asked?.cardSeen, testFlowId(1));
    expect(client.decisions, isEmpty, reason: 'the modal asks first');

    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions.map((RecordedDecision d) => d.flowId), <FlowId>[
      testFlowId(1),
    ]);
  });

  testWidgets('an_unreadable_request_is_not_allowed_on_its_card', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1, headers: const <Header>[])]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));

    expect(find.byKey(const Key('intercept-card-unreadable')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(client.decisions, isEmpty);
    expect(
      find.text(
        'The request arrived without its headers · it cannot be sent '
        'unedited from here',
      ),
      findsOneWidget,
    );
  });

  testWidgets('an_unreadable_request_stays_held_in_a_group', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1), npm(2, headers: const <Header>[]), npm(3)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions.map((RecordedDecision d) => d.flowId),
      unorderedEquals(<FlowId>[testFlowId(1), testFlowId(3)]),
    );
    expect(
      containerOf(tester).read(flowsProvider)[testFlowId(2)]?.isHeld,
      isTrue,
    );
  });

  testWidgets('a_batch_skips_a_request_that_left_the_queue', (
    WidgetTester tester,
  ) async {
    final GoneClient client = GoneClient(
      scriptOf(<FlowDetail>[npm(1), npm(2), npm(3)]),
      testFlowId(1),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions.map((RecordedDecision d) => d.flowId),
      unorderedEquals(<FlowId>[testFlowId(2), testFlowId(3)]),
      reason: 'the rest goes after the one that left',
    );
    expect(find.byKey(const Key('intercept-decision-error')), findsNothing);
    expect(
      find.text(
        'A request had left the queue meanwhile and was not sent · the '
        'others went',
      ),
      findsOneWidget,
    );
  });

  testWidgets('a_detail_that_never_comes_holds_only_its_request', (
    WidgetTester tester,
  ) async {
    final SilentClient client = SilentClient(
      scriptOf(<FlowDetail>[npm(1), npm(2), npm(3)]),
      testFlowId(2),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    client.mute = true;

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    expect(containerOf(tester).read(interceptDecisionProvider).isSending, true);

    await tester.pump(batchDetailTimeout + const Duration(milliseconds: 10));
    await tester.pump();
    await tester.pump();

    expect(
      containerOf(tester).read(interceptDecisionProvider).isSending,
      false,
    );
    expect(
      client.decisions.map((RecordedDecision d) => d.flowId),
      unorderedEquals(<FlowId>[testFlowId(1), testFlowId(3)]),
    );
    expect(
      containerOf(tester).read(flowsProvider)[testFlowId(2)]?.isHeld,
      isTrue,
    );
    // Der Grund ist das ausgebliebene Detail, kein WebSocket (Review).
    expect(
      find.text(
        'The detail of a request did not arrive in time · it stays held',
      ),
      findsOneWidget,
    );
  });

  testWidgets('a_batch_whose_requests_all_left_says_nothing_went', (
    WidgetTester tester,
  ) async {
    final GoneClient client = GoneClient(
      scriptOf(<FlowDetail>[npm(1), npm(2)]),
      testFlowId(1),
      allGone: true,
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty);
    expect(
      find.text('The requests had left the queue meanwhile · nothing was sent'),
      findsOneWidget,
    );
  });

  testWidgets('a_batch_with_a_rule_sends_nothing_when_its_source_stays_held', (
    WidgetTester tester,
  ) async {
    // Der Satz der Regel nennt die erste Anfrage des Stapels, einen
    // WebSocket-Aufbau. Hielte der Filter ihn zurück und nähme die Regel die
    // zweite, speicherte der Stapel eine andere Regel als die gezeigte.
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1, headers: upgrade), npm(2)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    final container = containerOf(tester);
    final BatchRequest asked = await openForever(tester, <int>[1, 2]);
    expect(asked.shownRule?.source, testFlowId(1));

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty, reason: 'nothing goes');
    expect(client.rules, isEmpty, reason: 'and no rule is saved');
    expect(container.read(selectedFlowIdProvider), testFlowId(1));
    expect(
      find.text(
        'The rule comes from a request that is decided on its own card · '
        'nothing was sent',
      ),
      findsOneWidget,
    );
  });

  testWidgets('the_saved_rule_is_the_object_the_modal_showed', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1), npm(2)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    final BatchRequest asked = await openForever(tester, <int>[1, 2]);
    final ShownRule shown = asked.shownRule!;

    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(2));
    final RecordedDecision first = client.decisions.first;
    expect(first.flowId, shown.source, reason: 'the rule travels on A');
    expect(first.remember, shown.rule, reason: 'unchanged, not rebuilt');
    expect(client.decisions[1].remember, isNull, reason: 'never on B');
  });

  testWidgets('a_rule_whose_source_left_while_the_modal_stood_saves_nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1), npm(2)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    final container = containerOf(tester);
    // Der Cursor steht auf B; ein Wechsel nähme sonst das Modal weg.
    container.read(selectedFlowIdProvider.notifier).select(testFlowId(2));
    // Die neue Auswahl muss erst scharf werden (`HMotion.rearm`).
    await playScript(tester);
    await openForever(tester, <int>[1, 2]);

    // A verlässt die Warteschlange, während das Modal steht.
    await client.decide(testFlowId(1), const Decision.block());
    await playScript(tester);
    expect(container.read(batchConfirmProvider), isNotNull);
    final int before = client.decisions.length;

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(before), reason: 'B does not go');
    expect(client.rules, isEmpty, reason: 'no rule from B');
    expect(
      find.text(
        'The request the rule came from left the queue · nothing was sent',
      ),
      findsOneWidget,
    );
  });

  testWidgets('a_rule_whose_source_is_not_held_any_more_rides_on_nothing', (
    WidgetTester tester,
  ) async {
    final GoneClient client = GoneClient(
      scriptOf(<FlowDetail>[npm(1), npm(2)]),
      testFlowId(1),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openForever(tester, <int>[1, 2]);

    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions,
      isEmpty,
      reason: 'B does not go without its rule source',
    );
    expect(client.rules, isEmpty);
  });

  testWidgets('a_single_source_that_left_under_its_modal_names_the_reason', (
    WidgetTester tester,
  ) async {
    // Die einzige Anfrage, ihr Modal für eine Regel „für immer" steht, und
    // sie läuft ab: Der Grund ist die verlorene Quelle, nicht ein offener
    // Fund (Review Runde 6).
    final FakeDaemonClient client = fakeDaemon(scriptOf(<FlowDetail>[npm(1)]));
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));
    final container = containerOf(tester);
    container
        .read(rememberDraftProvider.notifier)
        .setDuration(RememberDuration.forever);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(
      container.read(batchConfirmProvider)?.shownRule?.source,
      testFlowId(1),
    );

    await client.decide(testFlowId(1), const Decision.block());
    await tester.pump();
    await tester.pump();
    expect(container.read(batchConfirmProvider), isNotNull);
    final int before = client.decisions.length;

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();

    expect(client.decisions, hasLength(before));
    expect(client.rules, isEmpty);
    expect(
      find.text(
        'The request the rule came from left the queue · nothing was sent',
      ),
      findsOneWidget,
    );
    expect(find.text('Hold to send: a finding is unresolved'), findsNothing);
  });

  testWidgets('a_batch_without_a_rule_still_sends_around_the_websocket', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      scriptOf(<FlowDetail>[npm(1, headers: upgrade), npm(2)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    final container = containerOf(tester);
    final List<Flow> both = <Flow>[
      container.read(flowsProvider)[testFlowId(1)]!,
      container.read(flowsProvider)[testFlowId(2)]!,
    ];
    container
        .read(batchConfirmProvider.notifier)
        .ask(BatchRequest(kind: DecisionKind.allow, flows: both));

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions.map((RecordedDecision d) => d.flowId), <FlowId>[
      testFlowId(2),
    ]);
  });
}
