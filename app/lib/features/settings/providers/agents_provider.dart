/// The state of the agent depot as `Agents` answers it (HUM-521).
///
/// Every operation answers with the state afterwards, and that answer is the
/// new state of the section; nothing is computed here. While a maintenance
/// run goes on, the state is read again every [agentsPollInterval], so the
/// URLs of a login, the end of a run and the changes of a configuration run
/// appear without a click.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import 'file_opener.dart';

/// How often the state is read while a maintenance run goes on.
const Duration agentsPollInterval = Duration(milliseconds: 1500);

/// Opens a login URL on this machine; true when the desktop took it.
typedef LoginUrlOpener = Future<bool> Function(String url);

/// The opener of "Open in browser". Overridden in tests: a widget test must
/// never start a program on the person's desktop.
final Provider<LoginUrlOpener> loginUrlOpenerProvider =
    Provider<LoginUrlOpener>((Ref ref) => openWithDesktop);

/// The state of every known agent, and the way to change it.
///
/// No retry of its own, as with the configuration (`noConfigRetry`): a
/// refusal of the daemon stands at the section with "Try again", and the
/// state is read anew whenever the configuration is.
final AsyncNotifierProvider<AgentsController, AgentsStatus> agentsProvider =
    AsyncNotifierProvider.autoDispose<AgentsController, AgentsStatus>(
      AgentsController.new,
      retry: noConfigRetry,
    );

/// How many refreshes in a row may fail during a run before the section
/// says the daemon does not answer.
const int agentsPollFailureCap = 5;

/// Reads `Agents(status)`, sends operations, and keeps the latest answer.
class AgentsController extends AsyncNotifier<AgentsStatus> {
  Timer? _poll;
  int _failures = 0;

  @override
  Future<AgentsStatus> build() async {
    // Ein geschriebener Schlüssel `agents.*` ändert Quelle oder Rückschrift;
    // der Stand danach kommt vom Daemon.
    ref.watch(configProvider);
    ref.onDispose(() => _poll?.cancel());
    _failures = 0;
    final AgentsStatus status = await ref
        .read(daemonClientProvider)
        .agents(AgentsOp.status);
    _schedule(status);
    return status;
  }

  /// Sends [op]; the answer becomes the state. A refusal of the daemon comes
  /// back as its finding and leaves the state as it was.
  ///
  /// `config_show` answers with the files of one agent; the caller keeps
  /// them, and the state takes the answer like any other.
  Future<(AgentsStatus?, Diagnostic?)> run(AgentsOp op) async {
    try {
      final AgentsStatus status = await ref
          .read(daemonClientProvider)
          .agents(op);
      if (ref.mounted) {
        _failures = 0;
        state = AsyncData<AgentsStatus>(status);
        _schedule(status);
      }
      return (status, null);
    } on DaemonException catch (error) {
      return (null, error.diagnostic);
    }
  }

  /// Reads the state again. While a run goes on, a failed read is tried
  /// again; after [agentsPollFailureCap] failures in a row the section says
  /// that the daemon does not answer, instead of showing the run forever.
  Future<void> refresh() async {
    try {
      final AgentsStatus status = await ref
          .read(daemonClientProvider)
          .agents(AgentsOp.status);
      if (!ref.mounted) {
        return;
      }
      _failures = 0;
      state = AsyncData<AgentsStatus>(status);
      _schedule(status);
    } on DaemonException catch (error, stack) {
      if (!ref.mounted) {
        return;
      }
      _failures++;
      if (_failures < agentsPollFailureCap) {
        _poll = Timer(agentsPollInterval, () => unawaited(refresh()));
      } else {
        state = AsyncError<AgentsStatus>(error, stack);
      }
    }
  }

  void _schedule(AgentsStatus status) {
    _poll?.cancel();
    final bool running = status.agents.any(
      (AgentDepot entry) => entry.maintenance?.running ?? false,
    );
    if (running) {
      _poll = Timer(agentsPollInterval, () => unawaited(refresh()));
    }
  }
}
