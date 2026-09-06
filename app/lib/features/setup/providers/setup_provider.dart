/// The four checks of the setup screen, and where each verdict comes from
/// (HUM-044).
///
/// # This file decides nothing about the machine
///
/// Every verdict below was made in the daemon. `humanitl_sandbox::doctor`
/// reads the machine into facts and judges them; `Sandbox(Plan)` says whether
/// a start with the chosen folder would work; `ProbeLlm` says what the model
/// server answered. What happens here is that eleven daemon lines and two
/// stream answers are folded into four rows a person can read, and that fold
/// never turns a red line green (ADR-018, CONVENTIONS 4.13).
///
/// # A check that could not run is its own state
///
/// The contract knows `OK`, `WARN` and `FAIL` and nothing else, so a check the
/// daemon could not perform (`DOCTOR_012`) and an endpoint nobody asked it to
/// contact (`DOCTOR_013`) both arrive as warnings. [SetupCheckState.unmeasured]
/// gives them their own state again, and the screen draws them as their own
/// thing: a hollow mark, never the green one.
///
/// **What an unmeasured check does to the start button, and why.** It keeps it
/// shut, exactly like a failed one. The specification says so twice -- "all
/// four green, then the button `Start agent` is on", and the test it names,
/// `start_button_enabled_only_when_all_ok` -- and the reason is not a verdict
/// about the machine but about the sentence this screen would otherwise say. A
/// checklist that offers the start while admitting it did not look at one of
/// its four lines asks somebody to act on a list it has not filled in.
///
/// The proof of the three guarantees is a different gate at a different
/// moment. It happens **inside the running sandbox**
/// (`Sandbox(IsolationCheck)`, HUM-041), and a sandbox whose guarantees do not
/// hold is stopped by the daemon rather than handed over (`SANDBOX_013`). The
/// doctor checks preconditions, that gate proves the guarantees, and neither
/// stands in for the other. This button waits for the first of the two.
///
/// So a row nobody could measure is never green, it keeps the button shut, and
/// it is named on the button's own line together with the one control that
/// would measure it: `Check again`, `Contact it`, or the folder button.
library;

import 'package:flutter/foundation.dart';
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_diagnostics.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';

part 'setup_provider.g.dart';

/// The four rows of the setup screen, in the order they are drawn.
enum SetupCheckKind {
  /// Does the background service answer?
  daemon,

  /// Is there a model server, and does it answer?
  llm,

  /// Is there a folder the agent may work in?
  project,

  /// Will a sandbox start on this machine?
  sandbox,
}

/// What one row says.
///
/// The order is the ranking used by [SetupState.worst]: a failure outranks
/// everything, and "still asking" outranks a settled answer, because a row
/// that is still working is not yet an answer at all.
enum SetupCheckState {
  /// Measured, and in order.
  ok,

  /// Measured, and worth knowing about. Not green, so the start waits.
  warn,

  /// Nothing was measured -- the check could not run, or nobody asked for it.
  /// Never green, and therefore never a state a start happens in; see the
  /// library comment.
  unmeasured,

  /// The answer is not there yet.
  checking,

  /// Measured, and without a change nothing starts.
  failed,
}

/// One row: what it is about, how it went, and what to show under it.
///
/// A value type, and that is a budget and not a taste: the four providers this
/// row is folded out of publish a new object on every change, and without
/// value equality every one of them would rebuild `SetupHost`, its four rows
/// and the nine-row list under them, offstage as well as on
/// (`docs/UX.md` 7). [QueueSnapshot] with `listEquals` is the same idiom.
@immutable
class SetupCheck {
  /// Creates a row.
  const SetupCheck({
    required this.kind,
    required this.state,
    this.diagnostic,
    this.detail = '',
  });

  /// Which of the four.
  final SetupCheckKind kind;

  /// How it went.
  final SetupCheckState state;

  /// The finding of the daemon, when the row is not green.
  final Diagnostic? diagnostic;

  /// The evidence, in one line: the version, the folder, the endpoint.
  final String detail;

  /// True when this row is green: measured, and in order.
  ///
  /// The one state a start happens in. Every other one keeps the button shut,
  /// including [SetupCheckState.unmeasured] -- a row nobody looked at is not a
  /// row that passed (HUM-044, `start_button_enabled_only_when_all_ok`).
  bool get isGreen => state == SetupCheckState.ok;

  @override
  bool operator ==(Object other) =>
      other is SetupCheck &&
      other.kind == kind &&
      other.state == state &&
      other.diagnostic == diagnostic &&
      other.detail == detail;

  @override
  int get hashCode => Object.hash(kind, state, diagnostic, detail);
}

/// The finding of the last `Sandbox` call, and whether that call arrived.
///
/// Both belong under the start button, and they are not the same sentence. A
/// call that never reached the daemon says nothing at all about the machine,
/// so the screen says that nothing started and that every row still stands. A
/// call that arrived and came back with a refusal -- a red guarantee after
/// `Sandbox(Start)`, a launcher that would not run -- says that the start did
/// not come up. A screen that offered only the first sentence would tell
/// somebody their request was lost while the daemon was in fact answering
/// (CONVENTIONS 4.13).
@immutable
class SetupCallFailure {
  /// Creates the failure of a call that [delivered] or did not.
  const SetupCallFailure({required this.diagnostic, required this.delivered});

  /// What the finding says.
  final Diagnostic diagnostic;

  /// True when the call reached the daemon and the daemon answered with this
  /// finding; false when the call itself did not arrive.
  final bool delivered;

  @override
  bool operator ==(Object other) =>
      other is SetupCallFailure &&
      other.diagnostic == diagnostic &&
      other.delivered == delivered;

  @override
  int get hashCode => Object.hash(diagnostic, delivered);
}

/// What the whole screen says.
///
/// A value type for the reason given on [SetupCheck]: every change to any of
/// the four providers behind it yields a new object, and one that is never
/// equal rebuilds the whole screen and wakes `ShellScreen`'s listener with it
/// (`docs/UX.md` 7).
@immutable
class SetupState {
  /// Creates the state from its four rows.
  const SetupState({
    required this.checks,
    this.unmeasuredLines = 0,
    this.sandboxFailure,
  });

  /// The four rows, in the order of [SetupCheckKind].
  final List<SetupCheck> checks;

  /// The finding of the last `Sandbox` call that acted, while the snapshot of
  /// an earlier one still stands.
  ///
  /// It belongs to the gesture, not to a row, and for both of its two cases.
  /// A start that failed in transport says nothing about the folder, the
  /// machine or the model server: the daemon was stopped between two clicks,
  /// and every verdict this screen shows is as old and as true as it was a
  /// moment before. A start the daemon refused -- a guarantee that did not
  /// hold, a launcher that would not run -- says nothing about the folder
  /// either: `Sandbox(Plan)` measured that folder and said it was fine.
  /// Either way the screen anchors the finding under the start button, the
  /// control the gesture was made on and the one it stopped
  /// (`docs/UX.md` 4.4). Naming the project row instead sends somebody to
  /// repair a folder that was never wrong, and the picker they are sent to
  /// runs `Sandbox(Plan)`, which clears the finding and turns the row green
  /// again -- a dead end with a green mark on it.
  ///
  /// Null while no snapshot exists at all: then nobody ever measured, and the
  /// project row carries the failure itself as [SetupCheckState.unmeasured].
  final SetupCallFailure? sandboxFailure;

  /// How many of the lines behind the machine row carried no measurement.
  ///
  /// Counted over those lines alone, never over the whole report: the two
  /// lines with a row of their own are judged in that row, and the daemon's
  /// own line is never a measurement on any machine (see [machineLines]).
  /// The number stands on the button's line, so that somebody reads what was
  /// not looked at before they act on the list.
  final int unmeasuredLines;

  /// The row for [kind].
  SetupCheck operator [](SetupCheckKind kind) =>
      checks.firstWhere((SetupCheck check) => check.kind == kind);

  /// True when every one of the four rows is green.
  ///
  /// The contract of the specification, in one line: "all four green, then the
  /// button `Start agent` is on". A row that failed, one still being asked,
  /// one that was measured and is off, and one nobody could measure all keep
  /// it shut, and the sentence under the button names which one it is.
  bool get canStart => checks.every((SetupCheck check) => check.isGreen);

  /// True while a row carries an answer somebody has to act on: one failed, or
  /// one is still being asked.
  ///
  /// A different question from [canStart], asked by a different caller. The
  /// shell opens the setup section on this one, and it must not be the
  /// button's: the model row carries `DOCTOR_013` on every start nobody asked
  /// to probe, which is most of them, so a navigation switch on [canStart]
  /// would put the queue behind a checklist on every launch for ever. What
  /// opens the screen is a row that failed or one still being asked; what
  /// turns the button on is all four rows green.
  bool get needsSetup => checks.any(
    (SetupCheck check) =>
        check.state == SetupCheckState.failed ||
        check.state == SetupCheckState.checking,
  );

  /// The worst state among the rows.
  SetupCheckState get worst {
    SetupCheckState worst = SetupCheckState.ok;
    for (final SetupCheck check in checks) {
      if (check.state.index > worst.index) {
        worst = check.state;
      }
    }
    return worst;
  }

  @override
  bool operator ==(Object other) =>
      other is SetupState &&
      listEquals(checks, other.checks) &&
      other.unmeasuredLines == unmeasuredLines &&
      other.sandboxFailure == sandboxFailure;

  @override
  int get hashCode =>
      Object.hash(Object.hashAll(checks), unmeasuredLines, sandboxFailure);
}

/// What the shell knows about the connection, in the vocabulary of this
/// feature.
///
/// The connection state lives in `core/ipc/connection.dart` (ARCHITECTURE 5);
/// this feature could read it there, and does read three other files of
/// `core/ipc`. [DaemonLink] exists for a different reason: the row has one
/// question, "does the background service answer?", and `ConnectionStatus` has
/// four states, two of which are the same answer for this row and differ only
/// in what the shell draws around it. The shell composes the sections, so it
/// is the shell that collapses its own state into this one and hands it over
/// -- the same way it carries a flow from the history into the queue.
sealed class DaemonLink {
  /// Creates a link state.
  const DaemonLink();
}

/// `GetInfo` is in flight.
class DaemonLinkConnecting extends DaemonLink {
  /// Creates it.
  const DaemonLinkConnecting();
}

/// The daemon answered and speaks a contract this app can read.
class DaemonLinkUp extends DaemonLink {
  /// Creates it for [info].
  const DaemonLinkUp(this.info);

  /// What `GetInfo` said.
  final DaemonInfo info;
}

/// No usable daemon; [diagnostic] says why.
class DaemonLinkDown extends DaemonLink {
  /// Creates it for [diagnostic], with [retrying] while an attempt runs.
  const DaemonLinkDown(this.diagnostic, {this.retrying = false});

  /// Why the daemon is not usable, with its registered code.
  final Diagnostic diagnostic;

  /// True while an attempt somebody asked for is in flight.
  ///
  /// It rides on the failure rather than replacing it with
  /// [DaemonLinkConnecting], because the reason and its remedy must stay on
  /// the screen while somebody works on them: this is the screen they are
  /// repairing the service from (`docs/UX.md` 4.2, case 2, and 4.4). The
  /// background two-second attempt never sets it -- nobody asked for it, and
  /// a row that redrew itself every two seconds would flicker.
  final bool retrying;
}

/// Riverpod 3 retries a failed provider on its own and reports `AsyncLoading`
/// with the error tucked inside while it does. A screen whose whole job is to
/// say what is wrong must not show "asking" for ever instead of the reason;
/// the retry of this screen is the one the person or the connection timer
/// triggers.
Duration? noSetupRetry(int retryCount, Object error) => null;

/// The daemon's report about the machine (HUM-075).
///
/// One call per visit to the screen, not one per frame: it starts short
/// programs and reads files. It contacts nothing on the network -- `Doctor`
/// takes no argument in which a client could ask for a connection, so opening
/// this screen opens none.
@Riverpod(keepAlive: true, retry: noSetupRetry)
class SetupDoctor extends _$SetupDoctor {
  @override
  Future<DoctorReport> build() => ref.watch(daemonClientProvider).doctor();

  /// Asks again. Used by the retry control and after the daemon came back.
  Future<void> refresh() async {
    state = const AsyncLoading<DoctorReport>();
    state = await AsyncValue.guard(
      () => ref.read(daemonClientProvider).doctor(),
    );
  }
}

/// What the endpoint probe last answered, or nothing.
///
/// Nothing is the starting state and stays it until a person asks: the probe
/// is the one call of this application that reaches a machine on the network
/// (HUM-044).
class LlmProbeState {
  /// Creates a state.
  const LlmProbeState({this.probe, this.failure, this.busy = false});

  /// Nobody has asked yet.
  static const LlmProbeState idle = LlmProbeState();

  /// What came back, when something did.
  final LlmProbe? probe;

  /// Why nothing came back, when nothing did.
  final Diagnostic? failure;

  /// True while a probe is in flight.
  final bool busy;

  /// True while no probe has been made at all.
  bool get isIdle => probe == null && failure == null && !busy;
}

/// The endpoint probe, on request and never on a keystroke.
@Riverpod(keepAlive: true)
class SetupLlmProbe extends _$SetupLlmProbe {
  @override
  LlmProbeState build() => LlmProbeState.idle;

  /// Asks the daemon to contact [endpoint].
  ///
  /// This opens a connection to a machine on the network, so it happens only
  /// where a person asked for it: the button of the endpoint field and the
  /// `Enter` key in it, and nothing else.
  ///
  /// **Every throw ends this row, not only the expected one.** The caller does
  /// not await the future, so nothing else would ever see the error: the row
  /// would stay `checking` for good, the field and its button are shut while
  /// it does, and `forget()` does not reach it either, because there is
  /// nothing to forget. A row that can only be cleared by restarting the
  /// application is a dead end (`docs/UX.md` 4.4), and the conversion of the
  /// answer (`response.toDomain`) can throw a `StateError`, a
  /// `FormatException` or a `TimeoutException` that is no [DaemonException].
  /// An unexpected failure therefore becomes the finding of the transport,
  /// exactly as it does on the decision path (`decision.dart`).
  Future<void> run(String endpoint) async {
    state = const LlmProbeState(busy: true);
    try {
      final LlmProbe probe = await ref
          .read(daemonClientProvider)
          .probeLlm(endpoint);
      state = LlmProbeState(probe: probe);
    } on DaemonException catch (error) {
      state = LlmProbeState(failure: error.diagnostic);
    } on Object catch (error) {
      state = LlmProbeState(
        failure: ClientDiagnostics.daemonUnreachable(
          socketPath: '?',
          detail: error.toString(),
        ),
      );
    }
  }

  /// Forgets the last answer.
  ///
  /// Used when the text in the field no longer is the text that was probed: a
  /// result shown next to another address would claim to be about that one.
  void forget() => state = LlmProbeState.idle;
}

/// The four rows, folded out of what the daemon said.
///
/// A pure function on purpose: it is the one place where eleven daemon lines
/// and two stream answers become four rows, and it can be checked without a
/// widget, a socket or a machine.
SetupState setupChecks({
  required DaemonLink daemon,
  required AsyncValue<DoctorReport> doctor,
  required AsyncValue<SandboxStatus> sandbox,
  required SandboxCall sandboxCall,
  required LlmProbeState llm,
}) {
  final DoctorReport report = doctor.value ?? DoctorReport.empty;
  final DoctorCheck? llmLine = report[DoctorCheckId.llm];
  return SetupState(
    checks: <SetupCheck>[
      _daemonCheck(daemon),
      _llmCheck(llm, llmLine),
      _projectCheck(sandbox, sandboxCall),
      _sandboxCheck(doctor),
    ],
    unmeasuredLines: machineLines(report).where(_isBlind).length,
    sandboxFailure: _sandboxFailure(sandbox, sandboxCall),
  );
}

/// The failure of the last `Sandbox` call that acted, while the snapshot of an
/// earlier one still stands, or null.
///
/// Two roads lead here and they carry different sentences. The call that never
/// arrived is an error on the provider; the call that arrived and was refused
/// is a finding in the snapshot, and [sandboxCall] is what says that the
/// finding came from a gesture rather than from a plan.
///
/// Without a snapshot there is nothing this could stand beside: the project
/// row is `unmeasured` then and carries the same finding itself, and a second
/// copy under the button would say one thing twice.
SetupCallFailure? _sandboxFailure(
  AsyncValue<SandboxStatus> sandbox,
  SandboxCall sandboxCall,
) {
  if (!sandbox.hasValue) {
    return null;
  }
  if (sandbox.error case final Object error) {
    return SetupCallFailure(diagnostic: _diagnosticOf(error), delivered: false);
  }
  if (sandboxCall.isAboutTheFolder) {
    return null;
  }
  final Diagnostic? refusal = _worstFailure(
    sandbox.value?.diagnostics ?? const <Diagnostic>[],
  );
  return refusal == null
      ? null
      : SetupCallFailure(diagnostic: refusal, delivered: true);
}

/// The daemon row: only a client can say whether it reaches the daemon.
///
/// The doctor says so itself and sends its own `daemon` line as "not
/// measured"; this row replaces it, exactly as `humanitl doctor` does with
/// what its own connection attempt found (`cmd/doctor.rs`).
SetupCheck _daemonCheck(DaemonLink daemon) => switch (daemon) {
  DaemonLinkConnecting() => const SetupCheck(
    kind: SetupCheckKind.daemon,
    state: SetupCheckState.checking,
  ),
  DaemonLinkUp(:final DaemonInfo info) => SetupCheck(
    kind: SetupCheckKind.daemon,
    state: SetupCheckState.ok,
    detail: 'humanitld ${info.daemonVersion}, contract ${info.protoVersion}',
  ),
  // Ein Versuch läuft, und der Knopf, der ihn ausgelöst hat, ruht solange:
  // `DaemonCheck` schaltet ihn an genau diesem Zustand aus, damit niemand
  // einen zweiten Versuch auf einen laufenden setzt. Der Befund bleibt
  // trotzdem stehen, denn er ist das eine Wichtige dieses Bildschirms und
  // trägt die Abhilfe, die gerade läuft (`docs/UX.md` 4.2, Fall 2, und 4.4).
  DaemonLinkDown(:final Diagnostic diagnostic, retrying: true) => SetupCheck(
    kind: SetupCheckKind.daemon,
    state: SetupCheckState.checking,
    diagnostic: diagnostic,
  ),
  // Ohne Beleg: Die Karte unter der Zeile trägt Code, Grund und Vorschlag,
  // und eine Zeile darüber, die den Code noch einmal zeigt, sagt dasselbe
  // zweimal.
  DaemonLinkDown(:final Diagnostic diagnostic) => SetupCheck(
    kind: SetupCheckKind.daemon,
    state: SetupCheckState.failed,
    diagnostic: diagnostic,
  ),
};

/// The model row: the doctor's line until somebody asks for a probe.
///
/// Before a probe there is nothing measured, and the row says so rather than
/// showing an address as if it had answered. After a probe the verdict is the
/// severity the daemon put on its own findings -- `LLM_006` for an endpoint
/// outside a private network, `LLM_003` for an API nothing here knows -- and
/// never one decided here.
SetupCheck _llmCheck(LlmProbeState llm, DoctorCheck? line) {
  if (llm.busy) {
    return const SetupCheck(
      kind: SetupCheckKind.llm,
      state: SetupCheckState.checking,
    );
  }
  if (llm.failure case final Diagnostic failure) {
    return SetupCheck(
      kind: SetupCheckKind.llm,
      state: failure.isFailure ? SetupCheckState.failed : SetupCheckState.warn,
      diagnostic: failure,
    );
  }
  if (llm.probe case final LlmProbe probe) {
    final Diagnostic? first = probe.first;
    return SetupCheck(
      kind: SetupCheckKind.llm,
      state: switch (first) {
        null => SetupCheckState.ok,
        final Diagnostic finding when finding.isFailure =>
          SetupCheckState.failed,
        _ => SetupCheckState.warn,
      },
      diagnostic: first,
      detail: probe.endpoint,
    );
  }
  return _fromDoctorLine(SetupCheckKind.llm, line);
}

/// The project row: what a start with the chosen folder would do.
///
/// The verdict is `Sandbox(Plan)`'s: a finding the daemon called a failure
/// forbids the start, and which findings those are is decided in
/// `daemon/crates/ipc/src/sandbox.rs` (`SANDBOX_006` for a folder that cannot
/// be used, `SANDBOX_001` for a missing bubblewrap). Nothing about the folder
/// is judged here -- not whether it exists, not whether it is writable, not
/// where it lies.
///
/// The one thing this row does answer on its own is whether anybody named a
/// folder at all. That is not a measurement of the machine and needs none: an
/// empty `work_dir` is the plain fact that the choice has not been made, and
/// the row says so and stays out of green (HUM-044, diagnostic table).
///
/// # A call that failed is not a folder that vanished
///
/// Every gesture of this row goes through the same provider, and so does the
/// start button: `Sandbox(Plan)`, `Sandbox(Start)`, `Sandbox(Status)`. When
/// one of them fails in transport -- the daemon was stopped between two
/// clicks -- the provider carries an error **on top of** the snapshot it
/// already had. The folder in that snapshot is still the one the daemon has;
/// nothing about it changed because a later call did not arrive.
///
/// **So the snapshot alone decides this row, and the error does not touch
/// it.** Both halves of that matter, and they used to be in each other's way.
/// A row that read the error as its own verdict said "Project folder blocks
/// the start" and put `DAEMON_001` under it, which names the wrong thing at
/// the wrong place: the folder was fine, the daemon was gone
/// (`docs/UX.md` 4.4). A row that let the error erase the evidence made the
/// chosen folder disappear from the picker, which is worse still, because the
/// person then has to choose again what they already chose. The folder stays
/// visible and the row stays as true as its last measurement; the failure of
/// the call is reported where it happened, under the start button, as
/// [SetupState.sandboxFailure]. "Not measured" keeps the case it describes --
/// no snapshot at all, because nobody ever got an answer.
///
/// # And a start that was refused is not a folder that went bad either
///
/// The same holds one step further in, and it is the half that used to be
/// missing. The findings of every `Sandbox` call land in the same
/// `diagnostics` list, so a refused `Sandbox(Start)` -- a guarantee that did
/// not hold (`SANDBOX_013`), a launcher that would not run -- used to be read
/// here as the verdict on the folder. The row then said "Project folder blocks
/// the start" and put the start's finding under it, and the only control the
/// screen offered was the folder picker, whose `Sandbox(Plan)` clears the
/// findings and turns the row green again: somebody is sent to repair a folder
/// that was never wrong, and the repair looks like it worked
/// (`docs/UX.md` 4.4). [sandboxCall] is what separates the two: only the two
/// calls that read a plan answer about the folder, and only their findings are
/// ranked here.
SetupCheck _projectCheck(
  AsyncValue<SandboxStatus> sandbox,
  SandboxCall sandboxCall,
) {
  if (sandbox.isLoading && !sandbox.hasValue) {
    return const SetupCheck(
      kind: SetupCheckKind.project,
      state: SetupCheckState.checking,
    );
  }
  final SandboxStatus? status = sandbox.value;
  if (status == null) {
    // Kein Schnappschuss, nie einer gewesen: Hier hat wirklich niemand
    // gemessen, und der Grund ist der Fehler, an dem es scheiterte.
    return SetupCheck(
      kind: SetupCheckKind.project,
      state: SetupCheckState.unmeasured,
      diagnostic: switch (sandbox.error) {
        null => null,
        final Object error => _diagnosticOf(error),
      },
    );
  }
  final Diagnostic? planFailure = sandboxCall.isAboutTheFolder
      ? _worstFailure(status.diagnostics)
      : null;
  if (planFailure != null) {
    return SetupCheck(
      kind: SetupCheckKind.project,
      state: SetupCheckState.failed,
      diagnostic: planFailure,
      detail: status.workDirHost ?? '',
    );
  }
  final String? dir = status.workDirHost;
  if (dir == null || dir.isEmpty) {
    // Nobody chose a folder. That is a fact and not a gap in a measurement:
    // the agent works in exactly one folder, and until it is named there is
    // nothing to start. The finding comes from the client, like `DAEMON_001`:
    // the daemon answers with an empty folder and raises nothing, because an
    // open step of the setup is not a fault of the daemon.
    return SetupCheck(
      kind: SetupCheckKind.project,
      state: SetupCheckState.failed,
      diagnostic: ClientDiagnostics.noProjectFolder(),
    );
  }
  return SetupCheck(
    kind: SetupCheckKind.project,
    state: SetupCheckState.ok,
    detail: dir,
  );
}

/// The machine row: the doctor's lines, minus the two that have a row of
/// their own.
///
/// The fold is by rank and never by count: one failing line makes the row red
/// however many green ones stand beside it, and a line that carried no
/// measurement keeps the row out of green even when nothing failed. Each of
/// the eleven verdicts is the daemon's; what happens here is the ranking.
SetupCheck _sandboxCheck(AsyncValue<DoctorReport> doctor) {
  if (doctor.isLoading && !doctor.hasValue) {
    return const SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.checking,
    );
  }
  if (doctor.error case final Object error) {
    // No report at all. Not a green machine and not a broken one: nobody
    // looked, because the daemon that looks is not answering.
    final Diagnostic diagnostic = _diagnosticOf(error);
    return SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.unmeasured,
      diagnostic: diagnostic,
    );
  }
  final List<DoctorCheck> lines = machineLines(
    doctor.value ?? DoctorReport.empty,
  );
  if (lines.isEmpty) {
    return const SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.unmeasured,
    );
  }
  final DoctorCheck? failed = _firstWhere(
    lines,
    (DoctorCheck line) => line.status == DoctorStatus.fail,
  );
  if (failed != null) {
    return SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.failed,
      diagnostic: failed.diagnostic,
      detail: failed.evidence,
    );
  }
  final DoctorCheck? blind = _firstWhere(lines, _isBlind);
  if (blind != null) {
    return SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.unmeasured,
      diagnostic: blind.diagnostic,
      detail: blind.evidence,
    );
  }
  final DoctorCheck? warned = _firstWhere(
    lines,
    (DoctorCheck line) => line.status == DoctorStatus.warn,
  );
  if (warned != null) {
    return SetupCheck(
      kind: SetupCheckKind.sandbox,
      state: SetupCheckState.warn,
      diagnostic: warned.diagnostic,
      detail: warned.evidence,
    );
  }
  return SetupCheck(
    kind: SetupCheckKind.sandbox,
    state: SetupCheckState.ok,
    detail: lines.first.evidence,
  );
}

/// The lines the machine row folds: the report without the two that have a
/// row of their own.
///
/// Public because the heading over the folded list counts the same lines. Two
/// filters over one report drift apart -- the heading said eleven while the
/// list drew nine -- and this is the one place the selection is made.
///
/// The daemon's own line is the reason this filter also decides the count on
/// the button. A daemon cannot measure from the inside whether a client
/// reaches its socket, so it reports that line as not tried on every machine,
/// including one where everything else was measured. Counted over the whole
/// report, the screen would say "a check could not be measured" for ever and
/// never reach the sentence that says the opposite. `humanitl doctor` replaces
/// the same line with what its own connection attempt found (`cmd/doctor.rs`),
/// and the first row of this screen does the same.
List<DoctorCheck> machineLines(DoctorReport report) => <DoctorCheck>[
  for (final DoctorCheck line in report.checks)
    if (!DoctorCheckId.ownRow.contains(line.id)) line,
];

/// True when [line] carries no measurement, for either of the three reasons.
///
/// The two the contract names (`DOCTOR_012`, `DOCTOR_013`), and a status this
/// build cannot read -- a newer daemon may say something in a word this
/// version does not know, and the one answer that must not be guessed is
/// "fine".
bool _isBlind(DoctorCheck line) =>
    !line.isMeasured || line.status == DoctorStatus.unknown;

/// One row built straight out of one line of the doctor.
SetupCheck _fromDoctorLine(SetupCheckKind kind, DoctorCheck? line) {
  if (line == null) {
    return SetupCheck(kind: kind, state: SetupCheckState.unmeasured);
  }
  return SetupCheck(
    kind: kind,
    state: switch (line.status) {
      DoctorStatus.fail => SetupCheckState.failed,
      DoctorStatus.unknown => SetupCheckState.unmeasured,
      DoctorStatus.warn when !line.isMeasured => SetupCheckState.unmeasured,
      DoctorStatus.warn => SetupCheckState.warn,
      DoctorStatus.ok => SetupCheckState.ok,
    },
    diagnostic: line.diagnostic,
    detail: line.evidence,
  );
}

/// The gravest finding that forbids a start, or null when none does.
///
/// `SandboxStatus.blocking` answers for [Severity.blocking] alone. A finding
/// the daemon called an error is a failure too (`Diagnostic.isFailure`), and a
/// row that stayed green over one would say the folder is in order while the
/// daemon said the opposite. The ranking is the severity the daemon put on the
/// finding, so the card under the row shows the gravest of them.
Diagnostic? _worstFailure(List<Diagnostic> diagnostics) {
  Diagnostic? worst;
  for (final Diagnostic diagnostic in diagnostics) {
    if (!diagnostic.isFailure) {
      continue;
    }
    if (worst == null || diagnostic.severity.index > worst.severity.index) {
      worst = diagnostic;
    }
  }
  return worst;
}

/// The first element [test] accepts, or null.
DoctorCheck? _firstWhere(
  List<DoctorCheck> lines,
  bool Function(DoctorCheck) test,
) {
  for (final DoctorCheck line in lines) {
    if (test(line)) {
      return line;
    }
  }
  return null;
}

/// The diagnostic behind an error that reached a provider.
Diagnostic _diagnosticOf(Object error) => switch (error) {
  DaemonException(:final Diagnostic diagnostic) => diagnostic,
  _ => Diagnostic(
    code: DiagnosticCodes.daemonUnreachable,
    severity: Severity.error,
    why: error.toString(),
  ),
};
