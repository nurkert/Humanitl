/// What the agent gets: the mirror of `SandboxEvent.Status` and its parts.
///
/// The sandbox screen answers one question -- does the agent get my whole
/// disk? -- and every answer on it comes from here, which means it comes from
/// the daemon. The application does not derive a mount, does not guess a
/// value and does not shorten a command line: [SandboxStatus.argvPreview] is
/// the exact line that starts, and [SandboxStatus.mounts] is read from that
/// same line inside the daemon (ADR-018, HUM-040).
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'diagnostic.dart';
import 'ids.dart';

part 'sandbox.freezed.dart';

/// The lifecycle of the sandbox, mirror of `SandboxState`.
enum SandboxState {
  /// Nothing runs; the screen shows what a start would do.
  stopped,

  /// The start is under way.
  starting,

  /// The sandbox runs.
  running,

  /// The stop is under way.
  stopping,

  /// The start failed; the diagnostic says why.
  failed,
}

/// Whether the project directory is mounted writable, mirror of `WorkMode`.
enum WorkMode {
  /// Read only.
  ro,

  /// Read and write.
  rw;

  /// The wire form, `ro` or `rw`.
  String get wire => name;

  /// The mode [wire] names, or [WorkMode.rw] for anything else.
  static WorkMode fromWire(String wire) => switch (wire.trim().toLowerCase()) {
    'ro' => WorkMode.ro,
    _ => WorkMode.rw,
  };
}

/// How a path is mounted, mirror of `MountMode`.
enum MountMode {
  /// The host path, read only (`--ro-bind`).
  ro,

  /// The host path, writable (`--bind`).
  rw,

  /// An empty in-memory filesystem; nothing of the host (`--tmpfs`).
  tmpfs,

  /// A file out of the daemon's memory, usually empty: this is how a path is
  /// masked and how the agent's own files are placed (`--ro-bind-data`).
  masked,

  /// A `procfs` of this PID namespace (`--proc`).
  proc,

  /// The minimal device filesystem of bubblewrap (`--dev`).
  dev,

  /// A link, not a mount (`--symlink`).
  symlink,
}

/// Where a mount or an environment variable comes from, mirror of
/// `ValueOrigin`.
enum ValueOrigin {
  /// From the sandbox profile.
  profile,

  /// Contributed by the agent adapter.
  adapter,

  /// From the session: project directory, proxy socket, CA, shim.
  session,

  /// An extension the person wrote into their own profile
  /// (`mounts.extra_ro`, `mounts.extra_rw`). Everything of this origin is an
  /// exception to the sentence "the agent sees only /work".
  user,
}

/// One path the agent sees inside the sandbox.
@freezed
abstract class MountEntry with _$MountEntry {
  /// Creates a mount.
  const factory MountEntry({
    required String dst,
    @Default('') String src,
    required MountMode mode,
    @Default(ValueOrigin.profile) ValueOrigin origin,
    @Default('') String linkTarget,
  }) = _MountEntry;

  const MountEntry._();

  /// True where there is a path on this machine behind this entry, so an empty
  /// [src] is the truth and not a missing value.
  ///
  /// A link is not one of them: its target lies inside the sandbox and is in
  /// [linkTarget]. The column that shows [src] is labelled "on this machine",
  /// and a sandbox path under that heading would be a false statement.
  bool get hasHostPath => src.isNotEmpty;
}

/// One environment variable the sandbox sets.
@freezed
abstract class EnvEntry with _$EnvEntry {
  /// Creates an environment entry.
  const factory EnvEntry({
    required String key,
    @Default('') String value,
    @Default(ValueOrigin.profile) ValueOrigin origin,
    @Default(false) bool withheld,
  }) = _EnvEntry;

  const EnvEntry._();

  /// True when the daemon kept the value to itself.
  ///
  /// It keeps every value that is not on its own short list of evidence --
  /// proxy, certificates, paths, language, and what the adapter sets to steer
  /// the agent. The question the daemon answers is not "is this a secret?",
  /// which cannot be answered from a name, but "do I need this as proof?",
  /// which can (`daemon/crates/ipc/src/sandbox.rs`, `VISIBLE_ENV`).
  ///
  /// A withheld value and an empty value must never look alike: the screen
  /// draws dots for this one and the word for "empty" for the other, and this
  /// getter is what separates them.
  bool get isMasked => withheld;

  /// True when the variable really carries no value.
  bool get isEmpty => !withheld && value.isEmpty;
}

/// One of the three guarantees, mirror of `IsolationCheck`.
///
/// The order is the order of the wire enum and the order the panel draws
/// them in; it is also the order the daemon folds the shim's five report
/// lines into (`daemon/crates/sandbox/src/bwrap.rs`).
enum IsolationCheck {
  /// There is no network interface but `lo`, so there is nowhere to go.
  noNetworkInterface,

  /// Exactly one socket leads out of the sandbox, and it leads to Humanitl.
  singleSocket,

  /// The seccomp filter is in force in the agent's process.
  seccompActive,
}

/// What one guarantee looks like right now.
///
/// A missing result is its own state and never the colour of a passed one:
/// nothing measured is not the same as measured and good (CONVENTIONS 4.13).
enum IsolationSegment {
  /// Nothing was measured. Not a claim in either direction.
  unknown,

  /// The sandbox is starting and the result has not arrived yet.
  running,

  /// Measured, and the guarantee holds.
  passed,

  /// Measured, and the guarantee does not hold.
  failed,
}

/// One measured guarantee, mirror of `CheckResult`.
///
/// [evidence] is what was actually measured, in the shim's own words -- the
/// interfaces it found, the sockets it walked, the `Seccomp:` line it read.
/// The panel shows it next to the sentence, because a green dot without the
/// line under it is decoration and a green dot with it is an argument.
@freezed
abstract class IsolationCheckResult with _$IsolationCheckResult {
  /// Creates a result.
  const factory IsolationCheckResult({
    required IsolationCheck check,
    required bool passed,
    @Default('') String evidence,
    Diagnostic? diagnostic,
  }) = _IsolationCheckResult;

  const IsolationCheckResult._();

  /// True when the shim's socket walk stopped at its budget instead of
  /// finishing.
  ///
  /// The walk runs to depth [socketWalkDepth] over at most
  /// [socketWalkEntries] entries and writes `limit=none|entries|depth` into
  /// its own line. Anything but `none` means the walk ran out before it was
  /// done: still a check, no longer a proof, and the panel has to say so
  /// rather than show smooth green (CONVENTIONS 4.13). The exhaustive proof
  /// stays ESC-2.
  ///
  /// This is the one place the application reads a field out of the evidence
  /// instead of out of a message field. The wire has no room for it, and the
  /// alternative -- letting a truncated walk look like a finished one -- is
  /// the one thing this screen must not do.
  bool get walkStopped =>
      evidence.contains('$socketWalkLimitKey=entries') ||
      evidence.contains('$socketWalkLimitKey=depth');

  /// The key the shim writes its walk budget under.
  static const String socketWalkLimitKey = 'limit';

  /// How deep the shim's socket walk goes (`SOCKET_WALK_MAX_DEPTH`).
  static const int socketWalkDepth = 3;

  /// How many entries it visits at most (`SOCKET_WALK_MAX_ENTRIES`).
  static const int socketWalkEntries = 2000;
}

/// Everything the sandbox screen shows, mirror of `SandboxEvent.Status`.
@freezed
abstract class SandboxStatus with _$SandboxStatus {
  /// Creates a snapshot.
  const factory SandboxStatus({
    @Default(SandboxState.stopped) SandboxState state,
    SessionId? sessionId,
    SandboxId? sandboxId,
    DateTime? startedAt,
    @Default('') String profile,
    @Default('bwrap') String backend,
    @Default('') String llmEndpoint,
    String? workDirHost,
    @Default(WorkMode.rw) WorkMode workMode,
    @Default(<MountEntry>[]) List<MountEntry> mounts,
    @Default(<EnvEntry>[]) List<EnvEntry> env,
    @Default('') String argvPreview,
    @Default(false) bool agentRunning,
    @Default(<Diagnostic>[]) List<Diagnostic> diagnostics,
    @Default(<IsolationCheckResult>[]) List<IsolationCheckResult> checks,
    SandboxId? checksSandboxId,
  }) = _SandboxStatus;

  const SandboxStatus._();

  /// True while a start or a stop is under way; both controls rest then.
  bool get isBusy =>
      state == SandboxState.starting || state == SandboxState.stopping;

  /// True while the sandbox is up, whatever runs inside it.
  bool get isUp => state == SandboxState.running;

  /// True when the sandbox is up but nothing runs inside it any more.
  ///
  /// Stopping then takes nothing away, so it does not ask (HUM-040).
  bool get agentExited => isUp && !agentRunning;

  /// The blocking diagnostic that forbids a start, if there is one.
  Diagnostic? get blocking {
    for (final Diagnostic diagnostic in diagnostics) {
      if (diagnostic.severity == Severity.blocking) {
        return diagnostic;
      }
    }
    return null;
  }

  /// The result for [check], or null when none arrived.
  ///
  /// Null is an answer: nothing was measured. It is never folded into a
  /// passed result anywhere on the way to the screen.
  IsolationCheckResult? checkFor(IsolationCheck check) {
    for (final IsolationCheckResult result in checks) {
      if (result.check == check) {
        return result;
      }
    }
    return null;
  }

  /// What one guarantee looks like right now.
  ///
  /// While the sandbox is starting a missing result means "not yet"; at every
  /// other moment it means "not measured". Neither is [IsolationSegment.passed].
  IsolationSegment segmentFor(IsolationCheck check) {
    final IsolationCheckResult? result = checkFor(check);
    if (result != null) {
      return result.passed ? IsolationSegment.passed : IsolationSegment.failed;
    }
    return state == SandboxState.starting
        ? IsolationSegment.running
        : IsolationSegment.unknown;
  }

  /// [next] with the results this snapshot may carry into it.
  ///
  /// **A result belongs to one run of one sandbox and to nothing else.** It is
  /// as worthless for the next run as no result at all -- the same lie as
  /// "nothing measured looks like measured and good", only along the time
  /// axis. Three things end that ownership:
  ///
  /// - **A start.** `starting` means this run has measured nothing yet. A
  ///   green dot left over from the last run, over a sandbox that is still
  ///   coming up, claims a guarantee nobody checked in it.
  /// - **A stop.** Nothing runs, so nothing is proven.
  /// - **Another sandbox id.** The daemon is answering about a different run
  ///   -- started from the command line, or in a second window -- and a
  ///   result from run A says nothing about run B.
  ///
  /// The results arrive between `Status(starting)` and the status that ends
  /// the start, and only that status carries the id of the run they were
  /// measured in ([SandboxEvent.Status.sandbox_id] is empty while nothing is
  /// launched). Leaving `starting` is therefore the one moment at which the
  /// id is written down ([checksSandboxId]); from then on it is compared, not
  /// adopted. A run that ended before it ever had an id -- a start the daemon
  /// stopped over a red guarantee -- keeps its results under a null id, and
  /// the next start clears them at `starting` before anything else can.
  SandboxStatus carryChecksInto(SandboxStatus next) {
    if (checks.isEmpty) {
      return next;
    }
    final bool aRunIsUp =
        next.state == SandboxState.running || next.state == SandboxState.failed;
    if (!aRunIsUp) {
      return next;
    }
    if (state == SandboxState.starting) {
      return next.copyWith(checks: checks, checksSandboxId: next.sandboxId);
    }
    if (checksSandboxId == next.sandboxId) {
      return next.copyWith(checks: checks, checksSandboxId: checksSandboxId);
    }
    return next;
  }

  /// How many of the three guarantees are proven right now.
  int get checksPassed =>
      checks.where((IsolationCheckResult result) => result.passed).length;

  /// The first guarantee that was measured and did not hold, if there is one.
  IsolationCheckResult? get failedCheck {
    for (final IsolationCheckResult result in checks) {
      if (!result.passed) {
        return result;
      }
    }
    return null;
  }

  /// True when all three guarantees were measured and all three hold.
  ///
  /// Counted against [IsolationCheck.values], not against the length of
  /// [checks]: two green results out of two received are not three out of
  /// three, and the ring must not close on them.
  bool get isolationProven =>
      checksPassed == IsolationCheck.values.length && failedCheck == null;

  /// The mounts that carry a host path, in the order of the command line.
  List<MountEntry> get hostMounts =>
      mounts.where((MountEntry mount) => mount.hasHostPath).toList();

  /// The paths the person added to their own profile beyond `/work`.
  ///
  /// The sentence in the mounts tab claims that the agent sees only `/work`.
  /// Whatever is in this list is an exception to that claim and is named next
  /// to it; a claim with a silent exception is worse than no claim.
  List<MountEntry> get extraHostPaths => mounts
      .where((MountEntry mount) => mount.origin == ValueOrigin.user)
      .toList();

  /// The project mount, the one the sentence is about.
  MountEntry? get workMount {
    for (final MountEntry mount in mounts) {
      if (mount.origin == ValueOrigin.session && mount.hasHostPath) {
        if (mount.mode == MountMode.ro || mount.mode == MountMode.rw) {
          return mount;
        }
      }
    }
    return null;
  }
}

/// One line the daemon logged about the sandbox, mirror of
/// `SandboxEvent.LogLine`.
@freezed
abstract class SandboxLogLine with _$SandboxLogLine {
  /// Creates a log line.
  const factory SandboxLogLine({required DateTime at, required String text}) =
      _SandboxLogLine;
}

/// One event of the `Sandbox` stream.
///
/// The screen never folds these itself into something the daemon did not say:
/// a status replaces the snapshot, a diagnostic is added to it, a log line
/// goes to the log, and an argv line is one word of the command.
@freezed
sealed class SandboxUpdate with _$SandboxUpdate {
  /// A new snapshot.
  const factory SandboxUpdate.status(SandboxStatus status) =
      SandboxUpdateStatus;

  /// Something went wrong, with cause and remedy.
  const factory SandboxUpdate.diagnostic(Diagnostic diagnostic) =
      SandboxUpdateDiagnostic;

  /// A line for the log tab.
  const factory SandboxUpdate.log(SandboxLogLine line) = SandboxUpdateLog;

  /// One argument of the command line.
  const factory SandboxUpdate.argvLine(String line) = SandboxUpdateArgvLine;

  /// One measured guarantee.
  const factory SandboxUpdate.check(IsolationCheckResult result) =
      SandboxUpdateCheck;
}
