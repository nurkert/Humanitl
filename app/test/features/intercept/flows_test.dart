// Unit-Tests der Queue-Provider (HUM-020): Ereignisfaltung, Auswahlregeln,
// Resync nach einer Lücke und die Entscheidung selbst.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/flow_events.dart';

import 'fixtures.dart';

ProviderContainer makeContainer(TestDaemonClient client, FixedNow clock) {
  final ProviderContainer container = ProviderContainer.test(
    overrides: [
      daemonClientProvider.overrideWithValue(client),
      nowProvider.overrideWith(() => clock),
      reconnectBackoffProvider.overrideWithValue(
        const Duration(milliseconds: 1),
      ),
    ],
  );
  // Riverpod 3 pausiert Provider ohne Zuhörer, samt ihrer `ref.listen`; im
  // Baum hängen Widgets daran, im Test hängen diese Zuhörer daran.
  container
    ..listen(visibleQueueFlowsProvider, (_, _) {})
    ..listen(selectedFlowIdProvider, (_, _) {})
    ..listen(heldFlowsProvider, (_, _) {});
  return container;
}

void main() {
  test('flows_apply_sequence', () async {
    final TestDaemonClient client = TestDaemonClient();
    final FixedNow clock = FixedNow(testStart);
    final ProviderContainer container = makeContainer(client, clock);
    // Der Zugriff baut `Flows`, das den Strom abonniert.
    expect(container.read(heldFlowsProvider), isEmpty);
    await settle();

    final DateTime deadline = testStart.add(const Duration(seconds: 300));
    final Flow flow = heldFlow(
      n: 1,
      deadline: deadline,
    ).copyWith(state: FlowState.received, deadline: null, heldAt: null);
    client
      ..emit(FlowEvent.received(at: testStart, flow: flow))
      ..emit(
        FlowEvent.analyzed(
          at: testStart,
          flowId: flow.id,
          findings: const <Finding>[
            Finding(
              kind: 'jwt',
              location: FindingLocation.body,
              spanStart: 0,
              spanEnd: 4,
              tier: FindingTier.regex,
            ),
          ],
        ),
      )
      ..emit(
        FlowEvent.held(at: testStart, flowId: flow.id, deadline: deadline),
      );
    await settle();

    expect(container.read(heldFlowsProvider), hasLength(1));
    final Flow held = container.read(heldFlowsProvider).single;
    expect(held.findingCount, 1);
    expect(held.deadline, deadline);
    expect(held.heldAt, testStart);

    client.emit(
      FlowEvent.decided(
        at: testStart,
        flowId: flow.id,
        kind: DecisionKind.allow,
        source: DecisionSource.user,
      ),
    );
    await settle();

    expect(container.read(heldFlowsProvider), isEmpty);
    expect(container.read(visibleQueueFlowsProvider).flows, hasLength(1));

    // Nach drei Sekunden ist die Zeile auch aus der Ansicht verschwunden.
    clock.moveTo(testStart.add(const Duration(seconds: 4)));
    expect(container.read(visibleQueueFlowsProvider).flows, isEmpty);
  });

  test('selection_never_stolen', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    expect(container.read(selectedFlowIdProvider), isNull);
    await settle();

    final Flow first = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(seconds: 120)),
    );
    client.emit(FlowEvent.received(at: testStart, flow: first));
    await settle();
    expect(container.read(selectedFlowIdProvider), first.id);

    final Flow second = heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(seconds: 60)),
      host: 'pypi.org',
    );
    client.emit(FlowEvent.received(at: testStart, flow: second));
    await settle();

    // Der neue Flow steht wegen der früheren Frist oben, die Auswahl bleibt.
    expect(container.read(visibleQueueFlowsProvider).flows.first.id, second.id);
    expect(container.read(selectedFlowIdProvider), first.id);
  });

  test('selection_moves_on_leave', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    expect(container.read(selectedFlowIdProvider), isNull);
    await settle();

    for (int i = 1; i <= 3; i++) {
      client.emit(
        FlowEvent.received(
          at: testStart,
          flow: heldFlow(
            n: i,
            deadline: testStart.add(Duration(seconds: 60 * i)),
          ),
        ),
      );
    }
    await settle();
    expect(container.read(selectedFlowIdProvider), testFlowId(1));

    client.emit(
      FlowEvent.decided(
        at: testStart,
        flowId: testFlowId(1),
        kind: DecisionKind.allow,
        source: DecisionSource.user,
      ),
    );
    await settle();

    // Der nächste in Frist-Reihenfolge, nicht der neueste.
    expect(container.read(selectedFlowIdProvider), testFlowId(2));
  });

  test('selection_stays_on_a_timeout_until_the_row_leaves', () async {
    final TestDaemonClient client = TestDaemonClient();
    final FixedNow clock = FixedNow(testStart);
    final ProviderContainer container = makeContainer(client, clock);
    expect(container.read(selectedFlowIdProvider), isNull);
    await settle();

    for (int i = 1; i <= 2; i++) {
      client.emit(
        FlowEvent.received(
          at: testStart,
          flow: heldFlow(
            n: i,
            deadline: testStart.add(Duration(seconds: 60 * i)),
          ),
        ),
      );
    }
    await settle();

    client.emit(FlowEvent.timedOut(at: testStart, flowId: testFlowId(1)));
    await settle();
    // Die Karte bleibt stehen, damit der Ausgang zu sehen ist.
    expect(container.read(selectedFlowIdProvider), testFlowId(1));

    clock.moveTo(testStart.add(const Duration(seconds: 4)));
    await settle();
    expect(container.read(selectedFlowIdProvider), testFlowId(2));
  });

  test('resync_on_lagged', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    expect(container.read(heldFlowsProvider), isEmpty);
    await settle();

    final Flow known = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(seconds: 30)),
    );
    client.emit(FlowEvent.received(at: testStart, flow: known));
    await settle();

    final Flow reloaded = heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(seconds: 90)),
      host: 'crates.io',
    );
    client.page = FlowPage(flows: <Flow>[reloaded], total: 1);
    // Seit HUM-034 synchronisiert auch die erste Verbindung; gezaehlt wird
    // deshalb der Zuwachs durch die Luecke, nicht die Gesamtzahl.
    final int beforeGap = client.listFlowsCalls;
    client.emit(FlowEvent.lagged(at: testStart, dropped: 12));
    await settle();

    expect(client.listFlowsCalls, beforeGap + 1);
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id).toList(),
      <FlowId>[reloaded.id],
    );
  });

  test('reconnect_resyncs_after_the_stream_broke', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    expect(container.read(heldFlowsProvider), isEmpty);
    await settle();
    expect(client.streams, hasLength(1));

    client.page = FlowPage(
      flows: <Flow>[
        heldFlow(n: 5, deadline: testStart.add(const Duration(seconds: 45))),
      ],
      total: 1,
    );
    final int beforeBreak = client.listFlowsCalls;
    client.breakStream();
    await Future<void>.delayed(const Duration(milliseconds: 20));
    await settle();

    expect(client.streams.length, greaterThan(1));
    // Wie oben: der Zuwachs durch den Neuaufbau, nicht die Gesamtzahl.
    expect(client.listFlowsCalls, beforeBreak + 1);
    expect(container.read(heldFlowsProvider), hasLength(1));
  });

  test('a refused decision becomes a diagnostic, not an exception', () async {
    final TestDaemonClient client = TestDaemonClient()
      ..decideFailure = const Diagnostic(
        code: DiagnosticCodes.flowNotHeld,
        severity: Severity.warning,
        why: 'flow is not held',
      );
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    await container
        .read(interceptDecisionProvider.notifier)
        .send(testFlowId(1), const Decision.allow());

    final DecisionProgress progress = container.read(interceptDecisionProvider);
    expect(progress, isA<DecisionFailed>());
    expect(
      (progress as DecisionFailed).diagnostic.code,
      DiagnosticCodes.flowNotHeld,
    );
  });
}
