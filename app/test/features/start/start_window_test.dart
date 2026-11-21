// Das Startfenster (HUM-537): erster Bildschirm ohne Sitzungsansicht,
// laufende Sitzung anhängen, zuletzt genutzte Projekte starten, aufräumen
// nur nach der Liste des Daemons, und der Rückweg aus einer Sitzung.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/start/providers/projects.dart';
import 'package:humanitl/features/start/start_screen.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';
import '../../harness/session_view.dart';

final DateTime _now = DateTime.utc(2026, 11, 21, 12);

ProjectEntry _project(
  String id,
  String name, {
  Duration ago = const Duration(hours: 2),
  String agent = 'claude',
  bool running = false,
  bool missing = false,
}) => ProjectEntry(
  projectId: id,
  path: '/home/dev/$name',
  displayName: name,
  lastStarted: _now.subtract(ago),
  lastAgent: agent,
  runs: 3,
  running: running,
  sessionId: running ? 'session-$id' : '',
  missing: missing,
);

/// Ein Fake mit drei Projekten, das jüngste zuerst, das mittlere ohne Ordner.
FakeDaemonClient _client({bool running = false}) {
  final FakeDaemonClient client = FakeDaemonClient()
    ..projects = <ProjectEntry>[
      _project(
        'p-api',
        'api',
        ago: const Duration(minutes: 5),
        running: running,
      ),
      _project('p-gone', 'gone', ago: const Duration(hours: 3), missing: true),
      _project('p-web', 'web', ago: const Duration(days: 2), agent: 'codex'),
    ];
  if (running) {
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
      workDirHost: '/home/dev/api',
    );
  }
  return client;
}

Future<void> _pumpStart(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(
    tester,
    client: client,
    startWindow: true,
    overrides: [nowProvider.overrideWith(() => FixedNow(_now))],
  );
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('without a project and without a session the app shows the start '
      'window and starts nothing, even with sandbox.work_dir set', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..projects = const <ProjectEntry>[];
    // Ein gespeicherter Ordner ist kein Projekt: Vor HUM-537 öffnete die
    // App mit ihm still eine Sitzungsansicht.
    await client.setConfig(FormKeys.sandboxWorkDir, '/home/dev/acme');

    await _pumpStart(tester, client);
    await tester.pump(const Duration(milliseconds: 100));

    expect(find.byType(StartScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);
    expect(find.byKey(const Key('start-empty')), findsOneWidget);
    expect(client.startWorkDirs, isEmpty);
    expect(client.startModes, isEmpty);
  });

  testWidgets('a running session stands under Running and attaches; a start of '
      'another project names CLI_005 and stays in the start window', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(running: true);
    await _pumpStart(tester, client);

    expect(find.byType(StartScreen), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const Key('start-running')),
        matching: find.byKey(const Key('start-project-p-api')),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byKey(const Key('start-recent')),
        matching: find.byKey(const Key('start-project-p-api')),
      ),
      findsNothing,
    );

    await tester.tap(find.byKey(const Key('start-start-p-web')));
    await tester.pump();
    await tester.pump();

    expect(client.startProjectIds, <String?>['p-web']);
    expect(find.byKey(const Key('start-diagnostic-CLI_005')), findsOneWidget);
    expect(find.byType(StartScreen), findsOneWidget);
    expect(find.byType(ShellScreen), findsNothing);

    await tester.tap(find.byKey(const Key('start-attach-p-api')));
    await tester.pump();
    await tester.pump();

    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(StartScreen), findsNothing);
    // Anhängen startet nichts.
    expect(client.startProjectIds, <String?>['p-web']);
  });

  testWidgets(
    'recent projects keep the order and content of the daemon; a missing '
    'folder is not startable, a present one starts in its folder',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client();
      await _pumpStart(tester, client);

      final List<double> rows = <String>['p-api', 'p-gone', 'p-web']
          .map(
            (String id) =>
                tester.getTopLeft(find.byKey(Key('start-project-$id'))).dy,
          )
          .toList();
      expect(rows[0], lessThan(rows[1]));
      expect(rows[1], lessThan(rows[2]));

      expect(find.text('api'), findsWidgets);
      expect(find.text('/home/dev/web'), findsOneWidget);
      expect(
        tester.widget<Text>(find.byKey(const Key('start-agent-p-web'))).data,
        'codex',
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('start-age-p-api'))).data,
        '5 min ago',
      );
      expect(
        tester.widget<Text>(find.byKey(const Key('start-age-p-web'))).data,
        '2 days ago',
      );

      expect(find.byKey(const Key('start-missing-p-gone')), findsOneWidget);
      await tester.tap(
        find.byKey(const Key('start-start-p-gone')),
        warnIfMissed: false,
      );
      await tester.pump();
      expect(client.startWorkDirs, isEmpty);

      await tester.tap(find.byKey(const Key('start-start-p-api')));
      await tester.pump();
      await tester.pump();

      // An der Kennung, nie am Pfad der Anzeige (HUM-537, Vertrag 1.58).
      expect(client.startProjectIds, <String?>['p-api']);
      expect(client.startWorkDirs, <String?>[null]);
      expect(find.byType(ShellScreen), findsOneWidget);
    },
  );

  testWidgets(
    'a new project goes through the folder dialog and the first-start '
    'screen, and starts in that folder by the id the saved choice got',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client();
      await pumpApp(
        tester,
        client: client,
        startWindow: true,
        overrides: [
          nowProvider.overrideWith(() => FixedNow(_now)),
          directoryChooserProvider.overrideWithValue(
            () async => '/home/dev/fresh',
          ),
        ],
      );
      await tester.pump();

      await tester.tap(find.byKey(const Key('start-new-project')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      // Ein unbekannter Ordner bekommt den Erststart-Bildschirm (HUM-538).
      expect(find.byKey(const Key('first-start')), findsOneWidget);
      expect(client.startProjectIds, isEmpty);
      await tester.tap(find.byKey(const Key('first-start-start')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(client.savedChoices.single.$2, '/home/dev/fresh');
      expect(client.startWorkDirs, <String?>[null]);
      expect(client.startProjectIds, <String?>['fake-project-1']);
      expect(find.byType(ShellScreen), findsOneWidget);
    },
  );

  testWidgets(
    'cleanup shows the plan of the daemon with the line of what stays; '
    'cancel sends nothing, delete sends the digest of the plan shown and '
    'the list is read again',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client()
        ..projectBytes = <String, int>{'p-web': 412000000};
      await _pumpStart(tester, client);

      await tester.tap(find.byKey(const Key('start-tab-cleanup')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-project-p-web')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-review')));
      await tester.pump();
      await tester.pump();

      expect(find.byKey(const Key('start-cleanup-confirm')), findsOneWidget);
      expect(find.byKey(const Key('start-cleanup-item-0')), findsOneWidget);
      expect(find.byKey(const Key('start-cleanup-item-1')), findsOneWidget);
      expect(find.text('412 MB'), findsOneWidget);
      expect(
        tester
            .widget<Text>(find.byKey(const Key('start-cleanup-not-deleted')))
            .data,
        FakeProjects.notDeletedLine,
      );
      expect(find.text('Delete 2 items, 412 MB'), findsOneWidget);

      await tester.tap(find.byKey(const Key('start-cleanup-cancel')));
      await tester.pump();
      expect(find.byKey(const Key('start-cleanup-confirm')), findsNothing);
      expect(client.cleanupRequests, isEmpty);
      expect(
        client.projectsCalls.where((String c) => c.startsWith('cleanup ')),
        isEmpty,
      );

      await tester.tap(find.byKey(const Key('start-cleanup-review')));
      await tester.pump();
      await tester.pump();
      final CleanupPlan shown = client.lastCleanupPlan!;
      await tester.tap(find.byKey(const Key('start-cleanup-delete')));
      await tester.pump();
      await tester.pump();

      expect(client.cleanupRequests, hasLength(1));
      final (CleanupSelection selection, String digest) =
          client.cleanupRequests.single;
      expect(digest, shown.digest);
      expect(digest, isNotEmpty);
      expect(selection, shown.selection);
      expect(find.byKey(const Key('start-cleanup-confirm')), findsNothing);
      expect(find.byKey(const Key('start-cleanup-done')), findsOneWidget);
      expect(
        find.byKey(const Key('start-diagnostic-SANDBOX_054')),
        findsNothing,
      );
      // Nach dem Löschen neu gelesen: das Projekt steht nicht mehr da.
      expect(client.projectsCalls.last, 'list');
      expect(
        find.byKey(const Key('start-cleanup-project-p-web')),
        findsNothing,
      );
    },
  );

  testWidgets(
    'the palette leads from the session view back to the start window; '
    'the session keeps running',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client(running: true);
      await pumpApp(
        tester,
        client: client,
        overrides: [nowProvider.overrideWith(() => FixedNow(_now))],
      );
      expect(find.byType(ShellScreen), findsOneWidget);

      await pressCtrl(tester, LogicalKeyboardKey.keyK);
      await tester.enterText(find.byKey(const Key('palette-input')), 'start');
      await tester.pump();
      await tester.tap(find.byKey(const Key('palette-start-window')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(find.byType(StartScreen), findsOneWidget);
      expect(find.byType(ShellScreen), findsNothing);
      expect(client.sandbox.state, SandboxState.running);
      expect(find.byKey(const Key('start-attach-p-api')), findsOneWidget);
    },
  );

  testWidgets(
    'the window attaches on the first running status while the daemon keeps '
    'the stream of the start open, and the stored folder starts, not the '
    'display path',
    (WidgetTester tester) async {
      // Der Daemon hält den Strom eines Starts für die ganze Sitzung offen
      // (`accompany`). Mutationsbeweis: erst nach dem Ende des Stroms
      // wechseln färbt den Test rot.
      final FakeDaemonClient client = _client()
        ..startStreamHold = Completer<void>()
        // Zwei Leerzeichen im Ordnernamen; `Projects` zeigt ihn gesäubert.
        ..projectFolders = <String, String>{'p-api': '/home/dev/my  api'};
      addTearDown(() => client.startStreamHold?.complete());
      await _pumpStart(tester, client);

      await tester.tap(find.byKey(const Key('start-start-p-api')));
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(find.byType(ShellScreen), findsOneWidget);
      expect(client.startProjectIds, <String?>['p-api']);
      expect(client.sandbox.workDirHost, '/home/dev/my  api');
    },
  );

  testWidgets(
    'a start that ends without a session and without a finding says so',
    (WidgetTester tester) async {
      final _SilentStart client = _SilentStart()..projects = _client().projects;
      await _pumpStart(tester, client);

      await tester.tap(find.byKey(const Key('start-start-p-api')));
      await tester.pump();
      await tester.pump();

      expect(find.byKey(const Key('start-ended-silently')), findsOneWidget);
      expect(find.byType(StartScreen), findsOneWidget);
    },
  );

  testWidgets('a start in the session view of a known project sends its id', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client()
      ..projectFolders = <String, String>{'p-web': '/home/dev/my  web'};
    await pumpApp(
      tester,
      client: client,
      startWindow: true,
      overrides: [inSessionView(path: '/home/dev/my web', projectId: 'p-web')],
    );
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(ShellScreen)),
    );
    await container.read(sandboxStatusProvider.notifier).start();
    await tester.pump();

    expect(client.startProjectIds, <String?>['p-web']);
    expect(client.startWorkDirs, <String?>[null]);
    expect(client.sandbox.workDirHost, '/home/dev/my  web');
  });

  testWidgets(
    'cancel and the scrim do nothing while a confirmed cleanup runs',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client()
        ..cleanupHold = Completer<void>();
      await _pumpStart(tester, client);
      await tester.tap(find.byKey(const Key('start-tab-cleanup')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-project-p-web')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-review')));
      await tester.pump();
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-delete')));
      await tester.pump();

      await tester.tap(
        find.byKey(const Key('start-cleanup-cancel')),
        warnIfMissed: false,
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(find.byKey(const Key('start-cleanup-confirm')), findsOneWidget);

      client.cleanupHold!.complete();
      await tester.pump();
      await tester.pump();
      expect(find.byKey(const Key('start-cleanup-confirm')), findsNothing);
      expect(find.byKey(const Key('start-cleanup-done')), findsOneWidget);
    },
  );

  testWidgets(
    'a stale plan is refused with SANDBOX_054, the question closes and the '
    'list is read again',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client();
      await _pumpStart(tester, client);
      await tester.tap(find.byKey(const Key('start-tab-cleanup')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-project-p-web')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('start-cleanup-review')));
      await tester.pump();
      await tester.pump();
      // Inzwischen hat sich der Plan geändert: Der Daemon kennt einen
      // neueren, und der gezeigte Digest gilt nicht mehr.
      await client.projectsCleanupPlan(
        const CleanupSelection(projectIds: <String>['p-web']),
      );
      final int listsBefore = client.projectsCalls
          .where((String c) => c == 'list')
          .length;

      await tester.tap(find.byKey(const Key('start-cleanup-delete')));
      await tester.pump();
      await tester.pump();

      expect(
        find.byKey(const Key('start-diagnostic-SANDBOX_054')),
        findsOneWidget,
      );
      expect(find.byKey(const Key('start-cleanup-confirm')), findsNothing);
      expect(
        client.projectsCalls.where((String c) => c == 'list').length,
        listsBefore + 1,
      );
      expect(
        find.byKey(const Key('start-cleanup-project-p-web')),
        findsOneWidget,
      );
    },
  );
  testWidgets(
    'against a daemon before 1.58 a recent project does not start and its '
    'display path is never sent; a picked folder does not start either, '
    'because the daemon cannot keep its choice',
    (WidgetTester tester) async {
      // Mutationsbeweis: der Rückfall auf `ProjectEntry.path` in
      // `StartLaunchNotifier.startRecent` färbt den Test rot.
      final FakeDaemonClient client = _old57();
      await pumpApp(
        tester,
        client: client,
        startWindow: true,
        overrides: [
          nowProvider.overrideWith(() => FixedNow(_now)),
          directoryChooserProvider.overrideWithValue(
            () async => '/home/dev/fresh',
          ),
        ],
      );
      await tester.pump();
      await tester.pump();

      final HButton button = tester.widget<HButton>(
        find.byKey(const Key('start-start-p-api')),
      );
      expect(button.onPressed, isNull);

      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(StartScreen)),
      );
      await container
          .read(startLaunchProvider.notifier)
          .startRecent(client.projects.first);
      await tester.pump();

      expect(client.startWorkDirs, isEmpty);
      expect(client.startProjectIds, isEmpty);
      expect(find.byKey(const Key('start-diagnostic-CLI_004')), findsOneWidget);

      await tester.tap(find.byKey(const Key('start-new-project')));
      await tester.pump();
      await tester.pump();
      await tester.pump();
      await tester.tap(find.byKey(const Key('first-start-start')));
      await tester.pump();
      await tester.pump();
      // Ohne `SaveChoice.path` (1.59) startet der Bildschirm nicht ohne die
      // Wahl, sondern sagt, wie der Daemon neu startet (HUM-538).
      expect(client.savedChoices, isEmpty);
      expect(client.startWorkDirs, isEmpty);
      expect(client.startProjectIds, isEmpty);
      expect(find.byKey(const Key('start-diagnostic-CLI_004')), findsOneWidget);
    },
  );

  testWidgets(
    'against a daemon before 1.58 the session view of a known project '
    'starts nothing and says how to restart the daemon',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _old57();
      await pumpApp(
        tester,
        client: client,
        startWindow: true,
        overrides: [
          inSessionView(path: '/home/dev/my web', projectId: 'p-web'),
        ],
      );
      final ProviderContainer container = ProviderScope.containerOf(
        tester.element(find.byType(ShellScreen)),
      );
      await container.read(sandboxStatusProvider.notifier).start();
      await tester.pump();

      expect(client.startWorkDirs, isEmpty);
      expect(client.startProjectIds, isEmpty);
      expect(
        container
            .read(sandboxStatusProvider)
            .value
            ?.diagnostics
            .map((Diagnostic d) => d.code),
        contains('CLI_004'),
      );
    },
  );
}

/// Ein Start, der ohne Sitzung und ohne Befund endet.
class _SilentStart extends FakeDaemonClient {
  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    WorkMode? workMode,
    SandboxMode? sandboxMode,
    String? backend,
    String? workDir,
    String? projectId,
  }) => const Stream<SandboxUpdate>.empty();
}

/// A fake as a real daemon of contract 1.57, without the capability of
/// the fake.
FakeDaemonClient _old57() => FakeDaemonClient(
  info: FakeDaemonClient.defaultInfo.copyWith(
    protoMinor: 57,
    capabilities: <String>['sandbox.run', 'proxy.h1'],
  ),
)..projects = _client().projects;
