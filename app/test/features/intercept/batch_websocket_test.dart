// Eine Freigabe über mehrere Anfragen nimmt keinen WebSocket-Aufbau mit
// (HUM-058, Zeile 11, Review Runde 2).
//
// Die Karte einer Mehrfachauswahl warnt vor keinem Protokollwechsel. Jede
// Freigabe über mehrere Anfragen geht durch `InterceptDecision._many`, und
// dort bleibt gehalten, was einen Wechsel verlangt oder dessen Detail nicht
// ankommt; die übrigen gehen hinaus, und die Leiste sagt, warum eine stehen
// blieb.
//
// Gegen welche Umkehrung der Test gerichtet ist: die Prüfung
// `_withoutUpgrade` in `_many` weg, und der Aufbau geht mit der Gruppe hinaus.

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';

import 'fixtures.dart';
import 'harness.dart';

Future<void> pressChord(WidgetTester tester, LogicalKeyboardKey key) async {
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(key);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
}

/// Drei Anfragen an dieselbe Domain; die zweite verlangt einen WebSocket.
List<ScriptedEvent> script() {
  final List<FlowDetail> details = <FlowDetail>[
    for (int i = 1; i <= 3; i++)
      held(i, host: 'registry.npmjs.org', path: '/react/$i'),
  ];
  final FlowDetail upgrade = details[1];
  details[1] = upgrade.copyWith(
    request: upgrade.request!.copyWith(
      headers: <Header>[
        header('connection', 'Upgrade'),
        header('upgrade', 'websocket'),
      ],
    ),
  );
  return <ScriptedEvent>[
    for (int i = 0; i < details.length; i++)
      ...arriveAt(details[i], Duration(milliseconds: 10 * i)),
  ];
}

/// Ein Fake, dessen Detail zur ersten Anfrage nicht ankommt.
class LostDetailClient extends FakeDaemonClient {
  LostDetailClient(List<ScriptedEvent> script)
    : super(script: script, clock: () => testStart);

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    if (id == testFlowId(1)) {
      throw const DaemonException(
        Diagnostic(
          code: 'DAEMON_001',
          severity: Severity.error,
          why: 'no daemon answers',
        ),
      );
    }
    return super.getFlow(id);
  }
}

void main() {
  testWidgets('a_group_allow_leaves_the_websocket_held', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(script());
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // Die Gruppe wählen, dann senden (dasselbe Kürzel zweimal).
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions.map((RecordedDecision d) => d.flowId),
      unorderedEquals(<FlowId>[testFlowId(1), testFlowId(3)]),
    );
    final Flow? left = containerOf(tester).read(flowsProvider)[testFlowId(2)];
    expect(left?.isHeld, isTrue, reason: 'the WebSocket stays held');
    // Der Cursor steht auf ihr: Auf ihrer Karte steht die Warnung.
    expect(containerOf(tester).read(selectedFlowIdProvider), testFlowId(2));
    expect(
      find.text(
        'A request that may open a WebSocket connection stays held · '
        'decide it on its own card',
      ),
      findsOneWidget,
    );
  });

  testWidgets('a request whose detail does not arrive stays held too', (
    WidgetTester tester,
  ) async {
    final LostDetailClient client = LostDetailClient(script());
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await pressChord(tester, LogicalKeyboardKey.keyF);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(client.decisions.map((RecordedDecision d) => d.flowId), <FlowId>[
      testFlowId(3),
    ]);
  });
}
