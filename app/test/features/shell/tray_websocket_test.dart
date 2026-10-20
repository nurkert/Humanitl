// „Erlauben“ in der Benachrichtigung gibt keinen WebSocket-Aufbau frei
// (HUM-058, Zeile 11, Review).
//
// Die Karte im Fenster warnt, dass eine Freigabe die Verbindung offen lässt,
// aufgezeichnet, aber nicht mehr angehalten. Eine Meldung kann das nicht
// sagen. Der Knopf fragt deshalb beim Druck nach dem Detail und gibt nur frei,
// was nachweislich keinen Wechsel verlangt; sonst kommt das Fenster auf der
// Anfrage nach vorn.
//
// Gegen welche Umkehrung der Test gerichtet ist: der Zweig `_plainRequest` in
// `_TrayHostState._decide` weg, und der Druck sendet den Aufbau, ohne dass
// jemand die Warnung gesehen hat; die Gegenprobe mit einer gewöhnlichen
// Anfrage steht in `tray_link_down_test.dart`. Der Zweig für
// `TrayDiagnostics.connectionTitle` im Benachrichtigungsschlitz weg, und dort
// stünde der Satz über einen Fund.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/notice.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';

import '../tray/fixtures.dart';
import 'tray_link_down_test.dart' show pumpNotified;

/// Ein Fake, dessen `GetFlow` wartet und dann einen WebSocket-Aufbau nennt.
class SlowUpgradeClient extends FakeDaemonClient {
  /// Hält eine Anfrage an.
  SlowUpgradeClient() : super(script: trayHoldScript(1));

  /// Gibt das Detail frei.
  final Completer<void> gate = Completer<void>();

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    await gate.future;
    final FlowDetail detail = await super.getFlow(id);
    final Flow flow = detail.summary;
    return detail.copyWith(
      request: HttpRequest(
        method: flow.method,
        scheme: flow.scheme,
        authority: flow.authority,
        pathAndQuery: flow.path,
        headers: <Header>[
          Header(name: 'connection', value: 'Upgrade'.codeUnits),
          Header(name: 'upgrade', value: 'websocket'.codeUnits),
        ],
        body: const BodyRef(sha256: <int>[], size: 0),
        version: 'HTTP/1.1',
      ),
    );
  }
}

void main() {
  testWidgets('a_notification_allows_no_websocket_unseen', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final SlowUpgradeClient client = SlowUpgradeClient();
    final container = await pumpNotified(
      tester,
      client: client,
      desktop: desktop,
    );
    expect(desktop.notifications.posts, isNotEmpty);

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();
    // Das Detail ist noch unterwegs: nichts geht hinaus.
    expect(client.decisions, isEmpty);

    client.gate.complete();
    await tester.pump();
    await tester.pump();

    // Es nennt einen WebSocket-Aufbau: nichts geht hinaus, und das Fenster
    // kommt auf der Anfrage nach vorn, mit dem Grund im Benachrichtigungsschlitz.
    expect(client.decisions, isEmpty);
    expect(desktop.window.reveals, 1);
    expect(
      container.read(attentionNoticeProvider)?.title,
      TrayDiagnostics.connectionTitle,
    );
    // Der Schlitz nennt den Grund dieses Falls, nicht den der Funde.
    expect(
      find.text('This connection is decided in the window'),
      findsOneWidget,
    );
    expect(find.text('The request carries a finding'), findsNothing);
    await tester.pump(const Duration(seconds: 11));
    await tester.pump();
  });

  testWidgets('a_notification_allows_no_request_without_headers', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final HeaderlessClient client = HeaderlessClient();
    await pumpNotified(tester, client: client, desktop: desktop);

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty);
    expect(desktop.window.reveals, 1);
    // Kein WebSocket, sondern ein unlesbares Detail: der passende Satz.
    expect(find.text('This request is decided in the window'), findsOneWidget);
    expect(find.text('This connection is decided in the window'), findsNothing);
    await tester.pump(const Duration(seconds: 11));
    await tester.pump();
  });
}

/// Ein Fake, dessen Detail eine Anfrage ohne Kopfzeilen nennt, etwa eine aus
/// einer unvollständigen Aufzeichnung nachgebaute (HUM-058, Review).
class HeaderlessClient extends FakeDaemonClient {
  /// Hält eine Anfrage an.
  HeaderlessClient() : super(script: trayHoldScript(1));

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    final FlowDetail detail = await super.getFlow(id);
    return detail.copyWith(
      request: detail.request!.copyWith(headers: const <Header>[]),
    );
  }
}
