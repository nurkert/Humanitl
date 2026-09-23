// „Queue: allow all…“ lässt keine Anfrage mit offenem Fund hinaus (HUM-207).
//
// Befund M5 des Sicherheitsdurchlaufs vom 2026-09-23: Das Modal der Palette
// nannte Anzahl, Hosts und Pfade, aber keinen Fund, und ein einfacher Knopf
// gab alle frei. Der Agent kann die Queue füllen, bis der Mensch zu „allow
// all“ greift. `docs/UX.md` 4.7 lehnt ein Modal als Schutz für einen Fund
// ab; nur das gehaltene Ventil oder die Fundpause bestätigen einen.
// Gezählt wird am Fake-Daemon: Was er nicht als Entscheidung gesehen hat, ist
// nicht hinausgegangen.
//
// Alle Tests laufen als Linux-Desktop: `flutter test` spielt sonst Android.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/widgets/batch_modal.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Nur Linux: die einzige Plattform, für die es die App gibt.
final TargetPlatformVariant linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Drei angehaltene Anfragen; die zweite trägt zwei offene Funde.
List<ScriptedEvent> queueWithFinding() => holdScript(<FlowDetail>[
  held(1, path: '/a'),
  held(2, path: '/b', findings: 2),
  held(3, path: '/c'),
]);

/// Die Flows, über die der Fake-Daemon eine Freigabe gesehen hat.
Set<FlowId> allowed(FakeDaemonClient client) => <FlowId>{
  for (final RecordedDecision d in client.decisions)
    if (d.decision is DecisionAllow) d.flowId,
};

/// Öffnet die Palette und wählt „Queue: allow all…“.
Future<void> allowAllFromPalette(WidgetTester tester) async {
  await pressControl(tester, LogicalKeyboardKey.keyK);
  await tester.pumpAndSettle();
  await tester.tap(find.text('Queue: allow all…'));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('allow all sends the clean requests and names the withheld one', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(queueWithFinding());
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await allowAllFromPalette(tester);

    // Das Modal zählt nur die sauberen und sagt, was angehalten bleibt.
    expect(find.byType(BatchModal), findsOneWidget);
    final BatchRequest request = containerOf(tester)
        .read(batchConfirmProvider)!;
    expect(request.flows.map((Flow f) => f.id), <FlowId>[
      testFlowId(1),
      testFlowId(3),
    ]);
    expect(request.withheld, 1);
    expect(
      find.text(
        '1 request with an unresolved finding stays held. '
        'Send it from its card.',
      ),
      findsOneWidget,
    );

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pumpAndSettle();

    expect(allowed(client), <FlowId>{testFlowId(1), testFlowId(3)});
    expect(
      allowed(client).contains(testFlowId(2)),
      isFalse,
      reason: 'a modal never confirms a finding (docs/UX.md 4.7)',
    );
  }, variant: linux);

  testWidgets('allow all over only findings opens no modal and sends nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[
        held(1, path: '/a', findings: 1),
        held(2, path: '/b', findings: 3),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await allowAllFromPalette(tester);

    expect(find.byType(BatchModal), findsNothing);
    expect(
      containerOf(tester).read(lastRefusalProvider)?.reason,
      RefusalReason.holdToSend,
    );
    expect(client.decisions, isEmpty);
  }, variant: linux);

  testWidgets('a batch without confirmed findings drops the request with one', (
    WidgetTester tester,
  ) async {
    // Die zweite Wand: Wer einen Batch an `askAllowAll` vorbei baut, kommt
    // in `confirmBatch` trotzdem nicht mit einem Fund durch.
    final FakeDaemonClient client = fakeDaemon(queueWithFinding());
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final List<Flow> queue = containerOf(tester).read(heldFlowsProvider);
    expect(queue, hasLength(3));
    containerOf(tester)
        .read(batchConfirmProvider.notifier)
        .ask(BatchRequest(kind: DecisionKind.allow, flows: queue));
    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .confirmBatch();
    await tester.pumpAndSettle();

    expect(allowed(client), <FlowId>{testFlowId(1), testFlowId(3)});
  }, variant: linux);

  testWidgets('a group the valve confirmed still sends its findings', (
    WidgetTester tester,
  ) async {
    // Der bestätigte Weg von `allowMany` (gehaltenes Ventil) bleibt offen:
    // Zwei Hosts lassen das Modal fragen, und das Flag trägt die Bestätigung
    // durch das Modal hindurch.
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[
        held(1, path: '/a'),
        held(2, path: '/b', findings: 2),
        held(3, host: 'api.example.com', apex: 'example.com', path: '/c'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    for (
      int i = 0;
      i < 30 && !containerOf(tester).read(allowArmedProvider);
      i++
    ) {
      await tester.pump(const Duration(milliseconds: 50));
    }
    expect(containerOf(tester).read(allowArmedProvider), isTrue);

    final List<Flow> queue = containerOf(tester).read(heldFlowsProvider);
    await containerOf(tester)
        .read(interceptDecisionProvider.notifier)
        .allowMany(queue, confirmed: true);
    await tester.pumpAndSettle();
    expect(find.byType(BatchModal), findsOneWidget);
    expect(
      containerOf(tester).read(batchConfirmProvider)!.findingsConfirmed,
      isTrue,
    );

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pumpAndSettle();

    expect(allowed(client), <FlowId>{
      testFlowId(1),
      testFlowId(2),
      testFlowId(3),
    });
  }, variant: linux);
}
