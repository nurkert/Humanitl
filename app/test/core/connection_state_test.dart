// Die Verbindungszustände aus HUM-058, Zeilen 1, 3 und 4: kein Daemon beim
// Start, ein Daemon mit anderem Vertrag, ein Daemon, der das Token
// zurückweist. Zeile 2, der Bruch mitten in der Arbeit, steht in
// `test/features/shell/frozen_shell_test.dart` (HUM-044).
//
// Jeder Test hier ist gegen eine Umkehrung gerichtet:
//
// - `_attempts++` in `DaemonConnection._failed` weg: die Zahl bleibt bei 0,
//   die Zeile verschwindet, und `start_without_daemon_shows_setup` wird rot.
// - Die Zeile in `DaemonCheck` nur bei `attempts > 1`: rot im selben Test,
//   weil der erste Fehlschlag schon zählt.
// - `subscribe` beim Kaltstart mit unverträglichem Vertrag:
//   `major_mismatch_incompatible` wird rot.
// - `withTokenFix` weg oder ohne Vorschlag: `unauthenticated` wird rot.

import 'dart:convert';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:grpc/grpc.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/generated/humanitl/v1/humanitl.pb.dart' as pb;
import 'package:humanitl/core/ipc/grpc_daemon_client.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/setup/widgets/setup_check_row.dart';
import 'package:humanitl/features/shell/shell_screen.dart';

import '../harness/app_harness.dart';
import '../harness/ui_state.dart';

/// Ein Fake, der zählt, wie oft jemand den Ereignisstrom öffnet.
class CountingClient extends FakeDaemonClient {
  /// Spricht den Vertrag [protoMajor].
  CountingClient({required int protoMajor})
    : super(
        info: FakeDaemonClient.defaultInfo.copyWith(protoMajor: protoMajor),
      );

  /// Wie oft `subscribe` gerufen wurde.
  int subscribes = 0;

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) {
    subscribes++;
    return super.subscribe(
      since: since,
      includePassthrough: includePassthrough,
    );
  }
}

/// Der Befund mit dem Code [code], im Schritt, zu dem er gehört (HUM-287).
Finder cardWith(String code) => find.byWidgetPredicate(
  (Widget widget) => widget is SetupFinding && widget.diagnostic.code == code,
);

/// Der Text der Zeile, die die Versuche zählt.
String attemptsLine(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('setup-daemon-attempts'))).data!;

void main() {
  /// Der Weg aus dem Akzeptanzkriterium: `HUMANITL_FAKE=daemon_missing`
  /// wählt beim Start den Fake, die App zeigt den Setup-Bildschirm mit der
  /// Karte, und der Zähler läuft, während sie es weiter versucht. Kein
  /// sichtbares Fenster: dieselbe Auflösung wie in `main`, nur im Test.
  testWidgets('start_without_daemon_shows_setup', (WidgetTester tester) async {
    final LaunchOptions options = LaunchOptions.resolve(
      const <String>[],
      environment: const <String, String>{},
      fakeDefine: 'daemon_missing',
      socketDefine: '',
    );
    expect(options.mode, ClientMode.fakeClient);
    await tester.binding.setSurfaceSize(const Size(1280, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ProviderScope(
        overrides: [
          launchOptionsProvider.overrideWithValue(options),
          connectionHeartbeatProvider.overrideWithValue(null),
          connectionReconnectProvider.overrideWithValue(
            const Duration(seconds: 2),
          ),
          uiStateOverride(),
        ],
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    await tester.pump();

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);
    expect(cardWith(DiagnosticCodes.daemonUnreachable), findsOneWidget);
    expect(attemptsLine(tester), '1 attempt failed · trying again');

    // Der Zwei-Sekunden-Takt versucht es weiter, und die Zeile sagt es.
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();
    expect(attemptsLine(tester), '2 attempts failed · trying again');
    await tester.pump(const Duration(seconds: 2));
    await tester.pump();
    await tester.pump();
    expect(attemptsLine(tester), '3 attempts failed · trying again');
    expect(cardWith(DiagnosticCodes.daemonUnreachable), findsOneWidget);
  });

  testWidgets('the attempts line goes once the daemon answers', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()..goOffline();
    await pumpApp(tester, client: client);
    expect(find.byKey(const Key('setup-daemon-attempts')), findsOneWidget);

    client.goOnline();
    await tester.tap(find.byKey(const Key('setup-daemon-retry')));
    await tester.pump();
    await tester.pump();

    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byKey(const Key('setup-daemon-attempts')), findsNothing);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    expect(container.read(connectionStateProvider), isA<ConnectionConnected>());
  });

  testWidgets('major_mismatch_incompatible', (WidgetTester tester) async {
    final CountingClient client = CountingClient(protoMajor: 2);
    await pumpApp(tester, client: client);
    await tester.pump(const Duration(seconds: 1));

    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);
    expect(cardWith(DiagnosticCodes.protoIncompatible), findsOneWidget);
    // Beide Fassungen stehen da, und die Abhilfe ist die Seite mit dem Paar.
    expect(find.textContaining('proto 2.0'), findsOneWidget);
    expect(find.textContaining('this app speaks 1.'), findsOneWidget);
    expect(find.text(ClientDiagnostics.releasesUrl), findsWidgets);
    // Mit einem Daemon, dessen Vertrag diese App nicht liest, öffnet sie
    // keinen Ereignisstrom.
    expect(client.subscribes, 0);

    // Gegenprobe: derselbe Fake mit passendem Vertrag hört zu.
    await tester.pumpWidget(const SizedBox.shrink());
    final CountingClient fitting = CountingClient(protoMajor: 1);
    await pumpApp(tester, client: fitting);
    await tester.pump(const Duration(seconds: 1));
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(fitting.subscribes, greaterThan(0));
  });

  group('unauthenticated', () {
    test('the client finding carries the restart of the unit', () {
      final Diagnostic diagnostic = diagnosticFromGrpcError(
        const GrpcError.unauthenticated('bad token'),
        socketPath: FakeDaemonClient.defaultSocket,
        tokenPath: '/run/user/1000/humanitl/token',
      );
      expect(diagnostic.code, DiagnosticCodes.tokenInvalid);
      expect(
        diagnostic.fix,
        const FixAction.copyCommand(
          command: 'systemctl --user restart humanitld',
        ),
      );
    });

    test('the finding of the daemon gets the same fix and keeps its why', () {
      final pb.Diagnostic shipped = pb.Diagnostic()
        ..code = DiagnosticCodes.tokenInvalid
        ..severity = pb.Severity.SEVERITY_ERROR
        ..why =
            'metadata key x-humanitl-token does not match the session token';
      final GrpcError error = GrpcError.custom(
        StatusCode.unauthenticated,
        'IPC_001',
        const [],
        null,
        <String, String>{
          statusDetailsTrailer: base64
              .encode(shipped.writeToBuffer())
              .replaceAll('=', ''),
        },
      );

      final Diagnostic diagnostic = diagnosticFromGrpcError(
        error,
        socketPath: FakeDaemonClient.defaultSocket,
        tokenPath: '/run/user/1000/humanitl/token',
      );

      expect(diagnostic.why, contains('does not match the session token'));
      expect(
        diagnostic.fix,
        FixAction.copyCommand(command: ClientDiagnostics.restartUnitCommand),
      );
    });

    test('a daemon chosen by hand gets no unit to restart', () {
      for (final (bool fake, bool socketFlag) in <(bool, bool)>[
        (true, false),
        (false, true),
      ]) {
        final Diagnostic diagnostic = diagnosticFromGrpcError(
          const GrpcError.unauthenticated('bad token'),
          socketPath: FakeDaemonClient.defaultSocket,
          tokenPath: '/run/user/1000/humanitl/token',
          fake: fake,
          socketFlag: socketFlag,
        );
        expect(diagnostic.code, DiagnosticCodes.tokenInvalid);
        expect(diagnostic.fix, isNull);
      }
    });

    test('a fix the daemon sent stays, other codes stay untouched', () {
      const Diagnostic own = Diagnostic(
        code: DiagnosticCodes.tokenInvalid,
        severity: Severity.error,
        fix: FixAction.copyCommand(command: 'humanitl daemon restart'),
      );
      expect(ClientDiagnostics.withTokenFix(own), own);
      const Diagnostic other = Diagnostic(
        code: DiagnosticCodes.daemonUnreachable,
        severity: Severity.error,
      );
      expect(ClientDiagnostics.withTokenFix(other), other);
    });

    testWidgets('the fake scenario shows the token finding without a unit', (
      WidgetTester tester,
    ) async {
      await pumpApp(
        tester,
        client: FakeDaemonClient.scenario('unauthenticated'),
      );

      expect(find.byType(SetupScreen), findsOneWidget);
      expect(cardWith(DiagnosticCodes.tokenInvalid), findsOneWidget);
      // Der Fake ist ein Daemon von Hand; die Unit neu zu starten, wäre die
      // Antwort auf eine Frage, die niemand gestellt hat.
      expect(
        find.textContaining(ClientDiagnostics.restartUnitCommand),
        findsNothing,
      );
    });
  });
}
