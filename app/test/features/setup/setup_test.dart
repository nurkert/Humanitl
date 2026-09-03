// Widget-Tests des Verbindungs-Gates (HUM-019): fehlender Daemon,
// Versionskonflikt, erneut verbinden, Herzschlag.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/splash.dart';

import '../../harness/app_harness.dart';

void main() {
  testWidgets('connection_failed_shows_setup', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient.unavailable());

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);
    expect(find.text(DiagnosticCodes.daemonUnreachable), findsOneWidget);
    expect(find.text('Daemon not reachable'), findsOneWidget);
    // Die Detailzeile nennt den Socket.
    expect(find.textContaining(FakeDaemonClient.defaultSocket), findsOneWidget);
    // Der Fix ist der Startbefehl des Fake-Daemons.
    expect(
      find.textContaining('humanitld --fake fixtures/sessions/mixed.jsonl'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('setup-retry')), findsOneWidget);
  });

  testWidgets('version_mismatch_shows_setup', (WidgetTester tester) async {
    await pumpApp(tester, client: FakeDaemonClient.incompatible());

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.text(DiagnosticCodes.protoIncompatible), findsOneWidget);
    expect(find.text('Incompatible daemon'), findsOneWidget);
    expect(find.textContaining('proto 2.0'), findsOneWidget);
  });

  testWidgets('a compatible minor is accepted', (WidgetTester tester) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient.incompatible(protoMajor: 1, protoMinor: 7),
    );
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('retry reconnects once the daemon is back', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()..goOffline();
    await pumpApp(tester, client: client);
    expect(find.byType(SetupScreen), findsOneWidget);

    client.goOnline();
    await tester.tap(find.byKey(const Key('setup-retry')));
    await tester.pump();
    expect(find.byType(Splash), findsOneWidget);
    await tester.pump();
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(client.infoCalls, 2);
  });

  testWidgets('the heartbeat notices a stopped daemon', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpApp(
      tester,
      client: client,
      heartbeat: const Duration(seconds: 1),
    );
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(client.infoCalls, 1);

    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(client.infoCalls, 2);
    expect(find.byType(ShellScreen), findsOneWidget);

    client.goOffline();
    await tester.pump(const Duration(seconds: 1));
    await tester.pump();
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.text(DiagnosticCodes.daemonUnreachable), findsOneWidget);

    // Nach dem Fehler schlägt nichts mehr; erneut verbinden startet neu.
    final int calls = client.infoCalls;
    await tester.pump(const Duration(seconds: 3));
    expect(client.infoCalls, calls);

    client.goOnline();
    await tester.tap(find.byKey(const Key('setup-retry')));
    await tester.pump();
    await tester.pump();
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('a daemon diagnostic with its own code is shown as is', (
    WidgetTester tester,
  ) async {
    const Diagnostic shipped = Diagnostic(
      code: 'IPC_001',
      severity: Severity.error,
      title: 'Ungültiges Token',
      why: 'metadata key x-humanitl-token does not match the session token',
    );
    await pumpApp(tester, client: FakeDaemonClient(infoFailure: shipped));
    expect(find.text('IPC_001'), findsOneWidget);
    expect(find.text('Token rejected'), findsOneWidget);
    expect(
      find.textContaining('does not match the session token'),
      findsOneWidget,
    );
  });
}
