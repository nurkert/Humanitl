/// The daemon's self-description, mirror of `Info` in `humanitl.proto`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

import 'intercept.dart';

part 'daemon_info.freezed.dart';
part 'daemon_info.g.dart';

/// Version, protocol and capabilities of the connected daemon.
@freezed
abstract class DaemonInfo with _$DaemonInfo {
  /// Creates a daemon description.
  const factory DaemonInfo({
    required String daemonVersion,
    required int protoMajor,
    required int protoMinor,
    @Default(<String>[]) List<String> capabilities,
    @Default('') String sessionId,

    /// The pause of intercepting as this daemon reports it with every
    /// `GetInfo`, so with every heartbeat (HUM-407); null from a daemon that
    /// cannot pause. Not part of the JSON form: it is a reading, not a
    /// setting.
    @JsonKey(includeFromJson: false, includeToJson: false)
    InterceptStatus? intercept,
  }) = _DaemonInfo;

  const DaemonInfo._();

  /// Reads a description from JSON.
  factory DaemonInfo.fromJson(Map<String, Object?> json) =>
      _$DaemonInfoFromJson(json);

  /// The capability the fake daemon announces (`fixtures/sessions/README.md`).
  static const String fakeCapability = 'fake';

  /// True when the daemon replays a recorded session instead of proxying.
  bool get isFake => capabilities.contains(fakeCapability);

  /// The capability of a daemon that opens further shells in a running
  /// sandbox (`Exec`, HUM-299).
  static const String execCapability = 'sandbox.exec';

  /// True when the daemon opens further shells next to the agent; the
  /// terminal pane offers "+ Shell" only then (HUM-314).
  bool get offersShells => capabilities.contains(execCapability);

  /// The capability of a daemon that can run a disposable sandbox: the
  /// project folder under a separate layer, reviewed and applied afterwards
  /// (HUM-350). A daemon on macOS does not name it.
  static const String disposableCapability = 'sandbox.disposable';

  /// True when the daemon runs disposable sandboxes; the app offers the
  /// switch, the setting and the changes panel only then (HUM-351).
  bool get offersDisposable => capabilities.contains(disposableCapability);

  /// The capability of a daemon whose intercepting can be paused over
  /// `Intercept` (HUM-407).
  static const String interceptPauseCapability = 'intercept.pause';

  /// True when the daemon can pause intercepting; only then is the intercept
  /// pill of the header a button.
  bool get offersInterceptPause =>
      capabilities.contains(interceptPauseCapability);

  /// True when the daemon answers `Egress` and reports the way out of the
  /// agent's traffic: contract 1.41 or later, or the fake, which plays it
  /// (HUM-511). The settings of the decision proxy stand only then.
  bool get offersEgress => isFake || (protoMajor == 1 && protoMinor >= 41);

  /// Whether the daemon decides cloud inference before a start (contract
  /// 1.43, HUM-516): it names the provider in the snapshot and refuses an
  /// unconfirmed start. An older one does neither, so the app starts no
  /// session against it.
  bool get knowsCloudInference =>
      isFake || (protoMajor == 1 && protoMinor >= 43);

  /// Whether a start can name a project by its id (`Start.project_id`,
  /// contract 1.58, HUM-537). An older daemon ignores the field and would
  /// start in the stored folder; the app then sends the folder instead.
  bool get startsByProjectId => isFake || (protoMajor == 1 && protoMinor >= 58);

  /// True when a session is running.
  bool get hasSession => sessionId.isNotEmpty;

  /// `major.minor` of the protocol the daemon speaks.
  String get protoVersion => '$protoMajor.$protoMinor';
}
