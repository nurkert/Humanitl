/// Updates of Humanitl itself, as the daemon reports them (HUM-382).
///
/// Plain value types and no freezed, like the settings: read-only views of an
/// answer of the daemon. The daemon decides everything -- whether an update
/// exists, whether this installation can take it, whether a signature holds;
/// the app shows what it says (ADR-018).
library;

import 'diagnostic.dart';

/// What the daemon knows about updates, mirror of `UpdateStatus`.
final class UpdateStatus {
  /// Creates a status.
  const UpdateStatus({
    this.currentVersion = '',
    this.latestVersion = '',
    this.available = false,
    this.canApply = false,
    this.checkedAt,
    this.size = 0,
    this.notice,
    this.method,
    this.upgradeCommand = const <String>[],
  });

  /// The version of the daemon.
  final String currentVersion;

  /// The version of the latest release by its signed manifest; empty while
  /// nobody asked.
  final String latestVersion;

  /// True when [latestVersion] is newer than [currentVersion].
  final bool available;

  /// True when `Apply` can install here (macOS, from a replaceable bundle).
  final bool canApply;

  /// When the daemon last asked successfully; null when never.
  final DateTime? checkedAt;

  /// The size of the zip in bytes; 0 when nothing is offered.
  final int size;

  /// Why [canApply] is false, with the command to copy.
  final Diagnostic? notice;

  /// How this installation gets an update; null for a way this build does
  /// not know.
  final UpdateMethod? method;

  /// For [UpdateMethod.apt] and [UpdateMethod.dnf] the fixed call the app
  /// starts in the person's session (`pkexec` needs their polkit agent);
  /// empty otherwise.
  final List<String> upgradeCommand;
}

/// How an installation gets an update, in the order of `UpdateMethod` on the
/// wire.
enum UpdateMethod {
  /// macOS: the signed zip replaces `Humanitl.app` (`Apply`).
  appBundle,

  /// Linux, an unpacked archive in a folder of the user (`Apply`).
  archive,

  /// Linux, an AppImage in a folder of the user (`Apply`).
  appimage,

  /// A package from the apt repository: `upgradeCommand` through pkexec.
  apt,

  /// A package from the dnf repository: `upgradeCommand` through pkexec.
  dnf,

  /// No way from inside the app; the notice names the command to copy.
  manual,
}

/// The stages of an `Apply`, in the order of `UpdateStage` on the wire.
enum UpdateStage {
  /// The zip is being downloaded.
  downloading,

  /// Unpacking and checking signature and version of the bundle.
  verifying,

  /// The old app is being exchanged for the new one.
  installing,
}

/// How far an `Apply` is.
final class UpdateProgress {
  /// Creates a progress report.
  const UpdateProgress({
    required this.stage,
    this.doneBytes = 0,
    this.totalBytes = 0,
  });

  /// The stage; null for a stage this build does not know.
  final UpdateStage? stage;

  /// Bytes downloaded so far.
  final int doneBytes;

  /// Bytes in total; 0 when unknown.
  final int totalBytes;

  /// Whole percent of the download, or null when the total is unknown.
  int? get percent =>
      totalBytes <= 0 ? null : ((doneBytes * 100) ~/ totalBytes).clamp(0, 100);
}

/// One event of an `Update` call.
sealed class UpdateEvent {
  const UpdateEvent();
}

/// The daemon's status, the one answer of `Check`.
final class UpdateEventStatus extends UpdateEvent {
  /// Creates the event.
  const UpdateEventStatus(this.status);

  /// The status.
  final UpdateStatus status;
}

/// Progress of an `Apply`.
final class UpdateEventProgress extends UpdateEvent {
  /// Creates the event.
  const UpdateEventProgress(this.progress);

  /// The progress.
  final UpdateProgress progress;
}

/// The new app lies in place; the last event of a successful `Apply`.
final class UpdateEventInstalled extends UpdateEvent {
  /// Creates the event.
  const UpdateEventInstalled({required this.version, required this.appPath});

  /// The installed version.
  final String version;

  /// The path of the bundle, for example `/Applications/Humanitl.app`.
  final String appPath;
}
