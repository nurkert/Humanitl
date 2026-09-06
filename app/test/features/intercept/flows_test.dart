// Unit-Tests der Queue-Provider (HUM-020): Ereignisfaltung, Auswahlregeln,
// Resync nach einer Lücke und die Entscheidung selbst.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
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

/// Ein Client, der ganz weggeht: `GetInfo` scheitert, solange [online] falsch
/// ist, und der Ereignisstrom bricht mit ihm.
class OfflineClient extends TestDaemonClient {
  /// Ob der Daemon antwortet.
  bool online = true;

  @override
  Future<DaemonInfo> getInfo() async {
    if (!online) {
      throw DaemonException(
        const Diagnostic(
          code: DiagnosticCodes.daemonUnreachable,
          severity: Severity.error,
          why: 'the daemon is not answering',
        ),
      );
    }
    return super.getInfo();
  }
}

/// Ein Client, dessen `ListFlows` erst antwortet, wenn der Test ihn loslässt.
///
/// Das ist das Fenster, in dem die Warteschlange eine Anfrage verlieren kann:
/// Der Daemon hat seine Seite schon gebaut, der Ereignisstrom läuft weiter,
/// und was jetzt ankommt, steht in keiner Antwort, die schon unterwegs ist.
class GatedListFlowsClient extends TestDaemonClient {
  Completer<void> _gate = Completer<void>();

  /// Lässt die wartende Antwort durch.
  void release() => _gate.complete();

  /// Legt den Riegel für den nächsten Aufruf wieder vor.
  ///
  /// Der erste Abgleich läuft schon beim Aufbau des Behälters, lange bevor
  /// ein Test eine Zeile in die Karte legen kann. Wer das Fenster über einer
  /// **gehaltenen** Zeile braucht, lässt den ersten Aufruf durch, legt den
  /// Riegel wieder vor und löst dann die Lücke aus.
  void rearm() => _gate = Completer<void>();

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    // Gezählt wird der Aufruf, nicht die Antwort: Der Test steht genau in dem
    // Fenster dazwischen.
    listFlowsCalls++;
    await _gate.future;
    return page;
  }
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

  /// Die Neusynchronisation ist nur über die Zeilen maßgeblich, nach denen sie
  /// gefragt hat. Was während des Aufrufs ankommt, steht in keiner Antwort,
  /// die schon unterwegs war, und seine Ereignisse sind verbraucht: Ein Purge
  /// über alles Gehaltene löschte die Zeile für immer, und kein `Lagged` holte
  /// sie zurück.
  test('a request that arrives during the resync stays in the queue', () async {
    final GatedListFlowsClient client = GatedListFlowsClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    expect(container.read(heldFlowsProvider), isEmpty);
    await settle();
    // Die erste Verbindung synchronisiert; die Antwort hängt am Riegel.
    expect(client.listFlowsCalls, isPositive);
    expect(container.read(heldFlowsProvider), isEmpty);

    // Was der Daemon wusste, als er die Seite baute.
    final Flow listed = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(seconds: 30)),
    );
    client.page = FlowPage(flows: <Flow>[listed], total: 1);

    // Und was danach kam, während die Antwort unterwegs war.
    final Flow late = heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(seconds: 90)),
      host: 'crates.io',
    );
    client.emit(FlowEvent.received(at: testStart, flow: late));
    await settle();
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id).toList(),
      <FlowId>[late.id],
    );

    client.release();
    await settle();

    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id).toSet(),
      <FlowId>{listed.id, late.id},
    );
  });

  /// Der Ereignisstrom wartet seine Rückfallzeit nicht ab, wenn die Leitung
  /// nachweislich zurück ist.
  ///
  /// `GetInfo` und `Subscribe` sind zwei Aufrufe über denselben Socket. Der
  /// erste kommt über den Zwei-Sekunden-Takt der Verbindung zurück, und
  /// `linkLiveProvider` sagt es; der zweite säße sonst noch auf einer
  /// Wartezeit, die sich bis auf 30 s verdoppelt hat, und die Shell behauptete
  /// eine halbe Minute lang, sie sei lebendig, während kein Ereignis mehr
  /// ankommt.
  test(
    'the event stream comes back with the link, not with its backoff',
    () async {
      final OfflineClient client = OfflineClient();
      final ProviderContainer container = ProviderContainer.test(
        overrides: [
          daemonClientProvider.overrideWithValue(client),
          nowProvider.overrideWith(() => FixedNow(testStart)),
          // Weit länger als alles, was dieser Test abwartet: Ein Strom, der von
          // selbst wiederkäme, bewiese nichts.
          reconnectBackoffProvider.overrideWithValue(
            const Duration(seconds: 30),
          ),
          connectionHeartbeatProvider.overrideWithValue(
            const Duration(milliseconds: 20),
          ),
          connectionReconnectProvider.overrideWithValue(
            const Duration(milliseconds: 20),
          ),
        ],
      );
      container
        ..listen(heldFlowsProvider, (_, _) {})
        ..listen(connectionStateProvider, (_, _) {});
      await settle();
      expect(container.read(linkLiveProvider), isTrue);
      expect(client.streams, hasLength(1));

      // Der Daemon geht weg: der Strom bricht, und der Herzschlag merkt es.
      client.online = false;
      client.breakStream();
      await Future<void>.delayed(const Duration(milliseconds: 120));
      await settle();
      expect(container.read(linkLiveProvider), isFalse);
      final int whileDown = client.streams.length;

      // Und er kommt zurück. Der Zwei-Sekunden-Takt der Verbindung findet ihn
      // binnen 20 ms; die Rückfallzeit des Stroms liefe noch 30 s.
      client.online = true;
      await Future<void>.delayed(const Duration(milliseconds: 200));
      await settle();

      expect(container.read(linkLiveProvider), isTrue);
      expect(
        client.streams.length,
        greaterThan(whileDown),
        reason: 'the stream reconnects with the link, not after its backoff',
      );
    },
  );

  /// Und der Riegel davor ist die Entscheidung, nicht „nicht gehalten".
  ///
  /// Eine Zeile, die hier `received` steht, weil ihr `Held` in der Lücke
  /// verlorenging, ist genau der Fall, für den es diesen Abgleich gibt: Sie
  /// muss die Seite bekommen. Ein Riegel auf `!isHeld` überspränge sie und
  /// ließe sie für immer als Ankunft ohne Frist stehen.
  test('a row whose Held was dropped is repaired by the page', () async {
    final TestDaemonClient client = TestDaemonClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    await settle();

    final Flow held = heldFlow(
      n: 3,
      deadline: testStart.add(const Duration(seconds: 60)),
    );
    // Was ankam, bevor die Lücke begann: empfangen, aber ohne das `Held`.
    client.emit(
      FlowEvent.received(
        at: testStart,
        flow: held.copyWith(
          state: FlowState.received,
          deadline: null,
          heldAt: null,
        ),
      ),
    );
    await settle();
    expect(container.read(heldFlowsProvider), isEmpty);

    client.page = FlowPage(flows: <Flow>[held], total: 1);
    final int beforeGap = client.listFlowsCalls;
    client.emit(FlowEvent.lagged(at: testStart, dropped: 3));
    await settle();

    expect(client.listFlowsCalls, beforeGap + 1);
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id).toList(),
      <FlowId>[held.id],
      reason: 'the resync exists to repair exactly this row',
    );
  });

  /// Die andere Hälfte desselben Fensters: Was während des Aufrufs
  /// **entschieden** wird, ist neuer als die Seite, die schon unterwegs ist.
  ///
  /// `ListFlows` liest den Recorder, und der sieht eine Entscheidung erst,
  /// wenn er das `Decided` verarbeitet hat; der Daemon kann das Ereignis also
  /// ausliefern und trotzdem die ältere Seite beantworten, auf der die Anfrage
  /// noch gehalten steht. Wer diese Seite über die entschiedene Zeile
  /// schriebe, hinge eine bereits weitergeleitete Anfrage wieder als wartende
  /// in die Warteschlange, mit gelöschter Entscheidung und laufendem
  /// Countdown; ein zweites `Decide` darauf beantwortet der Daemon mit
  /// `FLOW_NOT_HELD`.
  test('a decision taken during the resync survives the older page', () async {
    final GatedListFlowsClient client = GatedListFlowsClient();
    final ProviderContainer container = makeContainer(
      client,
      FixedNow(testStart),
    );
    await settle();
    // Die erste Verbindung synchronisiert; die Antwort hängt am Riegel.
    expect(client.listFlowsCalls, isPositive);

    final Flow known = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(seconds: 30)),
    );
    // Die Seite, die der Daemon gebaut hat, bevor die Entscheidung bei seinem
    // Recorder ankam: die Anfrage steht darauf noch als gehalten.
    client.page = FlowPage(flows: <Flow>[known], total: 1);
    client.emit(FlowEvent.received(at: testStart, flow: known));
    client.emit(
      FlowEvent.held(
        at: testStart,
        flowId: known.id,
        deadline: known.deadline!,
      ),
    );
    await settle();
    expect(container.read(heldFlowsProvider), hasLength(1));

    // Und die Entscheidung, die fällt, während die Antwort unterwegs ist.
    client.emit(
      FlowEvent.decided(
        at: testStart.add(const Duration(seconds: 5)),
        flowId: known.id,
        kind: DecisionKind.allow,
        source: DecisionSource.user,
      ),
    );
    await settle();
    expect(container.read(heldFlowsProvider), isEmpty);

    client.release();
    await settle();

    final Flow after = container.read(flowsProvider)[known.id]!;
    expect(after.state, FlowState.decided);
    expect(after.decision, DecisionKind.allow);
    expect(after.decisionSource, DecisionSource.user);
    expect(
      container.read(heldFlowsProvider),
      isEmpty,
      reason: 'an allowed request never returns to the queue',
    );
  });

  /// Und dieselbe Entscheidung in der anderen Reihenfolge, in der der Daemon
  /// sie meistens ausliefert: Er hat sie verarbeitet, **bevor** er seinen
  /// eigenen Zustand gelesen hat, also steht die Zeile nicht auf der Antwort.
  ///
  /// Sie steht damit in der Menge, nach der dieser Aufruf gefragt hat -- beim
  /// Absenden war sie gehalten --, und fehlt in der Antwort. Ein Purge über
  /// diese Differenz allein nähme die Zeile in genau dem Frame weg, in dem die
  /// Entscheidung ankam, und mit ihr die drei Sekunden [queueExitWindow], in
  /// denen ihr Bestätigungsstreifen steht (`docs/UX.md` 4.6). Wofür der Purge
  /// da ist, ist enger: eine Zeile, die der Daemon nicht mehr hält **und** die
  /// diese Anwendung nie entschieden gesehen hat. Der zweite Flow in diesem
  /// Test ist genau die, und sie geht.
  test('a decision the daemon already dropped keeps its exit window', () async {
    final GatedListFlowsClient client = GatedListFlowsClient();
    final FixedNow clock = FixedNow(testStart);
    final ProviderContainer container = makeContainer(client, clock);
    await settle();
    // Der Abgleich der ersten Verbindung darf durch; danach liegt der Riegel
    // wieder vor, und das Fenster gehört dem Abgleich, den dieser Test macht.
    client.release();
    await settle();
    client.rearm();

    final Flow decided = heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(seconds: 30)),
    );
    // Die zweite Zeile ist die Gegenprobe: gehalten, vom Daemon nicht mehr
    // geführt, und hier nie entschieden gesehen.
    final Flow dropped = heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(seconds: 60)),
      host: 'crates.io',
    );
    for (final Flow flow in <Flow>[decided, dropped]) {
      client
        ..emit(FlowEvent.received(at: testStart, flow: flow))
        ..emit(
          FlowEvent.held(
            at: testStart,
            flowId: flow.id,
            deadline: flow.deadline!,
          ),
        );
    }
    await settle();
    expect(container.read(heldFlowsProvider), hasLength(2));

    // Die Lücke: Der Abgleich fragt nach genau diesen beiden Zeilen und
    // wartet auf die Antwort.
    final int before = client.listFlowsCalls;
    client.page = const FlowPage();
    client.emit(FlowEvent.lagged(at: testStart, dropped: 4));
    await settle();
    expect(client.listFlowsCalls, before + 1);

    // Und die Entscheidung fällt, während die Antwort unterwegs ist.
    client.emit(
      FlowEvent.decided(
        at: testStart,
        flowId: decided.id,
        kind: DecisionKind.allow,
        source: DecisionSource.user,
      ),
    );
    await settle();
    // Sie ist aus den gehaltenen heraus und steht als entschiedene noch da;
    // die andere wartet weiter, denn die Antwort ist noch unterwegs.
    expect(
      container.read(heldFlowsProvider).map((Flow flow) => flow.id),
      <FlowId>[dropped.id],
    );
    expect(
      container
          .read(visibleQueueFlowsProvider)
          .flows
          .map((Flow flow) => flow.id)
          .toSet(),
      <FlowId>{decided.id, dropped.id},
    );

    client.release();
    await settle();

    // Die entschiedene Zeile trägt ihren Streifen zu Ende ...
    expect(
      container.read(visibleQueueFlowsProvider).flows.map((Flow f) => f.id),
      <FlowId>[decided.id],
      reason: 'a decided row keeps its three seconds after the resync',
    );
    expect(
      container.read(flowsProvider)[decided.id]?.decision,
      DecisionKind.allow,
    );
    // ... und die andere ist weg, denn der Daemon hält sie nicht mehr und
    // niemand hat sie hier entschieden gesehen.
    expect(container.read(flowsProvider).containsKey(dropped.id), isFalse);
    expect(container.read(heldFlowsProvider), isEmpty);

    // Nach dem Fenster geht auch die entschiedene Zeile; die Warteschlange
    // wächst nicht.
    clock.moveTo(testStart.add(const Duration(seconds: 4)));
    await settle();
    expect(container.read(visibleQueueFlowsProvider).flows, isEmpty);
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
