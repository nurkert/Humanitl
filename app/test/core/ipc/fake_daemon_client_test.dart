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

  test('fake_records_the_note', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient();
      final List<FlowEvent> seen = <FlowEvent>[];
      client.subscribe().listen(seen.add);
      async.elapse(const Duration(seconds: 1));
      final FlowId id = seen.whereType<FlowEventHeld>().first.flowId;

      client.decide(id, const Decision.block(note: 'use PyPI'));
      async.flushMicrotasks();

      // Nicht nur im Ereignis: Die Zeile trägt die Notiz, wie sie im Daemon
      // aus der Spalte `decision_note` käme (HUM-117).
      expect(client.state.flow(id)?.decisionNote, 'use PyPI');
      expect(client.state.details[id]?.summary.decisionNote, 'use PyPI');
    });
  });

  test('a recorded session carries the note of a manual block', () {
    final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
    final Iterable<Flow> noted = client.state.flows.values.where(
      (Flow flow) => flow.decisionNote.isNotEmpty,
    );
    expect(noted, isNotEmpty);
    for (final Flow flow in noted) {
      expect(flow.decision, DecisionKind.block);
      expect(flow.decisionSource, DecisionSource.user);
    }
    // Keine Freigabe und kein Ablauf trägt eine.
    for (final Flow flow in client.state.flows.values) {
      if (flow.decision != DecisionKind.block) {
        expect(flow.decisionNote, isEmpty, reason: flow.id.value);
      }
    }
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

  test('apex: compares exactly, host: keeps its suffix match', () {
    fakeAsync((FakeAsync async) {
      final FakeDaemonClient client = FakeDaemonClient(
        clock: () => DateTime(2026, 9, 3, 10),
      );
      client.subscribe().listen((FlowEvent event) {});
      async.elapse(const Duration(seconds: 30));

      List<Flow> rows(String query) {
        FlowPage? page;
        client
            .listFlows(FlowFilter(query: query))
            .then((FlowPage result) => page = result);
        async.flushMicrotasks();
        return page!.flows;
      }

      // Der Daemon vergleicht `apex:` gegen die Spalte, Zeichen für Zeichen;
      // der Fake tut dasselbe. Eine Unterdomain ist kein Treffer, eine
      // Obermenge auch nicht (HUM-091).
      expect(rows('apex:github.com').map((Flow f) => f.host), <String>[
        'api.github.com',
      ]);
      expect(rows('apex:api.github.com'), isEmpty);
      expect(rows('apex:GitHub.COM').length, 1, reason: 'case does not count');

      // Zwei Hosts unter einer Domain: genau dafür gibt es den Schlüssel.
      expect(rows('apex:example.org').map((Flow f) => f.host).toSet(), <String>{
        'example.org',
        'ws.example.org',
      });

      // `host:` bleibt der Suffix-Treffer, den es immer war.
      expect(rows('host:github.com').length, 1);

      // `httpbin.org` steht selbst in der Public Suffix List; der Daemon nennt
      // dafür keine Domain, und der Fake erfindet keine.
      expect(rows('apex:httpbin.org'), isEmpty);

      // Ein Vergleichsoperator gehört zu `status:` und `findings:`, nicht zu
      // `apex:`; der Fake lehnt ihn ab wie der Daemon.
      Object? refusal;
      client.listFlows(const FlowFilter(query: 'apex:>github.com')).catchError((
        Object error,
      ) {
        refusal = error;
        return const FlowPage(flows: <Flow>[], total: 0);
      });
      async.flushMicrotasks();
      expect(refusal, isA<DaemonException>());
      expect((refusal! as DaemonException).code, 'RECORDER_002');
    });
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

  test('SetConfig takes only a sandbox.env variable set to the CA', () async {
    // Dieselbe schmale Tür wie im Daemon (HUM-151). Jede Zeile ist eine
    // Mutation, die rot wird: ohne Präfix, anderer Wert, Name, den keine
    // Umgebung trägt, leerer Name.
    final FakeDaemonClient client = FakeDaemonClient.empty();
    Future<String?> refusal(String key, String value) async {
      try {
        await client.setConfig(key, value);
        return null;
      } on DaemonException catch (error) {
        return error.code;
      }
    }

    expect(
      await refusal('sandbox.env.CURL_CA_BUNDLE', '/etc/humanitl/ca.crt'),
      isNull,
    );
    expect(
      await refusal('CURL_CA_BUNDLE', '/etc/humanitl/ca.crt'),
      'CONFIG_014',
    );
    expect(
      await refusal('sandbox.env.CURL_CA_BUNDLE', '/run/user/1000'),
      'CONFIG_014',
    );
    expect(
      await refusal('sandbox.env.curl-ca', '/etc/humanitl/ca.crt'),
      'CONFIG_014',
    );
    expect(await refusal('sandbox.env.', '/etc/humanitl/ca.crt'), 'CONFIG_014');
    // Nur der angenommene Auftrag steht in der Liste.
    expect(client.configWrites, <(String, String)>[
      ('sandbox.env.CURL_CA_BUNDLE', '/etc/humanitl/ca.crt'),
    ]);
  });
}
