// Tests des In-Process-Fakes (HUM-019): Skript, Entscheidungen, Szenarien.

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
}
