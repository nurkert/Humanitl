/// The diagnostic codes the app itself raises or matches on. Every constant
/// is a registered code from `daemon/crates/core-types/src/diagnostics/
/// codes.rs`; the app never invents one (CONVENTIONS 4.6).
library;

/// Registered codes used on the client side.
abstract final class DiagnosticCodes {
  /// The daemon is not running or its socket does not answer.
  static const String daemonUnreachable = 'DAEMON_001';

  /// Client and daemon speak different majors of the contract.
  static const String protoIncompatible = 'DAEMON_002';

  /// The token from the runtime directory is missing or does not match.
  static const String tokenInvalid = 'IPC_001';

  /// `AllowEdited` for more than one flow.
  static const String allowEditedSingle = 'IPC_002';

  /// The flow is no longer held; the decision came too late.
  static const String flowNotHeld = 'IPC_003';
}
