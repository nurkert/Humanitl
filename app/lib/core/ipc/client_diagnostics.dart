/// Diagnostics the client raises on its own, before or instead of an answer
/// from the daemon. Titles stay empty here: the setup screen localises them
/// by code (`setupDaemonMissingTitle` and friends); `why` carries the
/// technical detail in the language of the transport.
library;

import 'package:flutter/foundation.dart';

import '../domain/domain.dart';
import '../ui/shell_command.dart';
import 'private_path.dart';
import 'proto_version.dart';

/// Factories for the client-side diagnostics of HUM-019.
abstract final class ClientDiagnostics {
  /// Command that starts the real daemon, offered as the fix for
  /// [daemonUnreachable] whenever this app was pointed at a daemon by hand.
  static const String startDaemonCommand = 'humanitld';

  /// Command that starts the installed user unit.
  ///
  /// It stands in `why` and never as a second action: `Diagnostic.fix` is one
  /// `FixAction`, the line carries `InstallService`, and the person who would
  /// rather type it reads it in the cause (HUM-044, diagnostic table).
  ///
  /// On macOS the daemon is a launchd agent, and `humanitl daemon install`
  /// loads it (HUM-325). The command line lies in the app there and not on
  /// `PATH`, so the line names its full path (HUM-372).
  static String get startUnitCommand =>
      defaultTargetPlatform == TargetPlatform.macOS
      ? '$macCommandLinePath daemon install'
      : 'systemctl --user start humanitld';

  /// Command that restarts the installed user unit, offered as the fix for
  /// [tokenRejected] (HUM-058, row 4).
  ///
  /// A daemon writes a fresh token into the runtime directory every time it
  /// starts, so a token that does not match belongs to another daemon or to
  /// an earlier run. A restart of the one the app talks to writes the pair
  /// anew; the next attempt of the connection reads it.
  ///
  /// On macOS `humanitl daemon restart` restarts the launchd agent (HUM-325),
  /// by the full path for the same reason as [startUnitCommand] (HUM-372).
  static String get restartUnitCommand =>
      defaultTargetPlatform == TargetPlatform.macOS
      ? '$macCommandLinePath daemon restart'
      : 'systemctl --user restart humanitld';

  /// Where a matching pair of app and daemon is downloaded, offered as the
  /// fix for [protoIncompatible].
  ///
  /// `FixControl` renders `OpenUrl` as a copy button today; opening a browser
  /// would mean adding `url_launcher` as a dependency, which HUM-044 leaves
  /// open.
  static const String releasesUrl =
      'https://github.com/nurkert/Humanitl/releases';

  /// Command that starts the fake daemon with the bundled session.
  static const String startFakeCommand =
      'humanitld --fake fixtures/sessions/mixed.jsonl';

  /// `DAEMON_001`: nothing answers on [socketPath].
  ///
  /// [fake] switches the proposed command to the fake daemon; [socketFlag]
  /// appends `--socket PATH` when the app was pointed at a custom socket.
  ///
  /// The fix depends on which daemon was missed. On the ordinary path -- the
  /// user unit on the default socket -- it is `InstallService`, the action
  /// HUM-044 puts in the table: the app writes and starts the unit itself, and
  /// the two-second retry of the connection turns the row green. Was this app
  /// pointed at a daemon by hand, with `--fake` or `--socket`, then installing
  /// the unit would start a different daemon on a different socket and answer
  /// a question nobody asked; there the fix stays the command that starts the
  /// daemon that was meant.
  static Diagnostic daemonUnreachable({
    required String socketPath,
    String? detail,
    bool fake = false,
    bool socketFlag = false,
  }) {
    final bool byHand = fake || socketFlag;
    final StringBuffer command = StringBuffer(
      fake ? startFakeCommand : startDaemonCommand,
    );
    if (socketFlag) {
      // Ein Pfad mit Leerzeichen oder `'` bleibt ein Wort (HUM-215).
      command.write(' --socket ${shellQuote(socketPath)}');
    }
    final StringBuffer why = StringBuffer('no daemon answers on $socketPath');
    if (detail != null && detail.isNotEmpty) {
      why.write(': $detail');
    }
    if (!byHand) {
      why.write('; $startUnitCommand starts it');
    }
    return Diagnostic(
      code: DiagnosticCodes.daemonUnreachable,
      severity: Severity.error,
      why: why.toString(),
      fix: byHand
          ? FixAction.copyCommand(command: command.toString())
          : const FixAction.installService(),
    );
  }

  /// Section behind [runtimeUntrusted] in the fallback to the temporary
  /// directory, mirror of
  /// `humanitl_config::private_dir::OWN_RUNTIME_DIR_DOC_URL`.
  ///
  /// It explains how to set `XDG_RUNTIME_DIR` to a directory of one's own for
  /// the whole session, for bash and for zsh. A link and not a command: which
  /// login file a session reads depends on shell and display manager, and a
  /// wrong command in such a file can break the session.
  static const String ownRuntimeDirDocUrl =
      'https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#xdg_runtime_dir-ohne-logind';

  /// The sentence a [runtimeUntrusted] in the fallback carries in `why`,
  /// mirror of `humanitl_config::private_dir::OWN_RUNTIME_DIR_HINT`.
  static const String ownRuntimeDirHint =
      'daemon, CLI and app must all see the same XDG_RUNTIME_DIR, set for the whole session (see the linked section for bash and zsh), and it takes effect after logging in again; HUM-222 will let the clients find the directory themselves';

  /// Section behind [runtimeUntrusted] in the fallback on macOS (HUM-372),
  /// mirror of the macOS value of
  /// `humanitl_config::private_dir::OWN_RUNTIME_DIR_DOC_URL`.
  static const String macRuntimeDirDocUrl =
      'https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#runtime-directory-on-macos';

  /// The macOS counterpart of [ownRuntimeDirHint], mirror of the macOS value
  /// of `humanitl_config::private_dir::OWN_RUNTIME_DIR_HINT`.
  ///
  /// macOS has no session-wide `XDG_RUNTIME_DIR`. The fallback exists there
  /// because the path under `Application Support` is too long for a socket.
  static const String macRuntimeDirHint =
      'the path under ~/Library/Application Support is too long for a socket, so daemon, CLI and app use the temporary directory; the linked section says what to do about it';

  /// `DAEMON_001`: the runtime directory or the token is a symlink, belongs to
  /// another account or is open to group and others (HUM-212). The token
  /// there is not read.
  ///
  /// Only in the fallback to the temporary directory ([fallback]) does a
  /// directory of one's own help; in a session with `/run/user/<uid>` another
  /// `XDG_RUNTIME_DIR` would break Wayland, PipeWire and D-Bus. There the fix
  /// is `chmod` for open permissions and none for a foreign owner or a
  /// symlink, which this account cannot repair.
  static Diagnostic runtimeUntrusted(
    PrivatePathProblem problem, {
    required bool fallback,
  }) {
    if (fallback) {
      final bool mac = defaultTargetPlatform == TargetPlatform.macOS;
      return Diagnostic(
        code: DiagnosticCodes.daemonUnreachable,
        severity: Severity.error,
        why: '${problem.why}; ${mac ? macRuntimeDirHint : ownRuntimeDirHint}',
        fix: FixAction.openUrl(
          url: mac ? macRuntimeDirDocUrl : ownRuntimeDirDocUrl,
        ),
      );
    }
    return Diagnostic(
      code: DiagnosticCodes.daemonUnreachable,
      severity: Severity.error,
      why: problem.why,
      // A line break in the path would not survive the copy from the
      // diagnostic: then there is no command, as in [exportRefusal] and on
      // the daemon side.
      fix: problem.open && !problem.path.contains(RegExp('[\r\n]'))
          ? FixAction.copyCommand(
              command: 'chmod go-rwx ${shellQuote(problem.path)}',
            )
          : null,
    );
  }

  /// `IPC_001`: the daemon rejected the token from [tokenPath].
  ///
  /// The fix is [restartUnitCommand] on the ordinary path. Was this app
  /// pointed at a daemon by hand ([byHand]: `--fake` or `--socket`), the unit
  /// is not the daemon that refused, and restarting it would answer a question
  /// nobody asked; there the finding carries no fix, the same split
  /// [daemonUnreachable] makes.
  static Diagnostic tokenRejected({
    required String tokenPath,
    String? detail,
    bool byHand = false,
  }) {
    return withTokenFix(
      Diagnostic(
        code: DiagnosticCodes.tokenInvalid,
        severity: Severity.error,
        why: detail == null || detail.isEmpty
            ? 'the daemon rejected the token from $tokenPath'
            : 'the daemon rejected the token from $tokenPath: $detail',
      ),
      byHand: byHand,
    );
  }

  /// [diagnostic] with [restartUnitCommand] as its fix when it is an
  /// `IPC_001` without one, otherwise unchanged.
  ///
  /// The daemon ships its own `IPC_001` in the status details, and that one
  /// wins over the client's sentence (`diagnosticFromGrpcError`); it carries
  /// no fix, because the daemon cannot know how it was started. The client
  /// does: on the ordinary path it talks to the user unit. A fix the daemon
  /// did send stays as it is.
  static Diagnostic withTokenFix(Diagnostic diagnostic, {bool byHand = false}) {
    if (diagnostic.code != DiagnosticCodes.tokenInvalid ||
        diagnostic.fix != null ||
        byHand) {
      return diagnostic;
    }
    return diagnostic.copyWith(
      fix: FixAction.copyCommand(command: restartUnitCommand),
    );
  }

  /// `DAEMON_002`: the daemon speaks another major of the contract.
  ///
  /// Both halves come from the same release, so the fix is the page that has
  /// both: [releasesUrl] as `OpenUrl`. A diagnostic that carries no action at
  /// all leaves the person with a version number and nowhere to go
  /// (`docs/UX.md` 4.4).
  static Diagnostic protoIncompatible(DaemonInfo info) {
    return Diagnostic(
      code: DiagnosticCodes.protoIncompatible,
      severity: Severity.blocking,
      why:
          'daemon ${info.daemonVersion} speaks proto ${info.protoVersion}, '
          'this app speaks ${ProtoVersion.text}',
      fix: const FixAction.openUrl(url: releasesUrl),
    );
  }

  /// `CONFIG_013`: nobody has chosen a project folder yet.
  ///
  /// Built here and not by the daemon, for the same reason as
  /// [daemonUnreachable]: the daemon answers `Sandbox(Status)` with an empty
  /// `work_dir_host` and raises nothing, because an open step of the setup is
  /// not a fault of the daemon. The screen is the place that knows the step is
  /// still open, and the register carries the code so both sides name it the
  /// same (`daemon/crates/core-types/src/diagnostics/codes.rs`).
  ///
  /// The fix is a `ChangeSetting` on `sandbox.work_dir`, the same one the
  /// daemon puts on a refused folder (`SANDBOX_006`). `FixControl` turns it
  /// into a button that opens the folder dialog of the row above, so the card
  /// offers the way out itself instead of pointing at a control somebody has
  /// to find first (HUM-282, which replaces the earlier rule of HUM-044 that
  /// the card carried no fix).
  static Diagnostic noProjectFolder() {
    return const Diagnostic(
      code: DiagnosticCodes.noProjectFolder,
      severity: Severity.blocking,
      why:
          'No project folder chosen. The agent needs exactly one folder to '
          'work in.',
      fix: FixAction.changeSetting(key: FormKeys.sandboxWorkDir, value: ''),
    );
  }

  /// Command that starts a session in the current directory, offered as the
  /// fix for [folderDialogFailed]: the way to a project folder that needs no
  /// dialog.
  static String get runInFolderCommand => '${commandLineProgram()} run';

  /// `UI_003`: the desktop's folder dialog did not open (HUM-282).
  ///
  /// [error] is what the platform threw, as text; it stands in `why` so the
  /// person can report it. A cancelled dialog never gets here: it answers
  /// null, and that is an answer, not a failure.
  static Diagnostic folderDialogFailed(Object error) {
    return Diagnostic(
      code: DiagnosticCodes.folderDialogFailed,
      severity: Severity.error,
      why:
          'The desktop could not show a folder dialog ($error). Open a '
          'terminal in the project folder and run `$runInFolderCommand` '
          'there.',
      fix: FixAction.copyCommand(command: runInFolderCommand),
    );
  }
}
