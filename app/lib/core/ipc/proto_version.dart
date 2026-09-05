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

  /// Minor version. Informational: minors are additive, so an app with an
  /// older minor keeps working against a newer daemon and simply does not
  /// read the newer fields. Raised to 1 with the rule test operation, the
  /// truncated-findings flag and the domain info (HUM-023 to HUM-031), to 2
  /// with the endpoint probe (HUM-039), to 3 with the sandbox snapshot —
  /// mounts, environment, command line and the `Plan` operation (HUM-040) —
  /// and to 5 with `FlowSummary.meta`, the mark on a request the proxy
  /// answered itself, which the history reads (HUM-103). Minor 4 is skipped
  /// on this side on purpose: the five fields of the per-session start
  /// (HUM-067) have no reader in the app yet. Minor 6 is skipped for the same
  /// reason: the session summary of a sandbox run — the diff over the project,
  /// the secret scan across it and the symlinks leaving it (HUM-043) — arrives
  /// on the wire and has no reader here until the sheet that shows it is
  /// built. Raising the number without a reader would claim otherwise.
  static const int minor = 5;

  /// `major.minor` as text.
  static const String text = '$major.$minor';

  /// Metadata key that carries the session token on every call.
  static const String tokenMetadataKey = 'x-humanitl-token';

  /// True when a daemon with [daemonMajor] can be talked to.
  static bool isCompatible(int daemonMajor) => daemonMajor == major;
}
