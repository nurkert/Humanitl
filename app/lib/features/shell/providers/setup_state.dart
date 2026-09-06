/// The four checks of the setup screen, composed out of three features
/// (HUM-044).
///
/// It lives in the shell because the shell is the frame that hangs the
/// features into each other, and it is the only place allowed to do so: the
/// sandbox snapshot belongs to `features/sandbox` and the doctor to
/// `features/setup`, and no feature may import another one (ARCHITECTURE 5,
/// `tools/check-deps.sh`). The connection is no longer one of the three -- it
/// moved to `core/ipc/connection.dart` with HUM-044 and either feature could
/// read it -- but collapsing its four states into the one question the daemon
/// row asks is still the shell's job, and it is done here. The same
/// arrangement carries a flow from the history into the queue
/// (`ShellScreen._takeHandoff`).
///
/// Nothing is judged here either. [setupChecks] folds what the daemon said
/// into four rows, and this file only says which providers it reads.
library;

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../../setup/providers/setup_provider.dart';
import 'connection.dart';

part 'setup_state.g.dart';

/// The four rows of the setup screen, as they stand right now.
@Riverpod(keepAlive: true)
SetupState setupState(Ref ref) => setupChecks(
  daemon: daemonLinkOf(ref.watch(connectionStateProvider)),
  doctor: ref.watch(setupDoctorProvider),
  sandbox: ref.watch(sandboxStatusProvider),
  sandboxCall: ref.watch(sandboxCallProvider),
  llm: ref.watch(setupLlmProbeProvider),
);

/// The connection state in the vocabulary of the setup feature.
///
/// Der laufende Versuch reist mit. Er ist im Zustand ein Feld des
/// Fehlschlags und nicht [ConnectionConnecting], weil er nicht entscheidet,
/// welcher Bildschirm gezeigt wird, sondern nur, was die erste Zeile darauf
/// sagt und ob ihr Knopf noch drückbar ist (`core/ipc/connection.dart`,
/// `_breakOrElse`). Hier bleibt genau diese Trennung erhalten: Der Grund geht
/// weiter an die Zeile, und daneben steht, dass gerade jemand nachfragt.
DaemonLink daemonLinkOf(ConnectionStatus status) => switch (status) {
  ConnectionConnecting() => const DaemonLinkConnecting(),
  ConnectionConnected(:final DaemonInfo info) => DaemonLinkUp(info),
  // Beide Fehlschläge sind für die Zeile dasselbe: Der Dienst antwortet nicht.
  // Was sie unterscheidet, entscheidet die Shell und nicht die Zeile -- ob der
  // Bildschirm der Setup-Bildschirm ist oder die Shell mit einem Banner
  // (`docs/UX.md` 4.2, Fälle 2 und 4).
  ConnectionFailed(:final Diagnostic diagnostic, :final bool retrying) ||
  ConnectionFrozen(
    :final Diagnostic diagnostic,
    :final bool retrying,
  ) => DaemonLinkDown(diagnostic, retrying: retrying),
};
