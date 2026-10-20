// Ein Ereignisstrom, der sauber endet, wird wieder aufgebaut (HUM-058,
// Review).
//
// Vorher schloss ein `onDone` ohne Fehler den Strom der Anwendung für immer:
// Nur ein Fehler plante einen neuen `Subscribe`. Ein Daemon, der seinen
// Strom ordentlich beendet, ließ die Warteschlange taub zurück.
//
// Gegen welche Umkehrung der Test gerichtet ist: `onDone` in
// `connectFlowEvents` wieder ohne `onBroken`, und es gibt keinen zweiten
// Strom, keinen Abgleich und keine neue Zeile.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';

import 'fixtures.dart';

void main() {
  test('a_stream_that_ends_cleanly_reconnects_and_resyncs', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = ProviderContainer.test(
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        nowProvider.overrideWith(() => FixedNow(testStart)),
        reconnectBackoffProvider.overrideWithValue(
          const Duration(milliseconds: 1),
        ),
      ],
    );
    container.listen(heldFlowsProvider, (_, _) {});
    await settle();
    expect(client.streams, hasLength(1));

    final Flow waiting = heldFlow(
      n: 3,
      deadline: testStart.add(const Duration(seconds: 90)),
    );
    client.page = FlowPage(flows: <Flow>[waiting], total: 1);
    final int before = client.listFlowsCalls;

    // Der Daemon schließt den Strom ordentlich, ohne Fehler.
    await client.current.close();
    await Future<void>.delayed(const Duration(milliseconds: 20));
    await settle();

    expect(client.streams, hasLength(2), reason: 'a new Subscribe');
    expect(client.listFlowsCalls, before + 1, reason: 'the gap is resynced');
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id),
      <FlowId>[waiting.id],
    );

    // Und der neue Strom trägt: ein später gehaltener Flow erscheint.
    final Flow later = heldFlow(
      n: 4,
      deadline: testStart.add(const Duration(seconds: 120)),
      host: 'crates.io',
    );
    client.emit(FlowEvent.received(at: testStart, flow: later));
    await settle();
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id),
      containsAll(<FlowId>[waiting.id, later.id]),
    );
  });
}
