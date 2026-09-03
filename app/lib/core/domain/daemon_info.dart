/// The daemon's self-description, mirror of `Info` in `humanitl.proto`.
library;

import 'package:freezed_annotation/freezed_annotation.dart';

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
  }) = _DaemonInfo;

  const DaemonInfo._();

  /// Reads a description from JSON.
  factory DaemonInfo.fromJson(Map<String, Object?> json) =>
      _$DaemonInfoFromJson(json);

  /// The capability the fake daemon announces (`fixtures/sessions/README.md`).
  static const String fakeCapability = 'fake';

  /// True when the daemon replays a recorded session instead of proxying.
  bool get isFake => capabilities.contains(fakeCapability);

  /// True when a session is running.
  bool get hasSession => sessionId.isNotEmpty;

  /// `major.minor` of the protocol the daemon speaks.
  String get protoVersion => '$protoMajor.$protoMinor';
}
