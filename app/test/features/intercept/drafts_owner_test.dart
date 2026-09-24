// Wem Notiz und Merk-Entwurf gehören (HUM-218).
//
// Beide gehören der ausgewählten Anfrage. Eine Entscheidung verbraucht sie nur,
// wenn sie mit ihnen begonnen hat und die Auswahl bis zu ihrem Ende dieselbe
// geblieben ist; eine Entscheidung aus einer Zeile benutzt sie gar nicht.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/note.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Ein Fake, dessen Entscheidungen warten, bis [gate] aufgeht.
class GatedClient extends FakeDaemonClient {
  /// Hält jede Entscheidung an, bis [gate] erfüllt ist.
  GatedClient(List<ScriptedEvent> script)
    : super(script: script, clock: () => testStart);

  /// Gibt die wartenden Entscheidungen frei; null lässt sie sofort durch.
  Completer<void>? gate = Completer<void>();

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    await gate?.future;
    return super.decide(id, decision, remember: remember);
  }
}

/// Die Anfrage A an GitHub.
final FlowDetail a = held(1, host: 'api.github.com', apex: 'github.com');

/// Die Anfrage B an PyPI.
final FlowDetail b = held(2, host: 'pypi.org', apex: 'pypi.org');

/// A und B, zwei Zeilen unter verschiedenen Domains.
List<ScriptedEvent> twoRows() => <ScriptedEvent>[
  ...arriveAt(a, Duration.zero),
  ...arriveAt(b, const Duration(milliseconds: 10)),
];

/// Blockt A und wählt, während die Entscheidung unterwegs ist, [path] durch;
/// danach steht eine Notiz für die zuletzt gewählte Anfrage.
Future<ProviderContainer> blockAWhileMoving(
  WidgetTester tester,
  GatedClient client,
  List<FlowId> path, {
  Future<void> Function(ProviderContainer container)? before,
}) async {
  await pumpIntercept(tester, client: client);
  await playScript(tester);
  final ProviderContainer container = containerOf(tester);
  if (before != null) {
    await before(container);
    container.read(selectedFlowIdProvider.notifier).select(a.summary.id);
    await tester.pump();
  }
  expect(container.read(selectedFlowIdProvider), a.summary.id);

  unawaited(container.read(interceptDecisionProvider.notifier).block());
  await tester.pump();
  for (final FlowId id in path) {
    container.read(selectedFlowIdProvider.notifier).select(id);
    await tester.pump();
  }
  container.read(blockNoteProvider.notifier)
    ..open()
    ..write('written during the send');
  await tester.pump();

  client.gate!.complete();
  await tester.pump();
  await tester.pump();
  expect(client.decisions.last.flowId, a.summary.id);
  return container;
}

void main() {
  testWidgets(
    'a decision in flight leaves the note of the next selection alone',
    (WidgetTester tester) async {
      final GatedClient client = GatedClient(twoRows());
      final ProviderContainer container = await blockAWhileMoving(
        tester,
        client,
        <FlowId>[b.summary.id],
      );

      expect(container.read(selectedFlowIdProvider), b.summary.id);
      expect(container.read(blockNoteProvider).text, 'written during the send');
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets('away and back during the send counts as a new selection', (
    WidgetTester tester,
  ) async {
    // B ist schon entschieden und ruht noch in der Liste; nach A bleibt
    // keine angehaltene Anfrage, zu der die Auswahl weiterziehen könnte.
    // Die Notiz entstand nach dem Weg über B zurück zu A, also nach dem
    // Beginn der Entscheidung, und gehört ihr nicht (HUM-218).
    final GatedClient client = GatedClient(twoRows());
    final ProviderContainer container = await blockAWhileMoving(
      tester,
      client,
      <FlowId>[b.summary.id, a.summary.id],
      before: (ProviderContainer container) async {
        final Completer<void>? gate = client.gate;
        client.gate = null;
        await container
            .read(interceptDecisionProvider.notifier)
            .send(b.summary.id, const Decision.block(), flow: b.summary);
        client.gate = gate;
      },
    );

    expect(container.read(selectedFlowIdProvider), a.summary.id);
    expect(container.read(blockNoteProvider).text, 'written during the send');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('allow all never turns the draft of the selection into a rule', (
    WidgetTester tester,
  ) async {
    // A ist ausgewählt, trägt einen Fund und bleibt deshalb draußen. Die
    // Regel für eine Stunde im Raster von A darf nicht an die erste saubere
    // Anfrage wandern, und Raster wie Notiz bleiben A.
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...arriveAt(
        held(1, host: 'api.github.com', apex: 'github.com', findings: 1),
        Duration.zero,
      ),
      ...arriveAt(b, const Duration(milliseconds: 10)),
      ...arriveAt(
        held(3, host: 'crates.io', apex: 'crates.io'),
        const Duration(milliseconds: 20),
      ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    final ProviderContainer container = containerOf(tester);
    expect(container.read(selectedFlowIdProvider), a.summary.id);
    container.read(blockNoteProvider.notifier)
      ..open()
      ..write('only for A');
    container
        .read(rememberDraftProvider.notifier)
        .setDuration(RememberDuration.oneHour);
    await tester.pump();

    final InterceptDecision decide = container.read(
      interceptDecisionProvider.notifier,
    );
    decide.askAllowAll();
    await tester.pump();
    await tester.pump();
    // Das Modal nennt keine Regel, weil keine gespeichert wird.
    expect(find.byKey(const Key('intercept-batch-modal')), findsOneWidget);
    expect(find.byKey(const Key('intercept-batch-rule')), findsNothing);
    await decide.confirmBatch();
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(2));
    expect(
      client.decisions.every((RecordedDecision d) => d.remember == null),
      isTrue,
    );
    expect(container.read(selectedFlowIdProvider), a.summary.id);
    expect(container.read(blockNoteProvider).text, 'only for A');
    expect(
      container.read(rememberDraftProvider).effective,
      RememberDuration.oneHour,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets(
    'a question about the selection falls away when the selection moves',
    (WidgetTester tester) async {
      // Das Modal fragt nach der Regel für immer, die im Raster von A steht.
      // Wählt jemand B, bevor er antwortet, gilt die Frage nicht mehr: A
      // darf nicht ohne Regel und ohne Notiz geblockt werden (HUM-218).
      final FakeDaemonClient client = fakeDaemon(twoRows());
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      final ProviderContainer container = containerOf(tester);
      container
          .read(rememberDraftProvider.notifier)
          .setDuration(RememberDuration.forever);
      final InterceptDecision decide = container.read(
        interceptDecisionProvider.notifier,
      );
      await decide.block();
      await tester.pump();
      expect(
        container.read(batchConfirmProvider)?.reason,
        ConfirmReason.forever,
      );

      container.read(selectedFlowIdProvider.notifier).select(b.summary.id);
      await tester.pump();
      expect(container.read(batchConfirmProvider), isNull);

      await decide.confirmBatch();
      await tester.pump();
      expect(client.decisions, isEmpty);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets(
    'a group blocked from its row neither uses nor consumes the drafts',
    (WidgetTester tester) async {
      // A ist ausgewählt, mit Notiz und einer Regel für eine Stunde. Die
      // Gruppe darunter wird aus ihrer Zeile geblockt: Sie bekommt weder die
      // Notiz noch die Regel, und beides bleibt A.
      final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
        ...arriveAt(a, Duration.zero),
        for (int i = 2; i <= 4; i++)
          ...arriveAt(
            held(i, path: '/react/-/react-19.$i.tgz'),
            Duration(milliseconds: 10 * i),
          ),
      ]);
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      final ProviderContainer container = containerOf(tester);
      expect(container.read(selectedFlowIdProvider), a.summary.id);

      container.read(blockNoteProvider.notifier)
        ..open()
        ..write('only for A');
      container
          .read(rememberDraftProvider.notifier)
          .setDuration(RememberDuration.oneHour);
      await tester.pump();

      await hoverOver(tester, find.byType(GroupHeaderRow));
      await tester.pump();
      final TestGesture gesture = await tester.startGesture(
        tester.getCenter(find.byKey(const Key('queue-group-block-npmjs.org'))),
      );
      await tester.pump();
      await tester.pump(HMotion.holdToBlock + const Duration(milliseconds: 50));
      await gesture.up();
      await tester.pump();
      await tester.pump();
      tester.takeException();

      expect(client.decisions, hasLength(3));
      for (final RecordedDecision decided in client.decisions) {
        expect(decided.decision, const Decision.block());
        expect(decided.remember, isNull);
      }
      expect(container.read(selectedFlowIdProvider), a.summary.id);
      expect(container.read(blockNoteProvider).text, 'only for A');
      expect(
        container.read(rememberDraftProvider).effective,
        RememberDuration.oneHour,
      );
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );
}
