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
}
