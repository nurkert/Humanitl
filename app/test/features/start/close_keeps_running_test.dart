// Schließen hängt ab, stoppt nie (#13): Der Terminalbereich und das Fenster
// der App lassen eine Sitzung laufen, wie das Terminal von `humanitl go`. Sie
// steht danach unter „Running", und ohne angehängtes Terminal sagt die Zeile
// das, nach der Zahl des Daemons. Gestoppt wird nur nach der Frage.
//
// Mutationsbeweis: Ein `Sandbox(Stop)` beim Schließen (etwa in
// `SessionViewNotifier.detach` oder hinter dem Fensterknopf) färbt den ersten
// Test rot; das Abzeichen ohne den Blick auf `terminalClients` färbt den
// zweiten rot.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:humanitl/features/start/start_screen.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';
import '../../harness/session_view.dart';
import '../tray/fixtures.dart';

final DateTime _now = DateTime.utc(2026, 11, 23, 12);

/// Ein Fake mit einer laufenden Sitzung in `api`, der die Stopps zählt und
/// in der Liste die Clients am Terminal meldet wie der Daemon: die offenen
/// Ströme von `Terminal`, oder [clients], wenn gesetzt.
class _Client extends FakeDaemonClient {
  _Client({this.clients}) {
    projects = <ProjectEntry>[
      ProjectEntry(
        projectId: 'p-api',
        path: '/home/dev/api',
        displayName: 'api',
        lastStarted: _now.subtract(const Duration(minutes: 5)),
        lastAgent: 'claude',
        runs: 2,
        running: true,
        sessionId: 'session-p-api',
      ),
    ];
    sandbox = sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
      workDirHost: '/home/dev/api',
    );
  }

  /// Die Zahl, die die Liste nennt; ohne die offenen Ströme.
  final int? clients;

  /// Wie oft `Sandbox(Stop)` kam.
  int stops = 0;

  @override
  Stream<SandboxUpdate> stopSandbox() {
    stops++;
    return super.stopSandbox();
  }

  @override
  Future<List<ProjectEntry>> listProjects() async {
    final List<ProjectEntry> listed = await super.listProjects();
    return <ProjectEntry>[
      for (final ProjectEntry entry in listed)
        ProjectEntry(
          projectId: entry.projectId,
          path: entry.path,
          displayName: entry.displayName,
          lastStarted: entry.lastStarted,
          lastAgent: entry.lastAgent,
          runs: entry.runs,
          running: entry.running,
          sessionId: entry.sessionId,
          changesWaiting: entry.changesWaiting,
          missing: entry.missing,
          replaced: entry.replaced,
          hasChoice: entry.hasChoice,
          terminalClients: entry.running ? (clients ?? openTerminals) : 0,
        ),
    ];
  }
}

Future<void> _settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

void main() {
  testWidgets(
    'closing the terminal area and the window sends no stop; the session '
    'stands under Running, and only the confirmed stop ends it',
    (WidgetTester tester) async {
      final _Client client = _Client();
      final FakeDesktop desktop = FakeDesktop();
      await pumpApp(
        tester,
        client: client,
        // Die Sitzungsansicht dieses Projekts kommt hier, nicht vom Gerüst.
        startWindow: true,
        overrides: [
          nowProvider.overrideWith(() => FixedNow(_now)),
          desktopPortsProvider.overrideWithValue(desktop.ports),
          inSessionView(path: '/home/dev/api', projectId: 'p-api'),
        ],
      );
      await _settle(tester);
      expect(find.byType(ShellScreen), findsOneWidget);

      // Der Terminalbereich geht zu: zurück ins Startfenster.
      await pressCtrl(tester, LogicalKeyboardKey.keyK);
      await tester.enterText(find.byKey(const Key('palette-input')), 'start');
      await tester.pump();
      await tester.tap(find.byKey(const Key('palette-start-window')));
      await _settle(tester);

      expect(find.byType(StartScreen), findsOneWidget);
      expect(client.stops, 0, reason: 'closing the terminal area stopped');
      expect(client.openTerminals, 0, reason: 'the terminal stream detached');
      expect(client.sandbox.state, SandboxState.running);
      expect(
        find.descendant(
          of: find.byKey(const Key('start-running')),
          matching: find.byKey(const Key('start-project-p-api')),
        ),
        findsOneWidget,
      );
      expect(
        find.byKey(const Key('start-no-terminal-p-api')),
        findsOneWidget,
        reason: 'no terminal hangs on the running session',
      );

      // Wieder anhängen und das Fenster schließen.
      await tester.tap(find.byKey(const Key('start-attach-p-api')));
      await _settle(tester);
      expect(find.byType(ShellScreen), findsOneWidget);
      await tester.tap(find.byKey(const Key('window-close')));
      await _settle(tester);
      expect(desktop.window.closes, 1);
      expect(client.stops, 0, reason: 'closing the window stopped');
      expect(client.sandbox.state, SandboxState.running);

      // Stoppen ist eine eigene Handlung mit Frage, im Bereich der Sandbox.
      await tester.tap(
        find.byWidgetPredicate(
          (Widget widget) =>
              widget is RailEntry && widget.section == Section.sandbox,
        ),
      );
      await _settle(tester);
      await tester.tap(find.byKey(const Key('sandbox-stop')));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 200));
      expect(find.byKey(const Key('sandbox-stop-confirm')), findsOneWidget);
      expect(
        client.stops,
        0,
        reason: 'nothing stops while the question stands',
      );
      await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
      await _settle(tester);
      expect(client.stops, 1);
    },
  );

  testWidgets('the running entry says so only when no terminal hangs on it', (
    WidgetTester tester,
  ) async {
    for (final (int clients, bool badge) in <(int, bool)>[
      (0, true),
      (1, false),
    ]) {
      final _Client client = _Client(clients: clients);
      await pumpApp(
        tester,
        client: client,
        startWindow: true,
        overrides: [nowProvider.overrideWith(() => FixedNow(_now))],
      );
      await _settle(tester);
      expect(
        find.byKey(const Key('start-no-terminal-p-api')),
        badge ? findsOneWidget : findsNothing,
        reason: '$clients client(s)',
      );
      if (badge) {
        expect(find.text('No terminal attached'), findsOneWidget);
      }
    }
  });

  testWidgets('in a narrow window the badge gives way and nothing overflows', (
    WidgetTester tester,
  ) async {
    // Mutationsbeweis: das Abzeichen ohne `Flexible` läuft über.
    final _Client client = _Client(clients: 0);
    client.projects = <ProjectEntry>[
      ProjectEntry(
        projectId: 'p-api',
        path: '/home/dev/a-rather-long-project-folder-name',
        displayName: 'a-rather-long-project-folder-name',
        lastStarted: _now.subtract(const Duration(minutes: 5)),
        lastAgent: 'claude',
        runs: 2,
        running: true,
        sessionId: 'session-p-api',
        changesWaiting: true,
      ),
    ];
    await pumpApp(
      tester,
      client: client,
      startWindow: true,
      size: const Size(_narrow, 800),
      overrides: [nowProvider.overrideWith(() => FixedNow(_now))],
    );
    await _settle(tester);
    expect(tester.takeException(), isNull);
    expect(find.byKey(const Key('start-no-terminal-p-api')), findsOneWidget);
  });
}

/// Eine Fensterbreite, in der die Zeile gerade passt, Name und beide
/// Abzeichen aber nicht nebeneinander (gemessen: ohne `Flexible` 198 Pixel
/// zu viel).
const double _narrow = 1040;
