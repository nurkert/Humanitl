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

  /// A `Rules` request cannot be carried out: no operation, an unreadable
  /// rule, or an id no rule carries.
  static const String rulesRequestInvalid = 'IPC_005';

  /// A bundled rule was to be changed or deleted. Bundled rules do not belong
  /// to the person; a rule of their own goes in front of one.
  static const String ruleBundled = 'RULES_010';

  /// `rules.yaml` could not be read. The `why` names the field and, where the
  /// parser found one, the line.
  static const String rulesFileInvalid = 'RULES_001';

  /// A host pattern is not a host, not a glob and not an address.
  static const String hostPatternInvalid = 'RULES_003';

  /// Two rules carry the same id. Every rule needs its own.
  static const String ruleIdDuplicate = 'RULES_007';

  /// A path pattern is neither a glob the engine can build nor a regular
  /// expression it accepts.
  static const String pathPatternInvalid = 'RULES_005';

  /// The rule set was read again. Carries what changed, and is information,
  /// not a failure.
  static const String rulesReloaded = 'RULES_011';

  /// A `Decide` request cannot be carried out as it stands. Raised by the
  /// client where it can see that itself, so that nothing is sent that would
  /// be refused or, worse, carried out unasked.
  static const String decideRequestInvalid = 'IPC_004';

  /// This desktop has no tray to register with. Information, not a failure:
  /// the count still stands in the window title.
  static const String noTray = 'UI_002';
}
