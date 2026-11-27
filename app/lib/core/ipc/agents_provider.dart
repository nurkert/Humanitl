/// The state of the agent depot as `Agents` answers it (HUM-521).
///
/// It lives in `core` because the settings and the guided agent setup of
/// the start window (GitHub #111) read and drive the same state, and no
/// feature imports another one (ARCHITECTURE 5).
///
/// Every operation answers with the state afterwards, and that answer is the
/// new state of the section; nothing is computed here. While a maintenance
/// run goes on, the state is read again every [agentsPollInterval], so the
/// URLs of a login, the end of a run and the changes of a configuration run
/// appear without a click.
library;

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'client_providers.dart';
import 'config_providers.dart';
import 'daemon_client.dart';

/// How often the state is read while a maintenance run goes on.
const Duration agentsPollInterval = Duration(milliseconds: 1500);

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
