/// How the status bar restarts a daemon of another version (HUM-284).
///
/// A package upgrade replaces `humanitld` and its units, but the user manager
/// of systemd keeps the old daemon running until someone runs
/// `systemctl --user daemon-reload` and restarts the service. The package
/// does that itself where it can (`packaging/deb/maintscript-users.sh`); where
/// it could not -- a session was running, or the user manager did not answer
/// -- the status bar offers the same steps as a button.
///
/// Whether a session holds the daemon is the daemon's decision, made
/// atomically: `humanitl daemon retire-if-idle` asks it to end only if no
/// start holds a claim and no sandbox is held, and from then on no start gets
/// one. Only then does `systemctl --user restart humanitld.service` start the
/// new program. A daemon older than contract 1.19 does not know the request;
/// for it the app asks `Sandbox(Status)` once more and restarts. That old
/// daemon reports `stopped` for the whole of a start, so a start that runs or
/// begins before the restart can be cut off: the window is the whole start,
/// and only that one upgrade has it. From 1.19 on there is no fallback: no
/// command line next to the app, or a daemon that answers `none` although
/// this window is connected to it, ends in a sentence and never in a restart.
///
/// Every command is fixed: no shell, no argument from anywhere else, and only
/// for the user who runs this app. Tests replace the runner; a widget test
/// must never restart a service on the person's machine.
library;

import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/fix_control.dart' show installServiceCandidate;

/// Runs one program with its arguments, without a shell.
typedef CommandRunner = Future<ProcessResult> Function(
  String executable,
  List<String> arguments,
);

/// The first minor of the contract whose daemon knows `retire_if_idle`.
///
/// Mirror of `RETIRE_MINOR` in `daemon/bin/humanitl/src/cmd/daemon/retire.rs`.
const int daemonRetireMinor = 19;

/// `systemctl --user daemon-reload`: without it systemd would start the old
/// unit again and warn that the unit file changed on disk.
const List<String> daemonReloadCommand = <String>[
  'systemctl',
  '--user',
  'daemon-reload',
];

/// The arguments of the command line that ask the daemon to retire.
const List<String> daemonRetireArguments = <String>[
  '--json',
  'daemon',
  'retire-if-idle',
];

/// `systemctl --user restart humanitld.service`, after the daemon retired.
const List<String> daemonRestartCommand = <String>[
  'systemctl',
  '--user',
  'restart',
  'humanitld.service',
];

/// The runner the status bar uses. Overridden in tests.
final Provider<CommandRunner> daemonRestartRunnerProvider =
    Provider<CommandRunner>((Ref ref) => Process.run);

/// Where the command line lies, next to this app; null when none was found.
/// Overridden in tests.
final Provider<String?> daemonCliProvider = Provider<String?>(
  (Ref ref) => installServiceCandidate().path,
);

/// Whether [status] says that an agent session holds the daemon.
///
/// Mirrors `AgentSession::of` in `daemon/bin/humanitl/src/cmd/daemon/
/// agent_session.rs`: only a stopped sandbox, or a plan that fails without a
/// held sandbox, is no session. Unknown -- no answer yet, or an error -- counts
/// as a running session: a restart then would end the work of an agent.
bool agentSessionHolds(AsyncValue<SandboxStatus> status) {
  final SandboxStatus? value = status.value;
  if (value == null || status.hasError) {
    return true;
  }
  return switch (value.state) {
    SandboxState.stopped => false,
    SandboxState.failed => value.sandboxId != null,
    SandboxState.starting ||
    SandboxState.running ||
    SandboxState.stopping => true,
  };
}

/// How a restart ended.
sealed class DaemonRestart {
  const DaemonRestart();
}

/// The daemon of the installed version is starting.
final class DaemonRestarted extends DaemonRestart {
  /// Creates the outcome.
  const DaemonRestarted();
}

/// An agent session holds the daemon, or nobody could tell; nothing was
/// restarted.
final class DaemonSessionHolds extends DaemonRestart {
  /// Creates the outcome.
  const DaemonSessionHolds();
}

/// A step failed; [detail] names the command and what it wrote.
final class DaemonRestartFailed extends DaemonRestart {
  /// Creates the outcome.
  const DaemonRestartFailed(this.detail);

  /// The command that failed and what it wrote, untranslated.
  final String detail;
}

/// Runs the restart for a daemon that speaks contract minor [daemonMinor].
///
/// [cli] is the command line next to this app. [askSession] asks
/// `Sandbox(Status)` afresh for the old path and answers true, false or null
/// (unknown, counted as a session).
Future<DaemonRestart> restartUserDaemon(
  CommandRunner run, {
  required String? cli,
  required int daemonMinor,
  required Future<bool?> Function() askSession,
  bool? macOS,
}) async {
  // On macOS the daemon is a launchd agent (HUM-315): no unit to reload, and
  // the restart is `humanitl daemon restart` (`launchctl kickstart -k`), which
  // also starts an agent that retired with exit 0. The decision, retire only
  // when idle, is the same on both systems.
  final bool mac = macOS ?? defaultTargetPlatform == TargetPlatform.macOS;
  if (mac && cli == null) {
    return const DaemonRestartFailed(
      'no humanitl command line lies next to this app; run humanitl daemon '
      'retire-if-idle, then humanitl daemon restart',
    );
  }
  final List<String> restart = mac
      ? <String>[cli!, 'daemon', 'restart']
      : daemonRestartCommand;
  if (!mac) {
    final _Ran reload = await _run(run, daemonReloadCommand);
    if (reload.failure case final String failure) {
      return DaemonRestartFailed(failure);
    }
  }
  if (daemonMinor >= daemonRetireMinor) {
    if (cli == null) {
      return const DaemonRestartFailed(
        'no humanitl command line lies next to this app; run '
        'humanitl daemon retire-if-idle, then systemctl --user restart '
        'humanitld.service',
      );
    }
    final _Ran retire = await _run(run, <String>[
      cli,
      ...daemonRetireArguments,
    ]);
    if (retire.failure case final String failure) {
      return DaemonRestartFailed(failure);
    }
    switch (_outcome(retire.stdout)) {
      case 'retired':
        return _restart(run, restart);
      case 'none':
        // This window is connected to a daemon, and the command line found
        // none: they do not look at the same daemon. Restarting would be a
        // guess.
        return const DaemonRestartFailed(
          'humanitl daemon retire-if-idle found no running daemon, although '
          'this window is connected to one',
        );
      case 'busy':
        return const DaemonSessionHolds();
      case 'unsupported':
        break;
      default:
        // No readable answer: as unknown as a running session.
        return const DaemonSessionHolds();
    }
  }
  if (await askSession() != false) {
    return const DaemonSessionHolds();
  }
  return _restart(run, restart);
}

/// [command]: `systemctl --user restart humanitld.service`, or on macOS
/// `humanitl daemon restart`.
Future<DaemonRestart> _restart(CommandRunner run, List<String> command) async {
  final _Ran restart = await _run(run, command);
  return switch (restart.failure) {
    final String failure => DaemonRestartFailed(failure),
    null => const DaemonRestarted(),
  };
}

/// The field `outcome` of the one JSON line `retire-if-idle` writes.
String? _outcome(String stdout) {
  try {
    final Object? decoded = jsonDecode(stdout.trim());
    if (decoded is Map<String, Object?>) {
      final Object? outcome = decoded['outcome'];
      return outcome is String ? outcome : null;
    }
  } on FormatException {
    return null;
  }
  return null;
}

/// What one command did: its stdout, or the line that says why it failed.
typedef _Ran = ({String stdout, String? failure});

Future<_Ran> _run(CommandRunner run, List<String> command) async {
  final String line = command.join(' ');
  try {
    final ProcessResult result = await run(command.first, command.sublist(1));
    final Object? stdout = result.stdout;
    final String out = stdout is String ? stdout : '';
    if (result.exitCode == 0) {
      return (stdout: out, failure: null);
    }
    final Object? stderr = result.stderr;
    final String detail = stderr is String ? stderr.trim() : '';
    return (
      stdout: out,
      failure: detail.isEmpty
          ? '$line exited with ${result.exitCode}'
          : '$line exited with ${result.exitCode}: $detail',
    );
  } on ProcessException catch (error) {
    return (
      stdout: '',
      failure: '$line could not be started: ${error.message}',
    );
  }
}
