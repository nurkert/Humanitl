/// Diagnostics the client raises on its own, before or instead of an answer
/// from the daemon. Titles stay empty here: the setup screen localises them
/// by code (`setupDaemonMissingTitle` and friends); `why` carries the
/// technical detail in the language of the transport.
library;

import '../domain/domain.dart';
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
  static const String startUnitCommand = 'systemctl --user start humanitld';

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
      command.write(' --socket $socketPath');
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

  /// `IPC_001`: the daemon rejected the token from [tokenPath].
  static Diagnostic tokenRejected({required String tokenPath, String? detail}) {
    return Diagnostic(
      code: DiagnosticCodes.tokenInvalid,
      severity: Severity.error,
      why: detail == null || detail.isEmpty
          ? 'the daemon rejected the token from $tokenPath'
          : 'the daemon rejected the token from $tokenPath: $detail',
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
  /// Without `fix`: the row itself carries the folder button, and a second
  /// button beside it would lead somewhere else (HUM-044, diagnostic table).
  static Diagnostic noProjectFolder() {
    return const Diagnostic(
      code: DiagnosticCodes.noProjectFolder,
      severity: Severity.blocking,
      why:
          'No project folder chosen. The agent needs exactly one folder to '
          'work in.',
    );
  }
}
