/// The protocol version this app speaks, mirror of `PROTO_MAJOR`,
/// `PROTO_MINOR` and `TOKEN_METADATA_KEY` in `humanitl-ipc`.
///
/// The daemon reports its own pair in `Info`; a different major means the two
/// sides disagree about the shape of the contract and the app refuses to go on
/// (`DAEMON_002`). A different minor is fine: minors are additive.
library;

/// Version constants of the `humanitl.v1` contract.
abstract final class ProtoVersion {
  /// Major version. Must equal `Info.proto_major`.
  static const int major = 1;

  /// Minor version. Informational.
  static const int minor = 0;

  /// `major.minor` as text.
  static const String text = '$major.$minor';

  /// Metadata key that carries the session token on every call.
  static const String tokenMetadataKey = 'x-humanitl-token';

  /// True when a daemon with [daemonMajor] can be talked to.
  static bool isCompatible(int daemonMajor) => daemonMajor == major;
}
