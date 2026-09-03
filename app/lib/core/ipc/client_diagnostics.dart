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
  /// [daemonUnreachable].
  static const String startDaemonCommand = 'humanitld';

  /// Command that starts the fake daemon with the bundled session.
  static const String startFakeCommand =
      'humanitld --fake fixtures/sessions/mixed.jsonl';

  /// `DAEMON_001`: nothing answers on [socketPath].
  ///
  /// [fake] switches the proposed command to the fake daemon; [socketFlag]
  /// appends `--socket PATH` when the app was pointed at a custom socket.
  static Diagnostic daemonUnreachable({
    required String socketPath,
    String? detail,
    bool fake = false,
    bool socketFlag = false,
  }) {
    final StringBuffer command = StringBuffer(
      fake ? startFakeCommand : startDaemonCommand,
    );
    if (socketFlag) {
      command.write(' --socket $socketPath');
    }
    return Diagnostic(
      code: DiagnosticCodes.daemonUnreachable,
      severity: Severity.error,
      why: detail == null || detail.isEmpty
          ? 'no daemon answers on $socketPath'
          : 'no daemon answers on $socketPath: $detail',
      fix: FixAction.copyCommand(command: command.toString()),
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
  static Diagnostic protoIncompatible(DaemonInfo info) {
    return Diagnostic(
      code: DiagnosticCodes.protoIncompatible,
      severity: Severity.blocking,
      why:
          'daemon ${info.daemonVersion} speaks proto ${info.protoVersion}, '
          'this app speaks ${ProtoVersion.text}',
    );
  }
}
