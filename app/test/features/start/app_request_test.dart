// `humanitl app [PATH]` in der App (HUM-539): `--project` beim Start und
// `FlowEvent.open_in_app` an der laufenden App wählen den Ordner im
// Startfenster vor. Ein bekanntes Projekt steht markiert unter „Zuletzt",
// ein unbekannter Ordner bekommt den Erststart-Bildschirm, ein Pfad, der
// kein Ordner ist, steht als Befund des Daemons da. Nichts davon startet.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/start/start_screen.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';
import '../tray/fixtures.dart';

final DateTime _now = DateTime.utc(2026, 11, 21, 12);

/// Ein Fake mit einem bekannten Projekt `api`, dessen Strom auch die Bitten
/// von `humanitl app` trägt, die der Test schickt.
class _Client extends FakeDaemonClient {
  _Client() {
    projects = <ProjectEntry>[
      ProjectEntry(
        projectId: 'p-api',
        path: '/home/dev/api',
        displayName: 'api',
        lastStarted: _now.subtract(const Duration(hours: 2)),
        lastAgent: 'claude',
        runs: 3,
        hasChoice: true,
      ),
    ];
  }

  final StreamController<FlowEvent> _requests =
      StreamController<FlowEvent>.broadcast();

  /// Was der Daemon auf `Projects.open_in_app` an die App gibt.
  void openInApp(String path) =>
      _requests.add(FlowEvent.openInApp(at: _now, path: path));

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) {
    final StreamController<FlowEvent> out = StreamController<FlowEvent>();
    final List<StreamSubscription<FlowEvent>> sources =
        <StreamSubscription<FlowEvent>>[];
    out
      ..onListen = () {
        sources
          ..add(
            super
                .subscribe(since: since, includePassthrough: includePassthrough)
                .listen(out.add, onError: out.addError),
          )
          ..add(_requests.stream.listen(out.add));
      }
      ..onCancel = () async {
        for (final StreamSubscription<FlowEvent> source in sources) {
          await source.cancel();
        }
      };
    return out.stream;
  }
}

Future<void> _settle(WidgetTester tester) async {
  for (int i = 0; i < 6; i++) {
    await tester.pump();
  }
}

Future<void> _pump(
  WidgetTester tester,
  _Client client, {
  String? project,
  bool startWindow = true,
  FakeDesktop? desktop,
}) async {
  await pumpApp(
    tester,
    client: client,
    startWindow: startWindow,
    overrides: [
      nowProvider.overrideWith(() => FixedNow(_now)),
      launchOptionsProvider.overrideWithValue(
        LaunchOptions(projectPath: project),
      ),
      if (desktop != null)
        desktopPortsProvider.overrideWithValue(desktop.ports),
    ],
  );
  await _settle(tester);
}

/// Nichts wurde gestartet und keine Wahl gespeichert.
void _nothingStarted(_Client client) {
  expect(client.startProjectIds, isEmpty);
  expect(client.startWorkDirs, isEmpty);
  expect(client.savedChoices, isEmpty);
}

void main() {
  testWidgets('--project on a known folder marks it under Recent and starts '
      'nothing', (WidgetTester tester) async {
    // Mutationsbeweis: `preselected` in `_ProjectRow` fest auf false gesetzt
    // färbt den Test rot.
    final _Client client = _Client();
    await _pump(tester, client, project: '/home/dev/api');

    expect(find.byType(StartScreen), findsOneWidget);
    expect(client.proposalCalls.single.$1, '/home/dev/api');
    expect(
      find.descendant(
        of: find.byKey(const Key('start-recent')),
        matching: find.byKey(const Key('start-preselected-p-api')),
      ),
      findsOneWidget,
    );
    expect(find.byKey(const Key('first-start')), findsNothing);
    _nothingStarted(client);
  });

  testWidgets('--project on an unknown folder opens the first-start screen '
      'and starts nothing before Start', (WidgetTester tester) async {
    final _Client client = _Client();
    await _pump(tester, client, project: '/home/dev/fresh');

    expect(find.byKey(const Key('first-start')), findsOneWidget);
    expect(find.text('/home/dev/fresh'), findsWidgets);
    _nothingStarted(client);
  });

  testWidgets('--project on a file stands as the daemon\'s finding in the '
      'start window', (WidgetTester tester) async {
    final _Client client = _Client()
      ..proposalFailure = const Diagnostic(
        code: 'CONFIG_003',
        severity: Severity.error,
        why:
            '/home/dev/notes.txt is no folder a project can start in '
            '(Not a directory (os error 20))',
      );
    await _pump(tester, client, project: '/home/dev/notes.txt');

    expect(
      find.byKey(const Key('start-diagnostic-CONFIG_003')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('first-start')), findsNothing);
    expect(find.byKey(const Key('start-preselected-p-api')), findsNothing);
    _nothingStarted(client);
  });

  testWidgets('without --project the start window has no preselection', (
    WidgetTester tester,
  ) async {
    final _Client client = _Client();
    await _pump(tester, client);

    expect(find.byType(StartScreen), findsOneWidget);
    expect(client.proposalCalls, isEmpty);
    expect(find.byKey(const Key('start-preselected-p-api')), findsNothing);
  });

  testWidgets('a request to the running app leaves the session view for the '
      'start window, preselects, brings the window forward and starts '
      'nothing', (WidgetTester tester) async {
    // Mutationsbeweis: `_ask` ohne `detach()` lässt die Sitzungsansicht
    // stehen und färbt den Test rot.
    final _Client client = _Client();
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, client, startWindow: false, desktop: desktop);
    expect(find.byType(ShellScreen), findsOneWidget);

    client.openInApp('/home/dev/api');
    await _settle(tester);

    expect(find.byType(StartScreen), findsOneWidget);
    expect(find.byKey(const Key('start-preselected-p-api')), findsOneWidget);
    expect(desktop.window.reveals, 1);
    expect(desktop.notifications.announcements, isEmpty);
    _nothingStarted(client);
  });

  testWidgets('where the window cannot come forward by itself (Wayland), a '
      'notification says the start window waits', (WidgetTester tester) async {
    final _Client client = _Client();
    final FakeDesktop desktop = FakeDesktop();
    desktop.window.raisesItself = false;
    await _pump(tester, client, desktop: desktop);

    client.openInApp('/home/dev/fresh');
    await _settle(tester);

    expect(find.byKey(const Key('first-start')), findsOneWidget);
    expect(desktop.notifications.announcements, hasLength(1));
    expect(desktop.notifications.announcements.single.$2, contains('fresh'));
    _nothingStarted(client);
  });

  testWidgets('a request without a folder clears the preselection', (
    WidgetTester tester,
  ) async {
    final _Client client = _Client();
    await _pump(tester, client, project: '/home/dev/api');
    expect(find.byKey(const Key('start-preselected-p-api')), findsOneWidget);

    client.openInApp('');
    await _settle(tester);

    expect(find.byKey(const Key('start-preselected-p-api')), findsNothing);
    _nothingStarted(client);
  });

  testWidgets('a new request closes the first-start screen of the last one: '
      'for a known folder and for none', (WidgetTester tester) async {
    // Mutationsbeweis: `cancel()` in `PreselectionNotifier._open` entfernt
    // lässt den alten Bildschirm stehen und färbt den Test rot.
    final _Client client = _Client();
    await _pump(tester, client, project: '/home/dev/fresh');
    expect(find.byKey(const Key('first-start')), findsOneWidget);

    client.openInApp('/home/dev/api');
    await _settle(tester);
    expect(find.byKey(const Key('first-start')), findsNothing);
    expect(find.byKey(const Key('start-preselected-p-api')), findsOneWidget);

    client.openInApp('/home/dev/other');
    await _settle(tester);
    expect(find.byKey(const Key('first-start')), findsOneWidget);
    client.openInApp('');
    await _settle(tester);
    expect(find.byKey(const Key('first-start')), findsNothing);
    _nothingStarted(client);
  });

  testWidgets('a request for the folder of the open first-start screen keeps '
      'it with its inputs', (WidgetTester tester) async {
    // Mutationsbeweis: der Zweig für denselben Ordner entfernt baut den
    // Bildschirm neu, „Anpassen…" klappt zu, und der Test wird rot.
    final _Client client = _Client();
    await _pump(tester, client, project: '/home/dev/fresh');
    await tester.tap(find.byKey(const Key('first-start-customize')));
    await _settle(tester);
    expect(find.byKey(const Key('first-start-field-agent')), findsOneWidget);
    final int asked = client.proposalCalls.length;

    client.openInApp('/home/dev/fresh');
    await _settle(tester);

    expect(find.byKey(const Key('first-start-field-agent')), findsOneWidget);
    expect(client.proposalCalls, hasLength(asked));
    _nothingStarted(client);
  });

  testWidgets('the folder in the notification is plain text, never markup', (
    WidgetTester tester,
  ) async {
    // Mutationsbeweis: `plainNotificationText` im `TrayHost` weggelassen
    // färbt den Test rot.
    final _Client client = _Client();
    final FakeDesktop desktop = FakeDesktop();
    desktop.window.raisesItself = false;
    await _pump(tester, client, desktop: desktop);

    client.openInApp('/home/dev/<b>x</b> & co');
    await _settle(tester);

    final String body = desktop.notifications.announcements.single.$2;
    expect(body, contains('/home/dev/&lt;b&gt;x&lt;/b&gt; &amp; co'));
    expect(body, isNot(contains('<b>')));
  });
}
