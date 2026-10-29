/// How the shell keeps the one bit of `agentSessionProvider` true (HUM-306).
///
/// The empty queue says "The agent is working" while the agent of a sandbox
/// session runs. What runs is known to the sandbox feature, in its snapshot;
/// the queue may not import that feature (ARCHITECTURE 5), so the shell,
/// which knows both, copies the bit into the core. This file is that copy
/// and the care that keeps it fresh:
///
/// - **The snapshot changes.** A start, a stop and the end of the agent on
///   the stream of the start (`SandboxEvent.Exit`) change the snapshot, and
///   the bit follows.
/// - **Somebody else changes the session.** A session started or stopped on
///   the command line sends nothing to this window; the daemon has no stream
///   that announces sessions. While the empty queue is what the person looks
///   at, the shell asks again: when the queue comes into view, when the
///   window comes back to the front, and every [agentSessionPollInterval]
///   in between. Only the snapshot, never the isolation check, and only
///   while the link lives.
/// - **The link is down.** Then nobody knows what runs, and the queue does
///   not claim that the agent works.
library;

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/connection.dart';
import '../../core/ui/agent_session.dart';
import '../../core/ui/terminal_placement.dart';
import '../intercept/providers/flows.dart';
import '../sandbox/providers/sandbox_status_provider.dart';
import 'providers/navigation.dart';
import 'section.dart';

/// How often the shell asks for the snapshot while the empty queue shows.
///
/// One `Sandbox(Status)` call, answered from the daemon's memory. Five
/// seconds is the time after which a person who started a session in a
/// terminal next to the window starts to wonder why the window has not
/// noticed.
const Duration agentSessionPollInterval = Duration(seconds: 5);

/// The listeners, the timer and the lifecycle hook behind the bit.
///
/// Created in `initState` of the shell and disposed with it.
class AgentSessionFeed {
  /// Starts feeding through [ref], which belongs to the shell.
  ///
  /// One feed per app: a second shell in the same process (a test that
  /// builds two, a window opened twice) would double every question. The
  /// first feed works, a later one stays idle for its whole life.
  AgentSessionFeed(this._ref) {
    if (_active != null) {
      _idle = true;
      return;
    }
    _active = this;
    _ref
      ..listenManual<bool>(_agentRunning, (bool? _, bool _) => _report())
      ..listenManual<bool>(linkLiveProvider, (bool? _, bool _) {
        _report();
        _syncPoll();
      })
      ..listenManual<Section>(navigationProvider, (Section? _, Section _) {
        _syncPoll();
      })
      ..listenManual<TerminalPlacement>(
        terminalPlacementProvider,
        (TerminalPlacement? _, TerminalPlacement _) => _syncPoll(),
      )
      ..listenManual<bool>(
        heldFlowsProvider.select((List<Flow> held) => held.isEmpty),
        (bool? _, bool _) => _syncPoll(),
      );
    _lifecycle = AppLifecycleListener(onResume: _resumed);
    // Der erste Stand geht nach dem ersten Frame hinüber und nicht mit
    // `fireImmediately`: Riverpod verbietet, einen Provider zu ändern,
    // während Widgets gebaut werden, und `initState` der Shell gehört zum
    // Bau. Gelesen wird dann der aktuelle Stand, nicht ein gemerkter.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!_disposed) {
        _report();
        _syncPoll();
      }
    });
  }

  /// The feed that works; `null` while there is none.
  static AgentSessionFeed? _active;

  final WidgetRef _ref;
  AppLifecycleListener? _lifecycle;
  bool _idle = false;
  Timer? _poll;
  bool _disposed = false;

  /// Dieselbe Frage wie das Terminal (`SandboxTerminal`): läuft der Agent,
  /// nicht nur die Sandbox.
  static final ProviderListenable<bool> _agentRunning = sandboxStatusProvider
      .select(
        (AsyncValue<SandboxStatus> status) =>
            status.value?.agentRunning ?? false,
      );

  /// Writes the bit: the agent runs, and the link that said so still lives.
  void _report() => _ref
      .read(agentSessionProvider.notifier)
      .report(running: _ref.read(linkLiveProvider) && _ref.read(_agentRunning));

  /// Whether the empty queue is what the window shows right now.
  bool _queueEmptyInView() =>
      _ref.read(linkLiveProvider) &&
      _ref.read(navigationProvider) == Section.intercept &&
      _ref.read(terminalPlacementProvider) == TerminalPlacement.pane &&
      _ref.read(heldFlowsProvider).isEmpty;

  /// Starts the timer when the empty queue comes into view, with one question
  /// right away, and stops it when the queue leaves.
  void _syncPoll() {
    if (_disposed) {
      return;
    }
    final bool wanted = _queueEmptyInView();
    if (wanted && _poll == null) {
      _ask();
      _poll = Timer.periodic(agentSessionPollInterval, (Timer _) => _ask());
    } else if (!wanted && _poll != null) {
      _poll!.cancel();
      _poll = null;
    }
  }

  /// The window came back to the front: a session may have changed meanwhile.
  void _resumed() {
    if (!_disposed && _queueEmptyInView()) {
      _ask();
    }
  }

  void _ask() =>
      unawaited(_ref.read(sandboxStatusProvider.notifier).pollStatus());

  /// Stops the timer and the lifecycle hook.
  void dispose() {
    _disposed = true;
    _poll?.cancel();
    _poll = null;
    _lifecycle?.dispose();
    if (identical(_active, this)) {
      _active = null;
    }
  }

  /// Whether this feed stays idle because another one works.
  @visibleForTesting
  bool get idle => _idle;
}
