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
  Future<SandboxStatus> build() async {
    final DaemonClient client = ref.watch(daemonClientProvider);
    SandboxStatus status = await _drain(client.sandboxStatus());
    // The ring in the header reads this provider and is on screen from the
    // first frame. A sandbox that was already up when the window opened has
    // three results to give, and a grey ring over a running sandbox is an
    // answer nobody asked for (BACKLOG.md 5).
    if (status.isUp) {
      status = await _drain(client.checkIsolation(), from: status);
    }
    return status;
  }

  /// Asks again. Used when the section becomes visible.
  ///
  /// A sandbox that was already up when this screen opened -- started from
  /// the command line, or in a second window -- never sent its three results
  /// down this stream. They are fetched here rather than left as three grey
  /// lines: an unknown guarantee is honest only as long as nobody could have
  /// asked (CONVENTIONS 4.13).
  Future<void> refresh() async {
    await _apply((DaemonClient client) => client.sandboxStatus());
    if (state.value?.isUp ?? false) {
      await _apply(
        (DaemonClient client) => client.checkIsolation(),
        clearDiagnostics: false,
      );
    }
  }

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
  ///
  /// [clearDiagnostics] is false where one gesture makes two calls -- a
  /// refresh that then measures the isolation -- because the second call
  /// would otherwise drop the findings of the first.
  Future<void> _apply(
    Stream<SandboxUpdate> Function(DaemonClient) call, {
    bool clearDiagnostics = true,
  }) async {
    final DaemonClient client = ref.read(daemonClientProvider);
    SandboxStatus current = state.value ?? const SandboxStatus();
    // A new operation starts without the findings of the last one: a
    // diagnostic that named the previous failure would otherwise keep the
    // start button disabled after the cause is gone.
    if (clearDiagnostics) {
      current = current.copyWith(diagnostics: const <Diagnostic>[]);
    }
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
  Future<SandboxStatus> _drain(
    Stream<SandboxUpdate> updates, {
    SandboxStatus from = const SandboxStatus(),
  }) async {
    SandboxStatus status = from;
    await for (final SandboxUpdate update in updates) {
      status = _fold(status, update);
    }
    return status;
  }

  /// One event on top of [current].
  SandboxStatus _fold(
    SandboxStatus current,
    SandboxUpdate update,
  ) => switch (update) {
    // A snapshot replaces the old one but keeps the findings of this
    // operation: the daemon sends the reason first and the failed state
    // after it, and a state that dropped the reason would show a red
    // header with nothing under it. `SandboxEvent.Status` has no field
    // for them.
    //
    // The measured guarantees travel the same way and are carried over
    // under one condition: they still belong to the run the snapshot is
    // about. `carryChecksInto` is what decides that -- a start, a stop or
    // another sandbox id ends the ownership.
    SandboxUpdateStatus(:final SandboxStatus status) =>
      current
          .carryChecksInto(status)
          .copyWith(diagnostics: current.diagnostics),
    SandboxUpdateDiagnostic(:final Diagnostic diagnostic) => current.copyWith(
      diagnostics: <Diagnostic>[...current.diagnostics, diagnostic],
    ),
    SandboxUpdateLog(:final SandboxLogLine line) => _log(current, line),
    SandboxUpdateArgvLine() => current,
    // One measured guarantee. It replaces the result for the same check
    // and is appended otherwise, so a second measurement of a running
    // sandbox never doubles the list. The first result of a run writes
    // down which sandbox it was measured in -- `null` while the sandbox
    // is still coming up, and then adopted when the start ends.
    SandboxUpdateCheck(:final IsolationCheckResult result) => current.copyWith(
      checks: _withCheck(current.checks, result),
      checksSandboxId: current.checks.isEmpty
          ? current.sandboxId
          : current.checksSandboxId,
    ),
  };

  /// [checks] with [result] in the place of the earlier result for the same
  /// guarantee, or appended when there was none.
  List<IsolationCheckResult> _withCheck(
    List<IsolationCheckResult> checks,
    IsolationCheckResult result,
  ) {
    final List<IsolationCheckResult> next = <IsolationCheckResult>[
      for (final IsolationCheckResult old in checks)
        if (old.check != result.check) old,
      result,
    ];
    // The order of the wire enum, so the panel and the ring draw the three
    // guarantees in the same order no matter how the events arrived.
    next.sort(
      (IsolationCheckResult a, IsolationCheckResult b) =>
          a.check.index.compareTo(b.check.index),
    );
    return List<IsolationCheckResult>.unmodifiable(next);
  }

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

  /// The three guarantees, each with the evidence it produced.
  isolation,

  /// What the daemon logged about this sandbox.
  log,
}
