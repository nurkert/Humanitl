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
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';

import 'closed_failures.dart';
import 'disposable_review.dart';

part 'sandbox_status_provider.g.dart';

/// How many log lines the screen keeps.
///
/// The log is a window on what happened, not an archive: the recording is
/// (`humanitl history`). A bound is what keeps a long session from turning
/// the list into the memory profile of the whole run.
const int sandboxLogLimit = 2000;

/// How long a background question for the snapshot waits for an event
/// before it gives up (HUM-306).
const Duration sandboxPollDeadline = Duration(seconds: 10);

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
    // A folder change that had to wait -- for the first snapshot, for a
    // sandbox to come down -- is planned once the snapshot allows it
    // (HUM-295).
    listenSelf((AsyncValue<SandboxStatus>? _, AsyncValue<SandboxStatus> _) {
      _reconcileWorkDir();
    });
    SandboxStatus status = await _drain(
      client.sandboxStatus(),
      call: SandboxCall.status,
    );
    // The ring in the header reads this provider and is on screen from the
    // first frame. A sandbox that was already up when the window opened has
    // three results to give, and a grey ring over a running sandbox is an
    // answer nobody asked for (BACKLOG.md 5).
    if (status.isUp) {
      status = await _drain(
        client.checkIsolation(),
        from: status,
        call: SandboxCall.isolationCheck,
      );
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
    final int generation = ++_snapshots;
    await _apply(
      (DaemonClient client) => client.sandboxStatus(),
      call: SandboxCall.status,
      generation: generation,
    );
    if (generation == _snapshots && (state.value?.isUp ?? false)) {
      await _apply(
        (DaemonClient client) => client.checkIsolation(),
        call: SandboxCall.isolationCheck,
        clearDiagnostics: false,
        generation: generation,
      );
    }
  }

  /// Asks what a start with the stored folder and [workMode] would mount,
  /// without starting anything.
  ///
  /// The daemon answers with the mounts, the environment and the command line
  /// and the screen shows that answer. Nothing is computed here (ADR-018).
  Future<void> plan({WorkMode? workMode}) {
    _plannedWorkDir = _storedWorkDir ?? _plannedWorkDir;
    return _apply(
      (DaemonClient client) => client.planSandbox(workMode: workMode),
      call: SandboxCall.plan,
      generation: ++_snapshots,
    );
  }

  /// Takes note of the project folder the configuration holds now, and plans
  /// again once it differs from the folder the snapshot was planned with
  /// (HUM-295).
  ///
  /// The settings write `sandbox.work_dir` through `configProvider`, and no
  /// feature imports another one (ARCHITECTURE 5). The shell, which holds
  /// both, hands every readable value of the folder in here; a folder
  /// changed in the settings then reaches the sandbox header and the setup
  /// row without a change of screen. The first value noted is the folder the
  /// snapshot already shows, not a change.
  void storedWorkDirRead(String workDir) {
    final String? before = _storedWorkDir;
    _storedWorkDir = workDir;
    if (before == null) {
      _plannedWorkDir ??= workDir;
      return;
    }
    _reconcileWorkDir();
  }

  /// Plans once when the stored folder is not the one the snapshot was
  /// planned with, and the snapshot allows it now.
  ///
  /// Three cases wait instead of dropping the change. A [chooseWorkDir]
  /// under way plans by itself after its write and calls this when the last
  /// one ends; a plan now would only race it. Before the first snapshot it
  /// is unknown whether a sandbox runs. A sandbox that is up or on its way
  /// keeps its folder, the new one applies to the next start; the folder
  /// pickers rest in that state for the same reason. The last two are
  /// reconciled by the listener on the snapshot in [build].
  void _reconcileWorkDir() {
    final String? stored = _storedWorkDir;
    if (stored == null || stored == _plannedWorkDir || _choosing > 0) {
      return;
    }
    final SandboxStatus? current = state.value;
    if (current == null || current.isUp || current.isBusy) {
      return;
    }
    _plannedWorkDir = stored;
    unawaited(
      _apply(
        (DaemonClient client) => client.planSandbox(),
        call: SandboxCall.plan,
        generation: ++_snapshots,
      ),
    );
  }

  /// The folder [storedWorkDirRead] noted last, or null before the first.
  String? _storedWorkDir;

  /// The stored folder the snapshot was last planned with, or null before
  /// the first one is known.
  String? _plannedWorkDir;

  /// Counts the requests that answer with a whole snapshot: a refresh and
  /// every plan. Only the latest one writes the snapshot, so an older answer
  /// that arrives late never replaces a newer one (HUM-295). A start and a
  /// stop do not count; their events are the state of the sandbox itself.
  int _snapshots = 0;

  /// Stores [workDir] as the project folder and asks what a start with it
  /// would mount (HUM-293).
  ///
  /// This is the whole of the project folder picker in setup and on the
  /// sandbox screen. The folder goes into `sandbox.work_dir` through
  /// `SetConfig`, the one place the daemon, the desktop launcher and
  /// `humanitl sandbox run` read it from; the daemon checks it before it
  /// writes. A folder it refuses is not stored: its `SANDBOX_006` stands
  /// under the folder control as the finding of a plan would, and the
  /// snapshot keeps the folder that was stored before. A running sandbox
  /// keeps its folder; the new one applies to the next start.
  ///
  /// The writes go out one after the other, in the order the folders were
  /// picked, so the latest choice is also the last write to `config.toml`.
  /// Only the latest choice writes the snapshot: a refusal of an older one,
  /// or a late event of its plan, must not cover a newer one. Only
  /// `SetConfig` itself can refuse; a configuration that cannot be read
  /// again afterwards is not a refusal, and the plan runs anyway.
  Future<void> chooseWorkDir(String workDir) async {
    _choosing++;
    try {
      await _chooseWorkDir(workDir);
    } finally {
      _choosing--;
    }
    // A folder the settings stored while this choice was under way was left
    // to it; whatever is stored now and not planned yet is planned once.
    _reconcileWorkDir();
  }

  Future<void> _chooseWorkDir(String workDir) async {
    final int mine = ++_chooses;
    final Future<Diagnostic?> stored = _chooseChain.then(
      (_) => _store(workDir),
    );
    _chooseChain = stored.then((_) {}, onError: (Object _) {});
    final Diagnostic? refusal = await stored;
    if (mine != _chooses) {
      return;
    }
    if (refusal != null) {
      final SandboxStatus current = state.value ?? const SandboxStatus();
      ref.read(sandboxCallProvider.notifier).record(SandboxCall.plan);
      state = AsyncData<SandboxStatus>(
        current.copyWith(diagnostics: <Diagnostic>[refusal]),
      );
      return;
    }
    // Stored. The form reads the file again; whether that works says nothing
    // about the folder.
    try {
      await ref.read(configProvider.notifier).refresh();
    } on Object {
      // The settings show their own state of the configuration.
    }
    if (mine != _chooses) {
      return;
    }
    _plannedWorkDir = workDir;
    await _apply(
      (DaemonClient client) => client.planSandbox(),
      call: SandboxCall.plan,
      choice: mine,
      generation: ++_snapshots,
    );
  }

  /// `SetConfig` for [workDir]; the refusal of the daemon, or null.
  Future<Diagnostic?> _store(String workDir) async {
    try {
      await ref
          .read(daemonClientProvider)
          .setConfig(FormKeys.sandboxWorkDir, workDir);
      return null;
    } on DaemonException catch (error) {
      return error.diagnostic;
    }
  }

  /// Counts the calls of [chooseWorkDir]; the latest one owns the snapshot.
  int _chooses = 0;

  /// How many calls of [chooseWorkDir] are under way.
  int _choosing = 0;

  /// The write of the choice before, which the next one waits for.
  Future<void> _chooseChain = Future<void>.value();

  /// Starts the sandbox. Every event of the start updates the snapshot as it
  /// arrives, so `starting` is on the screen before `running` is.
  ///
  /// The mode of the switch "Disposable" goes along only to a daemon that
  /// names `sandbox.disposable`, and only once somebody chose one
  /// ([SandboxModeChoice]); otherwise the field stays empty and the daemon
  /// takes `sandbox.mode` (HUM-351).
  Future<void> start() {
    final bool offered =
        ref.read(daemonInfoProvider).value?.offersDisposable ?? false;
    final SandboxMode? mode = offered
        ? ref.read(sandboxModeChoiceProvider)
        : null;
    return _apply(
      (DaemonClient client) => client.startSandbox(sandboxMode: mode),
      call: SandboxCall.start,
    );
  }

  /// Asks for the snapshot alone, for a window that only needs to know
  /// whether a session runs (HUM-306): the shell asks every few seconds while
  /// the empty queue shows, because a session started or stopped on the
  /// command line sends this window nothing.
  ///
  /// Lighter than [refresh] in three ways. It measures no isolation. It
  /// keeps the findings that stand, because a finding of the last start must
  /// not vanish from the setup screen because time passed. And it counts as
  /// no newer question: it takes the current count of [_snapshots] without
  /// raising it, so a plan or refresh in flight keeps its answer, and an
  /// answer of this call that arrives after one of them began is dropped. A
  /// failure leaves the snapshot alone; the next question comes anyway, and
  /// a red sandbox screen because a background question went unanswered
  /// would be a claim nobody asked for.
  ///
  /// It touches the snapshot only: a finding or a log line on its stream is
  /// not taken, because the daemon repeats the findings of its preflight
  /// with every snapshot, and a question every five seconds would pile up a
  /// copy each time and claim them for itself (`sandboxCallProvider`).
  ///
  /// Only one question at a time: while one is in flight, another call
  /// joins it. After [sandboxPollDeadline] without an end the question
  /// counts as lost: its answer, should it still come, is dropped, and the
  /// next call asks anew, so a daemon that hangs does not hold every later
  /// question back. An answer that set out before the last event of any
  /// other call is dropped as well ([_sessionEvents]): an `Exit` that
  /// arrived meanwhile is newer than a snapshot taken before it.
  Future<void> pollStatus() {
    final Future<void>? running = _poll;
    if (running != null) {
      return running;
    }
    final int token = ++_pollToken;
    final Timer deadline = Timer(sandboxPollDeadline, () {
      if (token == _pollToken) {
        _pollToken++;
        _poll = null;
      }
    });
    final Future<void> asked =
        _apply(
          (DaemonClient client) => client.sandboxStatus(),
          call: SandboxCall.status,
          clearDiagnostics: false,
          generation: _snapshots,
          poll: token,
        ).whenComplete(() {
          deadline.cancel();
          if (token == _pollToken) {
            _poll = null;
          }
        });
    return _poll = asked;
  }

  /// The count of questions of [pollStatus]; a question whose number is no
  /// longer the count was given up and its answer is dropped.
  int _pollToken = 0;

  /// The question of [pollStatus] in flight, if one is.
  Future<void>? _poll;

  /// How many events the other calls have folded in so far (HUM-306).
  ///
  /// A start, a stop, a plan and a refresh each count every event they
  /// fold; a question of [pollStatus] takes the count when it sets out and
  /// drops its answer when the count moved.
  int _sessionEvents = 0;

  /// Stops the sandbox.
  Future<void> stop() => _apply(
    (DaemonClient client) => client.stopSandbox(),
    call: SandboxCall.stop,
  );

  /// Runs [request] and folds every event into the snapshot as it arrives.
  ///
  /// [call] says which of the `Sandbox` calls this is, so that a finding can
  /// be drawn where it belongs: `sandboxCallProvider` carries it, and
  /// [SandboxCall.isAboutTheFolder] is what the setup screen asks.
  ///
  /// [clearDiagnostics] is false where one gesture makes two calls -- a
  /// refresh that then measures the isolation -- because the second call
  /// would otherwise drop the findings of the first.
  ///
  /// [generation] is the count of [_snapshots] the request took; its events
  /// are dropped once a newer refresh or plan was asked for.
  ///
  /// **[call] travels with the stream and is never a field of this notifier.**
  /// Two `Sandbox` calls can be in flight at the same time: the start of a
  /// sandbox takes as long as a sandbox takes to come up, and a `plan` from
  /// the folder picker or the `refresh` that a visible sandbox section fires
  /// runs to its end while the start still waits for its next event. A field
  /// would carry whichever call was begun last, so the refusal of the start
  /// would be recorded as `plan` or `status`, `SandboxCall.isAboutTheFolder`
  /// would say yes, and `_projectCheck` would put the start's finding back on
  /// the project row -- the very dead end `docs/UX.md` 4.4 forbids, returning
  /// through a race. The parameter is captured by the closure of this call and
  /// no other one can reach it.
  Future<void> _apply(
    Stream<SandboxUpdate> Function(DaemonClient) request, {
    required SandboxCall call,
    bool clearDiagnostics = true,
    int? choice,
    int? generation,
    int? poll,
  }) async {
    // A question of [pollStatus] carries its number; it is the quiet kind.
    final bool quiet = poll != null;
    // A plan of a folder choice counts only while no newer choice exists
    // (HUM-293); its late events would show the older folder.
    // An answer of a refresh or a plan counts only while no newer one was
    // asked for (HUM-295).
    final int sessionEvents = _sessionEvents;
    bool superseded() =>
        (choice != null && choice != _chooses) ||
        (generation != null && generation != _snapshots) ||
        (quiet && sessionEvents != _sessionEvents) ||
        (poll != null && poll != _pollToken);
    final DaemonClient client = ref.read(daemonClientProvider);
    // Eine Operation, kein Blick auf den Stand: Was ihre Vorgängerin an
    // Fehlschlägen meldete und jemand geschlossen hat, gilt für sie nicht
    // mehr (HUM-302).
    if (call != SandboxCall.status && call != SandboxCall.isolationCheck) {
      ref.read(closedFailuresProvider.notifier).newOperation();
    }
    SandboxStatus current = state.value ?? const SandboxStatus();
    // A new operation starts without the findings of the last one: a
    // diagnostic that named the previous failure would otherwise keep the
    // start button disabled after the cause is gone.
    if (clearDiagnostics) {
      current = current.copyWith(diagnostics: const <Diagnostic>[]);
    }
    try {
      await for (final SandboxUpdate update in request(client)) {
        if (superseded()) {
          return;
        }
        // A background question folds onto what stands now, not onto what
        // stood when it was asked: a start that measured the guarantees in
        // between would otherwise lose them to an answer that set out
        // earlier (HUM-306).
        if (quiet) {
          // Only the snapshot itself; see [pollStatus].
          if (update is! SandboxUpdateStatus) {
            continue;
          }
          current = state.value ?? current;
        } else {
          _sessionEvents++;
        }
        current = _fold(current, update, call);
        state = AsyncData<SandboxStatus>(current);
      }
    } on DaemonException catch (error, stack) {
      if (!superseded() && !quiet) {
        state = AsyncError<SandboxStatus>(error, stack);
      }
    }
  }

  /// Reads [updates] to its end and answers the snapshot it leaves behind.
  ///
  /// [call] says which `Sandbox` call [updates] belongs to, for the same
  /// reason it is a parameter of [_apply] and not a field.
  Future<SandboxStatus> _drain(
    Stream<SandboxUpdate> updates, {
    required SandboxCall call,
    SandboxStatus from = const SandboxStatus(),
  }) async {
    SandboxStatus status = from;
    await for (final SandboxUpdate update in updates) {
      status = _fold(status, update, call);
    }
    return status;
  }

  /// One event of [call] on top of [current].
  SandboxStatus _fold(
    SandboxStatus current,
    SandboxUpdate update,
    SandboxCall call,
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
    // Der Befund kommt mitsamt der Frage, wo er hingehört: Die Leitung hat für
    // alle Aufrufe dasselbe Feld, aber ein Befund von `Plan` redet über den
    // gewählten Ordner und einer von `Start` über den Druck auf den Knopf
    // (`docs/UX.md` 4.4). Geschrieben wird erst hier und nicht schon beim
    // Aufruf: Was zählt, ist der Aufruf, der die Befunde erzeugt hat, die
    // gerade dastehen. Welcher das ist, sagt [call] und kein Feld -- der
    // Grund dafür steht bei [_apply]. Macht eine Geste zwei Aufrufe und
    // liefern beide einen Befund -- `refresh` mit `Sandbox(Status)` und dann
    // der Isolationsprüfung --, dann gilt der zuletzt eingetroffene für die
    // ganze Liste.
    SandboxUpdateDiagnostic(:final Diagnostic diagnostic) => _withDiagnostic(
      current,
      diagnostic,
      call,
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
    // The whole tally of refused attempts; it replaces the last one, never
    // adds to it (HUM-138). It does not touch the queue of flows: a refused
    // socket never became a request, so there is nothing to decide.
    SandboxUpdateRefusals(:final SandboxRefusals refusals) => current.copyWith(
      refusals: refusals,
    ),
    // The agent ended; the sandbox around it may still stand, and that is
    // what the snapshot says from now on (HUM-306).
    SandboxUpdateExit() => current.copyWith(agentRunning: false),
    // The changes of a disposable run (HUM-351): they belong to the panel,
    // not to the snapshot, and the panel opens without a click.
    SandboxUpdateChanges(:final DisposableChanges changes) => _changes(
      current,
      changes,
    ),
    // The answer to apply and discard travels on the panel's own call and
    // never on this stream; should one arrive here, it says nothing about the
    // sandbox.
    SandboxUpdateChangesOutcome() => current,
  };

  /// Hands [changes] to the panel and leaves the snapshot alone.
  SandboxStatus _changes(SandboxStatus current, DisposableChanges changes) {
    ref.read(disposableReviewProvider.notifier).arrived(changes);
    return current;
  }

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

  /// Appends [diagnostic] and writes down that [call] produced it.
  SandboxStatus _withDiagnostic(
    SandboxStatus current,
    Diagnostic diagnostic,
    SandboxCall call,
  ) {
    ref.read(sandboxCallProvider.notifier).record(call);
    return current.copyWith(
      diagnostics: <Diagnostic>[...current.diagnostics, diagnostic],
    );
  }
}

/// Which `Sandbox` call produced the findings the snapshot carries right now.
///
/// A provider of its own and not a field on the snapshot: the snapshot is the
/// mirror of `SandboxEvent.Status` and carries nothing the daemon did not say
/// (ADR-018). Which of our own calls a finding came back from is knowledge of
/// this client, and the setup screen needs it to draw the finding under the
/// control it belongs to (`docs/UX.md` 4.4).
@Riverpod(keepAlive: true, name: 'sandboxCallProvider')
class SandboxCallOfDiagnostics extends _$SandboxCallOfDiagnostics {
  @override
  SandboxCall build() => SandboxCall.status;

  /// Writes down that [call] produced the findings that stand now.
  void record(SandboxCall call) => state = call;
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
