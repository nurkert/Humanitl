// Die harte Sperre in der Oberfläche (HUM-159).
//
// Eine gehaltene Anfrage mit einer bestätigten IBAN unter
// `hold.hard_block_checksum_secrets`: Der Daemon sagt am Flow an, dass sie
// nur bearbeitet hinausgeht (`HOLD_004`), in der Zeile
// (`FlowSummary.send_refusal`) und im Strom. Die Freigabe heißt dann „Cannot
// send", ist abgeschaltet und trägt den Grund des Daemons als Tooltip; die
// Pause zeigt „Send anyway" nicht, und keine Taste schickt die Anfrage. Über
// eine Gruppe bleiben gesperrte Anfragen stehen, und das Modal nennt sie. Die
// Oberfläche liest dafür keine Konfiguration (ADR-018).
//
// Alle Tests laufen als Linux-Desktop: `flutter test` spielt sonst Android.

import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/findings_pause.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Nur Linux: die einzige Plattform, für die es die App gibt.
final TargetPlatformVariant linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Der Rumpf der Anfrage; die IBAN steht an den Stellen 4 bis 31.
const String ibanBody = 'pay GB82 WEST 1234 5698 7654 32';

/// Der Satz, mit dem der Daemon die Sperre begründet: Art und Ort, nie der
/// Wert (`check_allow` in `daemon/crates/proxy/src/findings.rs`).
const String refusalWhy =
    'the request carries a checksum-confirmed iban in the body and '
    'hold.hard_block_checksum_secrets is on, so it cannot leave this machine '
    'unchanged; nothing was sent';

/// Der Befund `HOLD_004`, wie der Daemon ihn am Flow ansagt.
const Diagnostic refusal = Diagnostic(
  code: DiagnosticCodes.sendRefused,
  severity: Severity.blocking,
  why: refusalWhy,
  fix: FixAction.changeSetting(
    key: 'hold.hard_block_checksum_secrets',
    value: 'false',
  ),
);

/// Eine angehaltene Anfrage mit einer bestätigten IBAN im Rumpf.
FlowDetail withIban({int n = 1}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'bank.example.com',
    apex: 'example.com',
    path: '/v1/transfer',
    requestSize: ibanBody.length,
  ).copyWith(findingCount: 1),
  apex: 'example.com',
  bodyPreview: ibanBody,
  contentType: 'text/plain',
  findings: const <Finding>[
    Finding(
      kind: 'iban',
      location: FindingLocation.body,
      spanStart: 4,
      spanEnd: 31,
      tier: FindingTier.checksum,
      displayPrefix: 'GB82 …',
    ),
  ],
);

/// Eine angehaltene Anfrage an denselben Host ohne Fund.
FlowDetail clean({int n = 2}) => detailFor(
  heldFlow(
    n: n,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'bank.example.com',
    apex: 'example.com',
    path: '/v1/balance',
  ),
  apex: 'example.com',
);

/// Die Ansage der Sperre an [id] im Strom, wie `FlowHandler::analyze` sie
/// vor dem `Held` schickt.
ScriptedEvent announce(FlowId id) => ScriptedEvent(
  const Duration(microseconds: 500),
  (FakeSessionState state, DateTime now) =>
      FlowEvent.diagnostic(at: now, diagnostic: refusal, flowId: id),
);

/// Hält [details] an; die mit einer Id in [locked] tragen die Ansage.
FakeDaemonClient lockedClient(
  List<FlowDetail> details, {
  Set<FlowId> locked = const <FlowId>{},
  bool switchOn = false,
}) {
  final List<ScriptedEvent> script = <ScriptedEvent>[];
  for (final FlowDetail detail in details) {
    final List<ScriptedEvent> held = holdScript(<FlowDetail>[detail]);
    script.add(held.first);
    if (locked.contains(detail.summary.id)) {
      script.add(announce(detail.summary.id));
    }
    script.add(held.last);
  }
  final FakeDaemonClient client = fakeDaemon(script)
    ..hardBlockChecksumSecrets = switchOn;
  client.state.bodies[List<String>.filled(32, '07').join()] =
      Uint8List.fromList(utf8.encode(ibanBody));
  return client;
}

/// Wartet, bis die Freigabe scharf ist (`docs/UX.md` 5.4).
Future<void> armed(WidgetTester tester) async {
  for (
    int i = 0;
    i < 30 && !containerOf(tester).read(allowArmedProvider);
    i++
  ) {
    await tester.pump(const Duration(milliseconds: 50));
  }
  expect(containerOf(tester).read(allowArmedProvider), isTrue);
}

/// Lässt Aufbau und den Weg zum Daemon zu Ende laufen.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

final Finder pause = find.byKey(const Key('intercept-findings-pause'));
final Finder sendAnyway = find.byKey(
  const Key('intercept-findings-pause-send'),
);
final Finder refusedTooltip = find.byKey(const Key('intercept-send-refused'));

ReleaseValve valve(WidgetTester tester) =>
    tester.widget<ReleaseValve>(find.byKey(const Key('intercept-allow')));

/// Die Freigabe sagt, dass es nicht geht, und warum.
void expectRefusedValve(WidgetTester tester) {
  expect(find.text('Cannot send'), findsOneWidget);
  expect(find.text('Send with 1 finding'), findsNothing);
  expect(valve(tester).enabled, isFalse);
  expect(tester.widget<HoverLabel>(refusedTooltip).label, refusalWhy);
}

/// Öffnet die Pause über [id] und prüft, dass „Send anyway" fehlt und `S`
/// nichts schickt, Blockieren aber bleibt.
Future<void> expectPauseWithoutSend(
  WidgetTester tester,
  FakeDaemonClient client,
  FlowId id,
) async {
  containerOf(tester).read(openFindingsPauseProvider.notifier).open(id);
  await settle(tester);
  expect(pause, findsOneWidget);
  expect(sendAnyway, findsNothing);
  expect(
    find.byKey(const Key('intercept-findings-pause-block')),
    findsOneWidget,
  );
  await tester.sendKeyEvent(LogicalKeyboardKey.keyS);
  await settle(tester);
  expect(client.decisions, isEmpty, reason: 'S sends nothing either');
}

void main() {
  testWidgets('hard_block_hides_send_anyway', (WidgetTester tester) async {
    final FlowId id = withIban().summary.id;
    final FakeDaemonClient client = lockedClient(
      <FlowDetail>[withIban()],
      locked: <FlowId>{id},
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);
    expectRefusedValve(tester);

    // `Enter` und `A` senden nichts und öffnen auch keine Pause.
    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.enter,
      LogicalKeyboardKey.keyA,
    ]) {
      await tester.sendKeyEvent(key);
      await settle(tester);
    }
    expect(client.decisions, isEmpty, reason: 'nothing left');
    expect(pause, findsNothing);
    await expectPauseWithoutSend(tester, client, id);
  }, variant: linux);

  testWidgets('a client that starts after the announcement sees the lock', (
    WidgetTester tester,
  ) async {
    // Kein Ereignis im Strom: Die Anfrage war schon gehalten und angesagt,
    // bevor die Oberfläche startete. Sie steht nur in der Liste des Daemons,
    // und die Zeile trägt die Sperre (`FlowSummary.send_refusal`).
    final FlowDetail detail = withIban();
    final FlowId id = detail.summary.id;
    final FakeDaemonClient client = fakeDaemon(const <ScriptedEvent>[]);
    client.state.flows[id] = detail.summary.copyWith(sendRefusal: refusal);
    client.state.details[id] = detail;
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);

    expectRefusedValve(tester);
    await expectPauseWithoutSend(tester, client, id);
  }, variant: linux);

  testWidgets('an allow the daemon refuses locks the valve', (
    WidgetTester tester,
  ) async {
    // Die letzte Linie der Oberfläche: Wo weder Zeile noch Strom die Sperre
    // trugen, sagt sie die Antwort auf das `Allow`.
    final FakeDaemonClient client = lockedClient(<FlowDetail>[
      withIban(),
    ], switchOn: true);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await armed(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    await tester.tap(sendAnyway);
    await settle(tester);

    expect(client.decisions, isEmpty, reason: 'the daemon refused it');
    expect(pause, findsOneWidget);
    expect(sendAnyway, findsNothing);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await settle(tester);
    expect(find.text('Cannot send'), findsOneWidget);
    expect(valve(tester).enabled, isFalse);
    expect(
      tester.widget<HoverLabel>(refusedTooltip).label,
      contains('checksum-confirmed iban in the body'),
    );
  }, variant: linux);

  testWidgets('a group sends what is not locked and names the rest', (
    WidgetTester tester,
  ) async {
    final FlowId lockedId = withIban().summary.id;
    final FlowId cleanId = clean().summary.id;
    final FakeDaemonClient client = lockedClient(
      <FlowDetail>[withIban(), clean()],
      locked: <FlowId>{lockedId},
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await armed(tester);

    // `Enter` über eine Gruppe gilt als Bestätigung der Funde; die gesperrte
    // Anfrage bleibt stehen, und das Modal sagt es, bevor etwas hinausgeht.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(
      find.byKey(const Key('intercept-batch-locked')),
      findsOneWidget,
      reason: 'the modal names the locked request',
    );
    expect(client.decisions, isEmpty, reason: 'the modal asks first');

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await settle(tester);
    expect(
      <FlowId>[
        for (final RecordedDecision each in client.decisions) each.flowId,
      ],
      <FlowId>[cleanId],
      reason: 'the clean request left, the locked one was never sent',
    );
    expect(
      containerOf(tester).read(interceptDecisionProvider).isSending,
      isFalse,
    );
  }, variant: linux);

  testWidgets('a group of locked requests cannot be sent', (
    WidgetTester tester,
  ) async {
    final FlowDetail first = withIban();
    final FlowDetail second = withIban(n: 2);
    final FakeDaemonClient client = lockedClient(
      <FlowDetail>[first, second],
      locked: <FlowId>{first.summary.id, second.summary.id},
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await armed(tester);

    expect(find.text('Cannot send'), findsOneWidget);
    expect(valve(tester).enabled, isFalse);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-batch-confirm')), findsNothing);
  }, variant: linux);

  testWidgets('the refusal speaks German', (WidgetTester tester) async {
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('de')];
    addTearDown(tester.platformDispatcher.clearLocalesTestValue);
    final FlowId id = withIban().summary.id;
    await pumpIntercept(
      tester,
      client: lockedClient(<FlowDetail>[withIban()], locked: <FlowId>{id}),
    );
    await playScript(tester);

    expect(find.text('Senden nicht möglich'), findsOneWidget);
  }, variant: linux);

  testWidgets('a lock that arrives while the modal is open still holds', (
    WidgetTester tester,
  ) async {
    // Beim Öffnen des Modals ist nur die erste Anfrage gesperrt; die zweite
    // wird gesperrt, während das Modal steht. Bestätigt wird der Stand von
    // jetzt, nicht der beim Öffnen: Nur die dritte geht hinaus.
    final FlowId first = withIban().summary.id;
    final FlowId late = clean().summary.id;
    final FlowId third = clean(n: 3).summary.id;
    final FakeDaemonClient client = lockedClient(
      <FlowDetail>[withIban(), clean(), clean(n: 3)],
      locked: <FlowId>{first},
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await pressControl(tester, LogicalKeyboardKey.keyA);
    await armed(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(find.byKey(const Key('intercept-batch-confirm')), findsOneWidget);

    containerOf(tester).read(flowsProvider.notifier).refuseSend(late, refusal);
    await settle(tester);
    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await settle(tester);

    expect(
      <FlowId>[
        for (final RecordedDecision each in client.decisions) each.flowId,
      ],
      <FlowId>[third],
      reason: 'the request locked while the modal stood was not sent',
    );
  }, variant: linux);

  testWidgets('allow all counts locked requests apart from findings', (
    WidgetTester tester,
  ) async {
    final FlowId id = withIban().summary.id;
    await pumpIntercept(
      tester,
      client: lockedClient(
        <FlowDetail>[withIban(), clean()],
        locked: <FlowId>{id},
      ),
    );
    await playScript(tester);

    containerOf(tester).read(interceptDecisionProvider.notifier).askAllowAll();
    final BatchRequest? request = containerOf(tester)
        .read(batchConfirmProvider);
    expect(request?.locked, 1, reason: 'the locked request is named as such');
    expect(request?.withheld, 0, reason: 'no request is held for a finding');
    expect(request?.flows.map((Flow flow) => flow.id), <FlowId>[
      clean().summary.id,
    ]);
  }, variant: linux);

  testWidgets('a forever rule over a group with a lock asks as forever', (
    WidgetTester tester,
  ) async {
    final FlowId id = withIban().summary.id;
    await pumpIntercept(
      tester,
      client: lockedClient(
        <FlowDetail>[withIban(), clean()],
        locked: <FlowId>{id},
      ),
    );
    await playScript(tester);
    await armed(tester);
    containerOf(tester).read(rememberDraftProvider.notifier)
      ..open()
      ..setDuration(RememberDuration.forever);

    final List<Flow> group = containerOf(tester)
        .read(flowsProvider)
        .values
        .toList();
    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .allowMany(group, remember: true, confirmed: true);
    final BatchRequest? request = containerOf(tester)
        .read(batchConfirmProvider);
    expect(request?.reason, ConfirmReason.forever);
    expect(request?.locked, 1);
  }, variant: linux);
}
