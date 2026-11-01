// Eine kurze Lücke einer Verbindung, die stand, ist kein fehlender Dienst
// (HUM-385).
//
// Der Fall vom Bildschirm: Gleich nach `humanitl daemon restart` verlor das
// Fenster die Verbindung für einen Moment, und der Setup-Bildschirm zeigte
// sofort „The background service does not answer" mit `DAEMON_001`, dem
// Knopf „Install and start" und dem Banner, obwohl der Dienst installiert war
// und nach ein, zwei Sekunden wieder antwortete. Jetzt sagt das Fenster in
// der Frist (`connectionSettleAttempts`) ruhig, dass es neu verbindet; bleibt
// der Dienst weg, steht danach der Befund wie bisher, und ein Befund, der eine
// Antwort ist und keine Abwesenheit, steht sofort.
//
// Gegen Umkehrungen:
// - `settling` in `DaemonConnection._failed` immer false: der erste Test wird
//   rot, weil Banner und Karte sofort stehen.
// - `_attempts < connectionSettleAttempts` weg: der zweite Test wird rot, weil
//   der Befund nie kommt.
// - Die Bedingung auf `DAEMON_001` weg: der dritte Test wird rot.
// - Die Bedingung auf den Takt weg: der vierte Test wird rot.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/widgets/setup_check_row.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/frozen_banner.dart';

import '../../harness/app_harness.dart';

/// Ein Daemon, der wie ein installierter Dienst fehlt: `DAEMON_001` mit der
/// Abhilfe, die Unit zu installieren und zu starten, oder [failure].
class RestartingClient extends FakeDaemonClient {
  /// Erzeugt den Fake, zunächst erreichbar.
  RestartingClient();

  /// Was `GetInfo` gerade wirft, oder null, solange der Daemon antwortet.
  Diagnostic? failure;

  /// Der Dienst ist weg, wie während eines Neustarts.
  void restart() => failure = ClientDiagnostics.daemonUnreachable(
    socketPath: '/run/user/1000/humanitl/daemon.sock',
    detail: 'connection refused',
  );

  /// Der Dienst ist zurück.
  void back() => failure = null;

  @override
  Future<DaemonInfo> getInfo() async {
    final Diagnostic? down = failure;
    if (down != null) {
      throw DaemonException(down);
    }
    return super.getInfo();
  }
}

/// Der Befund mit dem Code [code] in einem Schritt des Setups.
Finder cardWith(String code) => find.byWidgetPredicate(
  (Widget widget) => widget is SetupFinding && widget.diagnostic.code == code,
);

/// Der Zustand der Verbindung im Baum der App.
ConnectionStatus linkOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)))
        .read(connectionStateProvider);

/// Baut die App mit Herzschlag und Takt von zwei Sekunden und öffnet das
/// Setup.
Future<void> openSetup(WidgetTester tester, RestartingClient client) async {
  await pumpApp(
    tester,
    client: client,
    heartbeat: const Duration(seconds: 2),
    reconnect: const Duration(seconds: 2),
  );
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
}

/// Lässt [seconds] Sekunden vergehen und die Antworten durchlaufen.
Future<void> wait(WidgetTester tester, int seconds) async {
  await tester.pump(Duration(seconds: seconds));
  await tester.pump();
  await tester.pump();
}

/// Das ruhige Bild: Banner ohne Grund und Knopf, Zeile ohne Befund und ohne
/// Installation.
void expectReconnecting() {
  expect(find.byType(FrozenBanner), findsOneWidget);
  expect(find.byKey(const Key('shell-reconnecting-title')), findsOneWidget);
  expect(find.text('Reconnecting to the background service'), findsOneWidget);
  expect(find.byKey(const Key('shell-frozen-title')), findsNothing);
  expect(find.byKey(const Key('shell-frozen-why')), findsNothing);
  expect(find.byKey(const Key('shell-frozen-reconnect')), findsNothing);
  expect(find.byKey(const Key('setup-daemon-reconnecting')), findsOneWidget);
  expect(cardWith(DiagnosticCodes.daemonUnreachable), findsNothing);
  expect(find.text('Install and start'), findsNothing);
  expect(find.text('The background service does not answer'), findsNothing);
}

/// Das Bild des Fehlschlags: Banner mit Grund und Knopf, Zeile mit Befund.
void expectFailure(String code) {
  expect(find.byKey(const Key('shell-frozen-title')), findsOneWidget);
  expect(find.byKey(const Key('shell-frozen-why')), findsOneWidget);
  expect(find.byKey(const Key('shell-frozen-reconnect')), findsOneWidget);
  expect(find.byKey(const Key('shell-reconnecting-title')), findsNothing);
  expect(find.byKey(const Key('setup-daemon-reconnecting')), findsNothing);
  expect(cardWith(code), findsOneWidget);
}

void main() {
  testWidgets('a restart that comes back within the grace only ever shows '
      'reconnecting', (WidgetTester tester) async {
    final RestartingClient client = RestartingClient();
    await openSetup(tester, client);
    expect(linkOf(tester), isA<ConnectionConnected>());

    client.restart();
    await wait(tester, 2);
    expect(
      linkOf(tester),
      isA<ConnectionFrozen>().having(
        (ConnectionFrozen link) => link.settling,
        'settling',
        isTrue,
      ),
    );
    expectReconnecting();

    client.back();
    await wait(tester, 2);
    expect(linkOf(tester), isA<ConnectionConnected>());
    expect(find.byType(FrozenBanner), findsNothing);
    expect(cardWith(DiagnosticCodes.daemonUnreachable), findsNothing);
  });

  testWidgets('a service that stays away shows the failure after the grace', (
    WidgetTester tester,
  ) async {
    final RestartingClient client = RestartingClient();
    await openSetup(tester, client);

    client.restart();
    await wait(tester, 2);
    expectReconnecting();
    // Der zweite Fehlschlag liegt noch in der Frist.
    await wait(tester, 2);
    expectReconnecting();
    // Der dritte beendet sie: Der Befund steht mit seiner Abhilfe.
    await wait(tester, 2);
    expectFailure(DiagnosticCodes.daemonUnreachable);
    expect(find.text('Install and start'), findsOneWidget);
    // Und er bleibt, auch über weitere Versuche.
    await wait(tester, 2);
    expectFailure(DiagnosticCodes.daemonUnreachable);

    // Kommt der Dienst doch noch, geht der Befund.
    client.back();
    await wait(tester, 2);
    expect(linkOf(tester), isA<ConnectionConnected>());
    expect(find.byType(FrozenBanner), findsNothing);
  });

  testWidgets('a finding that is an answer shows at once', (
    WidgetTester tester,
  ) async {
    final RestartingClient client = RestartingClient();
    await openSetup(tester, client);

    client.failure = ClientDiagnostics.tokenRejected(
      tokenPath: '/run/user/1000/humanitl/token',
      detail: 'UNAUTHENTICATED',
    );
    await wait(tester, 2);
    expectFailure(DiagnosticCodes.tokenInvalid);
  });

  testWidgets('without the timer there is no grace', (
    WidgetTester tester,
  ) async {
    // Ohne Takt käme kein weiterer Versuch, und die Ruhe verdeckte den
    // Fehlschlag für immer.
    final RestartingClient client = RestartingClient();
    await pumpApp(
      tester,
      client: client,
      heartbeat: const Duration(seconds: 2),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    client.restart();
    await wait(tester, 2);
    expectFailure(DiagnosticCodes.daemonUnreachable);
  });
}
