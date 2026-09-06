// Tests des In-Process-Fakes (HUM-019): Skript, Entscheidungen, Szenarien.

import 'dart:async';

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

void main() {
  test('the default script replays in order and then stays open', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient(
        clock: () => DateTime(2026, 9, 3, 10),
      );
      final List<FlowEvent> seen = <FlowEvent>[];
      client.subscribe().listen(seen.add);

      async.elapse(const Duration(milliseconds: 500));
      expect(seen.map((e) => e.runtimeType).toList(), <Type>[
        FlowEventReceived,
        FlowEventAnalyzed,
        FlowEventHeld,
      ]);
      final Flow github = (seen.first as FlowEventReceived).flow;
      expect(github.host, 'api.github.com');
      expect(client.state.flow(github.id)?.isHeld, isTrue);
      expect(client.state.flow(github.id)?.findingCount, 2);

      async.elapse(const Duration(seconds: 3));
      // models.dev: von der Regel geblockt, mit Notiz und Regel-Id.
      final FlowEventDecided blocked = seen.whereType<FlowEventDecided>().first;
      expect(blocked.kind, DecisionKind.block);
      expect(blocked.ruleId, FakeDaemonClient.bundledBlockRule);
      expect(blocked.note, isNotEmpty);
      // Die Durchreiche bleibt ohne include_passthrough unsichtbar.
      expect(
        seen.whereType<FlowEventReceived>().any((e) => e.flow.passthrough),
        isFalse,
      );

      async.elapse(const Duration(seconds: 6));
      expect(
        seen.whereType<FlowEventDiagnostic>().single.diagnostic.code,
        'TLS_001',
      );
      // example.org lief in den Timeout.
      final FlowEventTimedOut timedOut = seen
          .whereType<FlowEventTimedOut>()
          .single;
      expect(
        client.state.flow(timedOut.flowId)?.decision,
        DecisionKind.timedOut,
      );
      expect(client.state.flows.values.where((Flow f) => f.isHeld).length, 3);
    });
  });

  test('include_passthrough shows the LLM flow', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient();
      final List<FlowEvent> seen = <FlowEvent>[];
      client.subscribe(includePassthrough: true).listen(seen.add);
      async.elapse(const Duration(seconds: 3));
      expect(
        seen.whereType<FlowEventReceived>().any((e) => e.flow.passthrough),
        isTrue,
      );
    });
  });

  test('decide emits decided, forwarded, responded, recorded for allow', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient();
      final List<FlowEvent> seen = <FlowEvent>[];
      client.subscribe().listen(seen.add);
      async.elapse(const Duration(seconds: 1));
      final FlowId id = seen.whereType<FlowEventHeld>().first.flowId;
      final int before = seen.length;

      client.decide(id, const Decision.allow());
      async.flushMicrotasks();

      expect(seen.skip(before).map((e) => e.runtimeType).toList(), <Type>[
        FlowEventDecided,
        FlowEventForwarded,
        FlowEventResponseHeaders,
        FlowEventRecorded,
      ]);
      expect(client.state.flow(id)?.state, FlowState.recorded);
      expect(client.state.flow(id)?.status, 200);
      expect(client.decisions.single.decision, const Decision.allow());

      // Ein zweites Mal ist der Flow nicht mehr gehalten: IPC_003.
      Object? error;
      client.decide(id, const Decision.block()).catchError((Object e) {
        error = e;
        return null;
      });
      async.flushMicrotasks();
      expect(error, isA<DaemonException>());
      expect((error! as DaemonException).code, DiagnosticCodes.flowNotHeld);
    });
  });

  test('block records the note and a 403', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient();
      final List<FlowEvent> seen = <FlowEvent>[];
      client.subscribe().listen(seen.add);
      async.elapse(const Duration(seconds: 1));
      final FlowId id = seen.whereType<FlowEventHeld>().first.flowId;
      client.decide(id, const Decision.block(note: 'use PyPI'));
      async.flushMicrotasks();
      final FlowEventDecided decided = seen.whereType<FlowEventDecided>().last;
      expect(decided.kind, DecisionKind.block);
      expect(decided.note, 'use PyPI');
      expect(decided.blockReason, BlockReason.user);
      expect(client.state.flow(id)?.status, 403);
    });
  });

  test('getFlow and getBody return what the script stored', () async {
    final FakeDaemonClient client = FakeDaemonClient();
    final FlowEvent first = await client.subscribe().first;
    final FlowId id = first.flowId!;
    final FlowDetail detail = await client.getFlow(id);
    expect(detail.request?.method, Method.post);
    expect(detail.bodyPreview, contains('createIssue'));
    final List<int> body = (await client.getBody(detail.request!.body).toList())
        .expand((chunk) => chunk)
        .toList();
    expect(body.length, detail.request!.body.size);
    final FlowPage page = await client.listFlows(FlowFilter.all);
    expect(page.flows.single.id, id);
  });

  test('scenarios', () async {
    await expectLater(
      FakeDaemonClient.scenario('unavailable').getInfo(),
      throwsA(
        isA<DaemonException>().having(
          (e) => e.code,
          'code',
          DiagnosticCodes.daemonUnreachable,
        ),
      ),
    );
    expect(
      (await FakeDaemonClient.scenario('mismatch').getInfo()).protoMajor,
      2,
    );
    expect(FakeDaemonClient.scenario('empty').script, isEmpty);
    expect(FakeDaemonClient.scenario('whatever').script, isNotEmpty);
    expect((await FakeDaemonClient().getInfo()).isFake, isTrue);
  });

  test('offline and closed', () async {
    final FakeDaemonClient client = FakeDaemonClient()..goOffline();
    await expectLater(client.getInfo(), throwsA(isA<DaemonException>()));
    client.goOnline();
    expect(await client.getInfo(), FakeDaemonClient.defaultInfo);
    expect(client.infoCalls, 2);
    await client.close();
    expect(client.isClosed, isTrue);
    expect(() => client.getInfo(), throwsStateError);
  });

  test('no terminal survives detach, cancel or the end of the agent', () async {
    final FakeDaemonClient client = FakeDaemonClient();
    const TerminalOpen open = TerminalOpen(sandboxId: '', cols: 80, rows: 24);

    // Der ordentliche Weg hinaus: `Detach` beendet den Strom, und `finish()`
    // traegt den Controller aus.
    final StreamController<TerminalCommand> first =
        StreamController<TerminalCommand>();
    final StreamSubscription<TerminalFrame> firstFrames = client
        .terminal(first.stream)
        .listen((TerminalFrame _) {});
    expect(client.openTerminals, 1);
    first
      ..add(open)
      ..add(const TerminalDetach());
    await pumpEventQueue();
    expect(client.openTerminals, isZero);
    await firstFrames.cancel();
    await first.close();

    // Abbrechen ohne `Detach`: Das loest kein `onDone` aus, also raeumt nur
    // `onCancel` auf. Ohne diese Zeile bliebe der Controller stehen.
    final StreamController<TerminalCommand> second =
        StreamController<TerminalCommand>();
    final StreamSubscription<TerminalFrame> secondFrames = client
        .terminal(second.stream)
        .listen((TerminalFrame _) {});
    second.add(open);
    await pumpEventQueue();
    expect(client.openTerminals, 1);
    await secondFrames.cancel();
    expect(client.openTerminals, isZero);
    await second.close();

    // Das Ende des Agenten: Das `Exit` kommt an, und die Sitzung ist danach
    // keine offene mehr, auch wenn der Strom absichtlich offen bleibt.
    final StreamController<TerminalCommand> third =
        StreamController<TerminalCommand>();
    final List<TerminalFrame> seen = <TerminalFrame>[];
    final StreamSubscription<TerminalFrame> thirdFrames = client
        .terminal(third.stream)
        .listen(seen.add);
    third.add(open);
    await pumpEventQueue();
    client.endTerminals(code: 3);
    await pumpEventQueue();
    expect(seen.whereType<TerminalExit>().single.code, 3);
    expect(client.openTerminals, isZero);
    await thirdFrames.cancel();
    await third.close();
  });

  test('the writing slot belongs to the session and not to the fake', () async {
    final FakeDaemonClient client = FakeDaemonClient();
    const TerminalOpen open = TerminalOpen(sandboxId: '', cols: 80, rows: 24);
    final List<TerminalFrame> first = <TerminalFrame>[];
    final List<TerminalFrame> second = <TerminalFrame>[];
    final List<TerminalFrame> third = <TerminalFrame>[];
    final StreamController<TerminalCommand> firstKeys =
        StreamController<TerminalCommand>();
    final StreamController<TerminalCommand> secondKeys =
        StreamController<TerminalCommand>();
    final StreamController<TerminalCommand> thirdKeys =
        StreamController<TerminalCommand>();

    final StreamSubscription<TerminalFrame> firstFrames = client
        .terminal(firstKeys.stream)
        .listen(first.add);
    firstKeys.add(open);
    await pumpEventQueue();
    expect(first.whereType<TerminalFinding>(), isEmpty);

    // Der Agent endet. Die Sitzung nimmt den Schreiber-Platz mit, also darf
    // das erste Fenster der naechsten Sitzung schreiben.
    client.endTerminals();
    await pumpEventQueue();
    final StreamSubscription<TerminalFrame> secondFrames = client
        .terminal(secondKeys.stream)
        .listen(second.add);
    secondKeys.add(open);
    await pumpEventQueue();
    expect(second.whereType<TerminalFinding>(), isEmpty);

    // Der alte Besitzer verabschiedet sich spaet und nimmt dem neuen nichts:
    // Ein dritter Schreiber bekommt weiterhin `TERM_001`.
    await firstFrames.cancel();
    await firstKeys.close();
    final StreamSubscription<TerminalFrame> thirdFrames = client
        .terminal(thirdKeys.stream)
        .listen(third.add);
    thirdKeys.add(open);
    await pumpEventQueue();
    expect(
      third.whereType<TerminalFinding>().single.diagnostic.code,
      DiagnosticCodes.terminalSecondWriter,
    );

    await secondFrames.cancel();
    await thirdFrames.cancel();
    await secondKeys.close();
    await thirdKeys.close();
  });
}
