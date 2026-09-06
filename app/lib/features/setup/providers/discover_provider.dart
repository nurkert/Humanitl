/// The search for model servers in the local network (HUM-076).
///
/// # Nothing here starts on its own
///
/// This is the second call of the application that reaches machines on the
/// network, and the larger one: a connection attempt per address and port of
/// the local /24. It runs when a person presses the button in the sheet, and
/// at no other moment — not when the sheet opens, not when the setup screen
/// builds, not on a keystroke. [SetupLlmDiscover.build] therefore returns the
/// idle state and touches no client.
///
/// # What the state says, and what it never claims
///
/// Servers arrive as they answer, so the list grows while the search runs.
/// [LlmDiscoverState.running] is the honest signal for that: the sheet shows
/// motion, not a percentage, because the daemon streams answers and not
/// progress. A finished search with an empty list says "nothing answered" —
/// never "there is nothing".
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';

part 'discover_provider.g.dart';

/// What the search has found so far.
class LlmDiscoverState {
  /// Creates a state.
  const LlmDiscoverState({
    this.servers = const <LlmServer>[],
    this.failure,
    this.running = false,
    this.finished = false,
  });

  /// Nobody has searched yet.
  static const LlmDiscoverState idle = LlmDiscoverState();

  /// The servers that answered, in the order they did.
  final List<LlmServer> servers;

  /// Why the search could not run, when it could not.
  final Diagnostic? failure;

  /// True while the search is running.
  final bool running;

  /// True once a search has ended by itself.
  final bool finished;

  /// True while nobody has searched at all.
  bool get isIdle => !running && !finished && failure == null;

  /// True when a finished search found nothing.
  bool get isEmpty => finished && servers.isEmpty && failure == null;

  /// The same state with one more server.
  LlmDiscoverState plus(LlmServer server) => LlmDiscoverState(
    servers: <LlmServer>[...servers, server],
    running: running,
    finished: finished,
  );
}

/// The search, on request and never on its own.
@Riverpod(keepAlive: true)
class SetupLlmDiscover extends _$SetupLlmDiscover {
  StreamSubscription<LlmServer>? _running;

  @override
  LlmDiscoverState build() {
    ref.onDispose(_cancel);
    return LlmDiscoverState.idle;
  }

  /// Starts a search over the local /24.
  ///
  /// A second call while one runs is ignored: the button is shut then, and a
  /// double press must not put two scans on the same network.
  void start() {
    if (state.running) {
      return;
    }
    _cancel();
    state = const LlmDiscoverState(running: true);
    // Der Strom wird abonniert und nicht abgewartet: Jede Zeile soll sichtbar
    // werden, sobald sie da ist. Jeder Fehlerweg endet hier, nicht nur der
    // erwartete -- sonst bliebe das Blatt für immer im Zustand „sucht" stehen
    // (dieselbe Begründung wie bei `SetupLlmProbe.run`).
    _running = ref
        .read(daemonClientProvider)
        .discoverLlm()
        .listen(
          (LlmServer server) => state = state.plus(server),
          onError: (Object error) {
            _running = null;
            state = LlmDiscoverState(
              servers: state.servers,
              finished: true,
              failure: error is DaemonException
                  ? error.diagnostic
                  : ClientDiagnostics.daemonUnreachable(
                      socketPath: '?',
                      detail: error.toString(),
                    ),
            );
          },
          onDone: () {
            _running = null;
            state = LlmDiscoverState(servers: state.servers, finished: true);
          },
          cancelOnError: true,
        );
  }

  /// Stops a running search.
  ///
  /// Cancelling the subscription ends the call, and the daemon stops asking:
  /// that is what closing the sheet does. What was found stays visible.
  void stop() {
    if (_running == null) {
      return;
    }
    _cancel();
    state = LlmDiscoverState(servers: state.servers, finished: true);
  }

  /// Forgets everything a search found.
  void forget() {
    _cancel();
    state = LlmDiscoverState.idle;
  }

  void _cancel() {
    unawaited(_running?.cancel());
    _running = null;
  }
}
