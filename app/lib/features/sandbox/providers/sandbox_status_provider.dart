/// What the agent gets, as the daemon last said it (HUM-040).
///
/// Everything on the sandbox screen reads from here, and everything here came
/// out of the `Sandbox` RPC. The screen derives no mount, guesses no value and
/// shortens no command line; when the daemon does not know something, it stays
/// unknown rather than becoming a plausible sentence (ADR-018,
/// CONVENTIONS 4.13).
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';

part 'sandbox_status_provider.g.dart';

/// How many log lines the screen keeps.
///
/// The log is a window on what happened, not an archive: the recording is
/// (`humanitl history`). A bound is what keeps a long session from turning
/// the list into the memory profile of the whole run.
const int sandboxLogLimit = 2000;

/// Riverpod 3 retries a failed provider on its own and reports `AsyncLoading`
/// with the error tucked inside while it does. A screen whose whole job is to
/// say what is mounted must not show a skeleton for ever instead of the
/// reason it has nothing; reloading is explicit, and the diagnostic carries
/// the control.
Duration? noSandboxRetry(int retryCount, Object error) => null;

/// The snapshot of the sandbox, as the daemon last answered.
@Riverpod(keepAlive: true, retry: noSandboxRetry)
class SandboxStatusNotifier extends _$SandboxStatusNotifier {
  @override
  Future<SandboxStatus> build() =>
      _drain(ref.watch(daemonClientProvider).sandboxStatus());

  /// Asks again. Used when the section becomes visible.
  Future<void> refresh() =>
      _apply((DaemonClient client) => client.sandboxStatus());

  /// Asks what a start with [workDir] and [workMode] would mount, without
  /// starting anything.
  ///
  /// This is the whole of the project directory picker: the person chooses a
  /// directory, the daemon answers with the mounts, the environment and the
  /// command line that directory produces, and the screen shows that answer.
  /// Nothing is computed here (ADR-018).
  Future<void> plan({String? workDir, WorkMode? workMode}) => _apply(
    (DaemonClient client) =>
        client.planSandbox(workDir: workDir, workMode: workMode),
  );

  /// Starts the sandbox. Every event of the start updates the snapshot as it
  /// arrives, so `starting` is on the screen before `running` is.
  Future<void> start() =>
      _apply((DaemonClient client) => client.startSandbox());

  /// Stops the sandbox.
  Future<void> stop() => _apply((DaemonClient client) => client.stopSandbox());

  /// Runs [call] and folds every event into the snapshot as it arrives.
  Future<void> _apply(Stream<SandboxUpdate> Function(DaemonClient) call) async {
    final DaemonClient client = ref.read(daemonClientProvider);
    SandboxStatus current = state.value ?? const SandboxStatus();
    // A new operation starts without the findings of the last one: a
    // diagnostic that named the previous failure would otherwise keep the
    // start button disabled after the cause is gone.
    current = current.copyWith(diagnostics: const <Diagnostic>[]);
    try {
      await for (final SandboxUpdate update in call(client)) {
        current = _fold(current, update);
        state = AsyncData<SandboxStatus>(current);
      }
    } on DaemonException catch (error, stack) {
      state = AsyncError<SandboxStatus>(error, stack);
    }
  }

  /// Reads [updates] to its end and answers the snapshot it leaves behind.
  Future<SandboxStatus> _drain(Stream<SandboxUpdate> updates) async {
    SandboxStatus status = const SandboxStatus();
    await for (final SandboxUpdate update in updates) {
      status = _fold(status, update);
    }
    return status;
  }

  /// One event on top of [current].
  SandboxStatus _fold(SandboxStatus current, SandboxUpdate update) =>
      switch (update) {
        // A snapshot replaces the old one but keeps the findings of this
        // operation: the daemon sends the reason first and the failed state
        // after it, and a state that dropped the reason would show a red
        // header with nothing under it.
        SandboxUpdateStatus(:final SandboxStatus status) => status.copyWith(
          diagnostics: current.diagnostics,
        ),
        SandboxUpdateDiagnostic(:final Diagnostic diagnostic) =>
          current.copyWith(
            diagnostics: <Diagnostic>[...current.diagnostics, diagnostic],
          ),
        SandboxUpdateLog(:final SandboxLogLine line) => _log(current, line),
        SandboxUpdateArgvLine() => current,
      };

  /// Puts [line] into the log and leaves the snapshot alone.
  SandboxStatus _log(SandboxStatus current, SandboxLogLine line) {
    ref.read(sandboxLogProvider.notifier).add(line);
    return current;
  }
}

/// The lines the daemon logged about the sandbox, oldest first.
///
/// The terminal of the agent is something else and arrives with HUM-042; this
/// is the daemon talking about the sandbox, not the agent talking.
@Riverpod(keepAlive: true)
class SandboxLog extends _$SandboxLog {
  @override
  List<SandboxLogLine> build() => const <SandboxLogLine>[];

  /// Appends [line], dropping the oldest once [sandboxLogLimit] is reached.
  void add(SandboxLogLine line) {
    final List<SandboxLogLine> next = <SandboxLogLine>[...state, line];
    state = next.length <= sandboxLogLimit
        ? next
        : next.sublist(next.length - sandboxLogLimit);
  }

  /// Empties the log.
  void clear() => state = const <SandboxLogLine>[];
}

/// Which tab of the lower half is open. A `PageStorageKey` keeps the scroll
/// position of each; this keeps the choice itself across a section change.
@Riverpod(keepAlive: true)
class SandboxTabChoice extends _$SandboxTabChoice {
  @override
  SandboxTab build() => SandboxTab.mounts;

  /// Opens [tab].
  void go(SandboxTab tab) => state = tab;
}

/// The four tabs below the terminal.
enum SandboxTab {
  /// Every path the agent sees.
  mounts,

  /// Every environment variable the sandbox sets.
  env,

  /// The three guarantees; the panel arrives with HUM-041.
  isolation,

  /// What the daemon logged about this sandbox.
  log,
}
