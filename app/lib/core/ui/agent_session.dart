/// Whether an agent runs in a sandbox session right now (HUM-306).
///
/// The answer lives in the sandbox feature, in its status snapshot, and the
/// empty queue needs it: a queue that offers "Start session" while the agent
/// works in the terminal sends somebody to start what already runs. No
/// feature imports another (ARCHITECTURE 5, `tools/check-deps.sh`), so the
/// shell, which knows both, copies the one bit here, and the queue reads it.
///
/// One bit and not the snapshot: the queue has no business with mounts,
/// sandbox ids or isolation results, and a smaller note is one fewer reason
/// for it to rebuild.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// True while the agent of a sandbox session runs; false at the start.
final NotifierProvider<AgentSessionNotifier, bool> agentSessionProvider =
    NotifierProvider<AgentSessionNotifier, bool>(AgentSessionNotifier.new);

/// The notifier behind [agentSessionProvider].
class AgentSessionNotifier extends Notifier<bool> {
  @override
  bool build() => false;

  /// Takes what the sandbox snapshot says about the agent.
  void report({required bool running}) => state = running;
}
