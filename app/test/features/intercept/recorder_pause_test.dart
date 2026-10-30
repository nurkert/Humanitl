// Die Pause einer gestörten Aufzeichnung an ihren Rändern (HUM-250, Review):
// ein echter Überlauf im Strom, eine neue Sitzung des Daemons, eine
// Freigabe, die erst hinter einem Modal hinausgeht, und eine fortgesetzte
// Freigabe, die sich selbst abweist.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/core/recorder/recorder_health.dart';

import 'fixtures.dart';
import 'harness.dart';

const Diagnostic _failure = Diagnostic(
  code: 'RECORDER_003',
  severity: Severity.error,
  title: 'Aufzeichnung konnte nicht schreiben',
  why: 'database or disk is full',
);

/// Die Verbindung, deren Sitzung der Test von Hand stellt.
class _Connection extends DaemonConnection {
  @override
  ConnectionStatus build() => _connected('session-a');

  void restart(String session) => state = _connected(session);
}

ConnectionStatus _connected(String session) => ConnectionStatus.connected(
  info: DaemonInfo(
    daemonVersion: '0.0.0',
    protoMajor: 1,
    protoMinor: 0,
    sessionId: session,
  ),
);

/// Ein Container über einen Strom, den der Test füllt.
(ProviderContainer, StreamController<FlowEvent>) _container() {
  final StreamController<FlowEvent> events = StreamController<FlowEvent>();
  final ProviderContainer container = ProviderContainer(
    overrides: [
      flowEventsProvider.overrideWith((Ref ref) => events.stream),
      connectionStateProvider.overrideWith(_Connection.new),
    ],
  );
  addTearDown(container.dispose);
  addTearDown(events.close);
  // Die Verbindung steht, wie in der App: Die Pause fragt sie nur, wo es sie
  // schon gibt.
  container.read(connectionStateProvider);
  container.listen(recorderHealthProvider, (_, _) {});
  container.listen(recorderPauseProvider, (_, _) {});
  return (container, events);
}

Future<void> _settle() => Future<void>.delayed(Duration.zero);

/// Ein Fake, in dem die Störung beginnt, sobald die erste Freigabe
/// angenommen ist: mitten in einem Stapel (HUM-250, Review).
class _OutageMidBatch extends FakeDaemonClient {
  _OutageMidBatch(List<ScriptedEvent> script)
    : super(script: script, clock: () => testStart);

  final StreamController<FlowEvent> _extra =
      StreamController<FlowEvent>.broadcast(sync: true);
  bool _fired = false;

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) {
    final StreamController<FlowEvent> merged = StreamController<FlowEvent>(
      sync: true,
    );
    super
        .subscribe(since: since, includePassthrough: includePassthrough)
        .listen(merged.add, onError: merged.addError, onDone: merged.close);
    _extra.stream.listen(merged.add);
    return merged.stream;
  }

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    final Rule? created = await super.decide(id, decision, remember: remember);
    if (!_fired && decision is DecisionAllow) {
      _fired = true;
      _extra.add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
      // Die Kette bis zum Provider besteht aus Mikroaufgaben.
      for (int i = 0; i < 20; i++) {
        await Future<void>.value();
      }
    }
    return created;
  }
}

void main() {
  test('a real lag keeps a running outage and drops an ended one', () async {
    final (ProviderContainer container, StreamController<FlowEvent> events) =
        _container();
    events.add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
    await _settle();
    expect(container.read(recorderHealthProvider), isNotNull);

    // Hinter dem Überlauf liefert der Daemon die Störung nach: Sie bleibt,
    // und die offene Pause steht weiter.
    container
        .read(recorderPauseProvider.notifier)
        .open((RecorderBypass _) async {});
    events
      ..add(FlowEvent.lagged(at: testStart, dropped: 7))
      ..add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
    await _settle();
    expect(container.read(recorderHealthProvider), isNotNull);
    expect(container.read(recorderPauseProvider).isOpen, isTrue);

    // Eine Regeländerung zwischen Überlauf und nachgelieferter Störung
    // entscheidet nichts (Review Runde 2).
    events
      ..add(FlowEvent.lagged(at: testStart, dropped: 4))
      ..add(FlowEvent.rulesChanged(at: testStart, revision: 2))
      ..add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
    await _settle();
    expect(container.read(recorderHealthProvider), isNotNull);
    expect(container.read(recorderPauseProvider).isOpen, isTrue);

    // Ein doppelt gelieferter Befund derselben Störung ändert nichts.
    final RecorderOutage? before = container.read(recorderHealthProvider);
    events.add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
    await _settle();
    expect(container.read(recorderHealthProvider), same(before));

    // Folgt dem Überlauf etwas anderes, lief keine Störung mehr: Ihr Ende
    // war unter dem Verlorenen.
    events
      ..add(FlowEvent.lagged(at: testStart, dropped: 3))
      ..add(FlowEvent.timedOut(at: testStart, flowId: testFlowId(1)));
    await _settle();
    expect(container.read(recorderHealthProvider), isNull);
  });

  test(
    'a new daemon session asks again, a refused resume does not count',
    () async {
      final (ProviderContainer container, StreamController<FlowEvent> events) =
          _container();
      events.add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
      await _settle();
      final RecorderPause pause = container.read(
        recorderPauseProvider.notifier,
      );
      expect(pause.owed, isTrue);

      // Die fortgesetzte Freigabe weist sich selbst ab: keine Bestätigung.
      pause.open((RecorderBypass _) async {});
      await Future<void>.delayed(
        HMotion.rearm + const Duration(milliseconds: 20),
      );
      await pause.confirm();
      expect(pause.owed, isTrue);

      // Sie geht hinaus: bestätigt für diese Sitzung.
      pause.open((RecorderBypass bypass) async => pause.noteAllowSent(bypass));
      await Future<void>.delayed(
        HMotion.rearm + const Duration(milliseconds: 20),
      );
      await pause.confirm();
      expect(pause.owed, isFalse);

      // Nach einem Neustart des Daemons fragt sie von vorn.
      (container.read(connectionStateProvider.notifier) as _Connection).restart(
        'session-b',
      );
      expect(pause.owed, isTrue);
    },
  );

  testWidgets(
    'a modal confirmed after the outage began pauses, block does not',
    (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
        ...holdScript(<FlowDetail>[
          held(1),
          held(2, host: 'api.github.com', apex: 'github.com'),
        ]),
        ScriptedEvent(
          const Duration(seconds: 2),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.diagnostic(at: now, diagnostic: _failure),
        ),
      ]);
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      // Das Modal geht auf, solange die Aufzeichnung schreibt.
      final ProviderContainer container = containerOf(tester);
      container.read(interceptDecisionProvider.notifier).askAllowAll();
      await tester.pump();
      expect(container.read(batchConfirmProvider), isNotNull);

      // Die Störung beginnt, dann wird das Modal bestätigt: Nichts geht hinaus,
      // die Pause steht.
      await tester.pump(const Duration(seconds: 2));
      await tester.pump();
      await container.read(interceptDecisionProvider.notifier).confirmBatch();
      await tester.pump();
      expect(client.decisions, isEmpty);
      expect(find.byKey(const Key('intercept-recorder-pause')), findsOneWidget);

      // Blockieren aus einer Zeile hält nicht an.
      await container
          .read(interceptDecisionProvider.notifier)
          .send(testFlowId(2), const Decision.block());
      await tester.pump();
      expect(client.decisions, hasLength(1));
      expect(client.decisions.single.decision, isA<DecisionBlock>());

      // Bestätigt, geht die angehaltene Freigabe hinaus.
      await tester.pump(HMotion.rearm);
      await tester.pump();
      await tester.tap(find.byKey(const Key('intercept-recorder-pause-send')));
      await tester.pump();
      await tester.pump();
      expect(
        client.decisions.where(
          (RecordedDecision d) => d.decision is DecisionAllow,
        ),
        hasLength(1),
      );
    },
  );

  testWidgets('a decision through the modal pauses once, after the modal', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ScriptedEvent(
        Duration.zero,
        (FakeSessionState state, DateTime now) =>
            FlowEvent.diagnostic(at: now, diagnostic: _failure),
      ),
      ...holdScript(<FlowDetail>[
        held(1),
        held(2, host: 'api.github.com', apex: 'github.com'),
      ]),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    const Key pause = Key('intercept-recorder-pause');

    // Die Störung läuft schon: Das Modal geht trotzdem zuerst auf, ohne Pause.
    final ProviderContainer container = containerOf(tester);
    container.read(interceptDecisionProvider.notifier).askAllowAll();
    await tester.pump();
    expect(container.read(batchConfirmProvider), isNotNull);
    expect(find.byKey(pause), findsNothing);

    // Nach dem Modal die eine Pause; bestätigt, geht alles hinaus, ohne
    // zweite.
    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();
    expect(find.byKey(pause), findsOneWidget);
    expect(client.decisions, isEmpty);
    await tester.pump(HMotion.rearm);
    await tester.pump();
    await tester.tap(find.byKey(const Key('intercept-recorder-pause-send')));
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(2));
    // Die Pause geht in ihrer Animation zu; danach steht keine zweite da.
    await tester.pump(const Duration(milliseconds: 600));
    await tester.pump();
    expect(find.byKey(pause), findsNothing);
  });

  testWidgets('an outage that begins inside a batch stops the rest', (
    WidgetTester tester,
  ) async {
    final _OutageMidBatch client = _OutageMidBatch(
      holdScript(<FlowDetail>[
        held(1),
        held(2, host: 'api.github.com', apex: 'github.com'),
        held(3, host: 'pypi.org', apex: 'pypi.org'),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // Gesund: Das Modal geht auf, bestätigt beginnt der Stapel.
    final ProviderContainer container = containerOf(tester);
    container.read(interceptDecisionProvider.notifier).askAllowAll();
    await tester.pump();
    await container.read(interceptDecisionProvider.notifier).confirmBatch();
    await tester.pump();

    // Nach der ersten Freigabe beginnt die Störung: Die Pause steht, und
    // nichts weiter geht hinaus.
    expect(client.decisions, hasLength(1));
    expect(find.byKey(const Key('intercept-recorder-pause')), findsOneWidget);

    // Bestätigt, gehen genau die übrigen zwei.
    await tester.pump(HMotion.rearm);
    await tester.pump();
    await tester.tap(find.byKey(const Key('intercept-recorder-pause-send')));
    await tester.pump();
    await tester.pump();
    expect(client.decisions, hasLength(3));
    expect(
      client.decisions.map((RecordedDecision d) => d.flowId).toSet(),
      hasLength(3),
    );
  });

  test('overlapping confirmations: one success confirms, a later refusal does not undo it', () async {
    // Eine Freigabe der Warteschlange ist bestätigt und wartet auf den
    // Daemon; inzwischen wird die Pause des Editors bestätigt. Die erste
    // geht hinaus, die zweite wird abgewiesen (HUM-269, Review Runde 2).
    final (ProviderContainer container, StreamController<FlowEvent> events) =
        _container();
    events.add(FlowEvent.diagnostic(at: testStart, diagnostic: _failure));
    await _settle();
    final RecorderPause pause = container.read(recorderPauseProvider.notifier);
    final Completer<void> first = Completer<void>();
    final Completer<void> second = Completer<void>();

    pause.open((RecorderBypass bypass) async {
      await first.future;
      pause.noteAllowSent(bypass);
    });
    await Future<void>.delayed(
      HMotion.rearm + const Duration(milliseconds: 20),
    );
    final Future<void> queue = pause.confirm();

    pause.open((RecorderBypass bypass) async {
      await second.future;
      // Abgewiesen: kein `noteAllowSent`.
    });
    await Future<void>.delayed(
      HMotion.rearm + const Duration(milliseconds: 20),
    );
    final Future<void> editor = pause.confirm();

    first.complete();
    await queue;
    expect(pause.owed, isFalse);

    second.complete();
    await editor;
    expect(pause.owed, isFalse);
  });
}
