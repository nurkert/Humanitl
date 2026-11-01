/// What the app does around an update of Humanitl itself (HUM-382).
///
/// The daemon decides everything that matters: whether a newer release
/// exists, whether its signed manifest holds, how this installation gets an
/// update, and for the app bundle, the archive and the AppImage it downloads,
/// checks and swaps the files itself (`Update` RPC, ADR-018). The app asks,
/// shows the answer, and does only what the daemon cannot do for it:
///
/// - for an apt or dnf package, start the fixed `pkexec` call the daemon
///   names, because pkexec needs the polkit agent of the person's session,
///   which a service does not have. The app runs it only when the daemon's
///   call equals one of the two constants below, word for word, and never
///   through a shell;
/// - after a successful update, let the old daemon retire through the
///   existing path (`restartUserDaemon`, which never cuts off an agent
///   session), open the new app and end this one.
///
/// Every command goes through a provider, so a widget test never runs
/// pkexec, relaunches or quits anything on the person's machine.
library;

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../tray/desktop_ports.dart';
import '../tray/providers/attention.dart';
import 'daemon_restart.dart';

export '../../core/ipc/update_status.dart';

/// The first minor of the contract whose daemon knows `Update`.
const int updateMinor = 30;

/// The one call the app starts for an apt package, if the daemon names
/// exactly this one.
const List<String> aptUpgradeCommand = <String>[
  'pkexec',
  'apt-get',
  'install',
  '--only-upgrade',
  '-y',
  'humanitl',
];

/// The one call the app starts for a dnf package, if the daemon names
/// exactly this one.
const List<String> dnfUpgradeCommand = <String>[
  'pkexec',
  'dnf',
  'upgrade',
  '-y',
  'humanitl',
];

/// How often the app asks the daemon for its update status. The daemon
/// itself asks GitHub at most once a day, and only with `update.check` on;
/// this is only how quickly the notice follows. Overridden in tests; null
/// asks once at start and never again.
final Provider<Duration?> updatePollIntervalProvider = Provider<Duration?>(
  (Ref ref) => const Duration(hours: 1),
);

/// The executable of this app, which a package update replaces in place.
/// Overridden in tests.
final Provider<String> updateOwnExecutableProvider = Provider<String>(
  (Ref ref) => Platform.resolvedExecutable,
);

/// Starts a program without a shell; detached when [detached] is true.
/// Answers null on success or the line that says why it failed.
typedef ProgramStarter = Future<String?> Function(
  List<String> command, {
  required bool detached,
});

/// The starter that opens the new app. Overridden in tests.
final Provider<ProgramStarter> updateRelaunchStarterProvider =
    Provider<ProgramStarter>((Ref ref) => startProgram);

/// The runner of the `pkexec` call for a package. Overridden in tests.
final Provider<CommandRunner> updatePackageRunnerProvider =
    Provider<CommandRunner>((Ref ref) => Process.run);

/// Ends this app after the new one was opened. Overridden in tests.
///
/// The same order as the tray's Quit: the desktop connections first, then
/// the window, because what follows `quit` does not return.
final Provider<Future<void> Function()> updateQuitProvider =
    Provider<Future<void> Function()>((Ref ref) {
      return () async {
        final DesktopPorts ports = ref.read(desktopPortsProvider);
        await ports.dispose();
        await ports.window.quit();
      };
    });

/// [ProgramStarter] on the real system.
Future<String?> startProgram(
  List<String> command, {
  required bool detached,
}) async {
  final String line = command.join(' ');
  try {
    if (detached) {
      await Process.start(
        command.first,
        command.sublist(1),
        mode: ProcessStartMode.detached,
      );
      return null;
    }
    final ProcessResult result = await Process.run(
      command.first,
      command.sublist(1),
    );
    return result.exitCode == 0 ? null : _failure(line, result);
  } on ProcessException catch (error) {
    return '$line could not be started: ${error.message}';
  }
}

String _failure(String line, ProcessResult result) {
  final Object? stderr = result.stderr;
  final String detail = stderr is String ? stderr.trim() : '';
  return detail.isEmpty
      ? '$line exited with ${result.exitCode}'
      : '$line exited with ${result.exitCode}: $detail';
}

/// The command that opens the app at [appPath], and whether it runs
/// detached: on macOS `open -n`, which returns at once; elsewhere the
/// program itself, detached so it outlives this one.
({List<String> command, bool detached}) relaunchCommand(
  String appPath, {
  bool? macOS,
}) => (macOS ?? defaultTargetPlatform == TargetPlatform.macOS)
    ? (command: <String>['/usr/bin/open', '-n', appPath], detached: false)
    : (command: <String>[appPath], detached: true);

/// The constant the app may run for [method], or null when there is none.
List<String>? packageCommandFor(UpdateMethod? method) => switch (method) {
  UpdateMethod.apt => aptUpgradeCommand,
  UpdateMethod.dnf => dnfUpgradeCommand,
  _ => null,
};

/// True for the ways on which the daemon installs itself (`Apply`).
bool appliedByDaemon(UpdateMethod? method) => switch (method) {
  UpdateMethod.appBundle ||
  UpdateMethod.archive ||
  UpdateMethod.appimage => true,
  _ => false,
};

/// How the steps after an update ended.
sealed class UpdateFinish {
  const UpdateFinish();
}

/// The new app was opened and this one asked to end.
final class UpdateRelaunched extends UpdateFinish {
  /// Creates the outcome.
  const UpdateRelaunched();
}

/// The new version is in place, but the daemon could not be restarted;
/// nothing was relaunched, so the line stays in this window (HUM-382,
/// review). [detail] names the command and what it wrote.
final class UpdateRestartFailed extends UpdateFinish {
  /// Creates the outcome.
  const UpdateRestartFailed(this.detail);

  /// The command that failed and what it wrote, untranslated.
  final String detail;
}

/// A step failed; [detail] names the command and what it wrote.
final class UpdateFinishFailed extends UpdateFinish {
  /// Creates the outcome.
  const UpdateFinishFailed(this.detail);

  /// The command that failed and what it wrote, untranslated.
  final String detail;
}

/// After the new version is in place at [appPath]: let the daemon retire if
/// it is idle, then open the new app and end this one.
///
/// The daemon restart is the existing one of the status bar: retire only
/// when no agent session holds the daemon. A session that holds it is no
/// failure: the new app finds an older daemon and offers the existing
/// restart button, and an AppImage's `daemon install --refresh` leaves the
/// copy alone while a session holds it. A restart that fails is shown here,
/// and nothing is relaunched.
Future<UpdateFinish> finishUpdate({
  required String appPath,
  required int daemonMinor,
  required String? cli,
  required CommandRunner restartRunner,
  required ProgramStarter relaunch,
  required Future<void> Function() quit,
  bool? macOS,
}) async {
  final DaemonRestart restarted = await restartUserDaemon(
    restartRunner,
    cli: cli,
    daemonMinor: daemonMinor,
    macOS: macOS,
    // Only a daemon before contract 1.19 is asked this way; one that knows
    // `Update` decides itself. Unknown counts as a session.
    askSession: () async => null,
  );
  if (restarted case DaemonRestartFailed(:final String detail)) {
    return UpdateRestartFailed(detail);
  }
  final ({List<String> command, bool detached}) open = relaunchCommand(
    appPath,
    macOS: macOS,
  );
  final String? failure = await relaunch(open.command, detached: open.detached);
  if (failure != null) {
    return UpdateFinishFailed(failure);
  }
  await quit();
  return const UpdateRelaunched();
}

/// Runs the package upgrade the daemon named for [status], or refuses.
///
/// Null on success; otherwise the line to show. Nothing runs unless the
/// daemon's call equals the constant for its method exactly.
Future<String?> runPackageUpgrade(
  UpdateStatus status,
  CommandRunner run,
) async {
  final List<String>? allowed = packageCommandFor(status.method);
  if (allowed == null || !listEquals(allowed, status.upgradeCommand)) {
    return 'the daemon named an upgrade command this app does not run: '
        '${status.upgradeCommand.join(' ')}';
  }
  final String line = allowed.join(' ');
  try {
    final ProcessResult result = await run(allowed.first, allowed.sublist(1));
    return result.exitCode == 0 ? null : _failure(line, result);
  } on ProcessException catch (error) {
    return '$line could not be started: ${error.message}';
  }
}
