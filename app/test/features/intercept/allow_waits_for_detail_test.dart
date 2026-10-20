// Keine Freigabe, bevor das Detail da ist (HUM-058, Zeile 11, Review).
//
// Ob eine Anfrage eine WebSocket-Verbindung öffnet, sagt erst ihr Detail, und
// erst dann warnt die Karte davor. Solange `GetFlow` unterwegs ist, ist die
// Freigabe aus, und auch `Enter` sendet nichts.
//
// Gegen welche Umkehrung der Test gerichtet ist: die Prüfung auf das Detail
// in `InterceptDecision.allow` weg, und `Enter` gibt das Upgrade frei, bevor
// die Warnung steht; `detailPending` in der Aktionsleiste weg, und das
// Ventil ist an.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Ein Fake, dessen `GetFlow` wartet, bis [gate] aufgeht.
class SlowDetailClient extends FakeDaemonClient {
  /// Spielt [script] ab.
  SlowDetailClient(List<ScriptedEvent> script)
    : super(script: script, clock: () => testStart);

  /// Gibt die wartenden Antworten frei.
  final Completer<void> gate = Completer<void>();

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    await gate.future;
    return super.getFlow(id);
  }
}

bool valveEnabled(WidgetTester tester) => tester
    .widget<ReleaseValve>(find.byKey(const Key('intercept-allow')))
    .enabled;

void main() {
  testWidgets('allow_waits_for_the_detail', (WidgetTester tester) async {
    final FlowDetail upgrade = detailFor(
      heldFlow(
        n: 1,
        deadline: testStart.add(const Duration(minutes: 5)),
        host: 'ws.example.com',
        path: '/socket',
      ),
      headers: <Header>[
        header('connection', 'Upgrade'),
        header('upgrade', 'websocket'),
      ],
    );
    final SlowDetailClient client = SlowDetailClient(
      holdScript(<FlowDetail>[upgrade]),
    );
    await pumpIntercept(tester, client: client);
    // Länger als das Scharfschalten: Gesperrt ist dann nur noch durch das
    // fehlende Detail.
    await playScript(tester, const Duration(seconds: 1));

    expect(find.byKey(const Key('intercept-card-websocket')), findsNothing);
    expect(valveEnabled(tester), isFalse);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    expect(client.decisions, isEmpty, reason: 'nothing leaves unwarned');

    client.gate.complete();
    await playScript(tester, const Duration(seconds: 1));

    expect(find.byKey(const Key('intercept-card-websocket')), findsOneWidget);
    expect(find.text('Allow connection'), findsOneWidget);
    expect(valveEnabled(tester), isTrue);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await playScript(tester);
    expect(client.decisions, hasLength(1));
  });
}
