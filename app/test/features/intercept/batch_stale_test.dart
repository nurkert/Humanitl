// Das Modal einer Sammelfreigabe bestätigt den Stand von jetzt (HUM-159).
//
// Zwischen dem Öffnen des Modals und dem Klick kann eine Anfrage entschieden
// werden oder aus der Warteschlange fallen. Ginge sie trotzdem mit, fände der
// Daemon sie nicht mehr gehalten, antwortete mit `FLOW_NOT_HELD`, und der Rest
// des Batches bliebe liegen. Die Tests laufen ohne Widget-Baum über einen
// Container; eine Plattform spielt für sie keine Rolle.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';

import 'fixtures.dart';

/// Ein Container über [client], dessen Warteschlange zuhört.
ProviderContainer containerFor(TestDaemonClient client) {
  final ProviderContainer container = ProviderContainer.test(
    overrides: [
      daemonClientProvider.overrideWithValue(client),
      nowProvider.overrideWith(() => FixedNow(testStart)),
      reconnectBackoffProvider.overrideWithValue(
        const Duration(milliseconds: 1),
      ),
    ],
  );
  container
    ..listen(visibleQueueFlowsProvider, (_, _) {})
    ..listen(heldFlowsProvider, (_, _) {});
  return container;
}

/// Zwei gehaltene Anfragen ohne Fund und das offene Modal über beiden.
Future<(ProviderContainer, Flow, Flow)> modalOverTwo(
  TestDaemonClient client,
) async {
  final ProviderContainer container = containerFor(client);
  await settle();
  final Flow first = heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
  );
  final Flow second = heldFlow(
    n: 2,
    deadline: testStart.add(const Duration(minutes: 5)),
  );
  for (final Flow flow in <Flow>[first, second]) {
    client
      ..emit(FlowEvent.received(at: testStart, flow: flow))
      ..emit(
        FlowEvent.held(
          at: testStart,
          flowId: flow.id,
          deadline: testStart.add(const Duration(minutes: 5)),
        ),
      );
  }
  await settle();
  expect(container.read(heldFlowsProvider), hasLength(2));
  container
      .read(batchConfirmProvider.notifier)
      .ask(
        BatchRequest(
          kind: DecisionKind.allow,
          flows: <Flow>[first, second],
          fromSelection: false,
        ),
      );
  return (container, first, second);
}

void main() {
  test('flow decided while modal open', () async {
    final TestDaemonClient client = TestDaemonClient();
    final (ProviderContainer container, Flow first, Flow second) =
        await modalOverTwo(client);

    // Die erste Anfrage entscheidet jemand anders, während das Modal steht.
    client.emit(
      FlowEvent.decided(
        at: testStart,
        flowId: first.id,
        kind: DecisionKind.block,
        source: DecisionSource.user,
      ),
    );
    await settle();

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    expect(
      <FlowId>[for (final (FlowId id, Decision _) in client.decisions) id],
      <FlowId>[second.id],
      reason: 'the decided request is not sent again',
    );
    expect(container.read(interceptDecisionProvider).isSending, isFalse);
  });

  test('flow removed while modal open', () async {
    final TestDaemonClient client = TestDaemonClient();
    final (ProviderContainer container, Flow first, Flow second) =
        await modalOverTwo(client);

    // Nach einer Lücke im Strom kennt der Daemon nur noch die zweite; die
    // Neusynchronisation nimmt die erste aus der Warteschlange.
    client
      ..page = FlowPage(flows: <Flow>[second], total: 1)
      ..emit(FlowEvent.lagged(at: testStart, dropped: 1));
    await settle();
    expect(container.read(flowsProvider).containsKey(first.id), isFalse);

    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    expect(
      <FlowId>[for (final (FlowId id, Decision _) in client.decisions) id],
      <FlowId>[second.id],
      reason: 'the request that left the queue is not sent',
    );
  });
}
