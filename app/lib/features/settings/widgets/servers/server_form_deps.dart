/// What a server's form needs to reach the daemon, and the rule of a
/// server name (HUM-414).
library;

import '../../../../core/ipc/config_providers.dart';
import '../../../../core/ipc/daemon_client.dart';
import '../../providers/server_checks.dart';

/// The rule of a server name, the same as the daemon's: 1 to 32 of `a-z`,
/// `0-9`, `-` and `_`.
final RegExp serverNamePattern = RegExp(r'^[a-z0-9_-]{1,32}$');

/// What the form needs to reach the daemon; handed in per gesture, so the
/// form holds no provider of its own.
class ServerFormDeps {
  /// Bundles the daemon client and the notifiers a gesture writes through.
  const ServerFormDeps({
    required this.client,
    required this.servers,
    required this.keys,
    required this.checks,
  });

  /// For `ProbeLlm` and `LlmKeyStatus`.
  final DaemonClient client;

  /// `LlmServers`: add, edit, rename, store a key.
  final LlmServersSetting servers;

  /// `LlmKey`: remove a key.
  final LlmKeySetting keys;

  /// The last check of each server in this app session.
  final ServerChecks checks;
}
