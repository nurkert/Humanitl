/// Saved model servers, mirror of `LlmServer` and `LlmServerList` (HUM-289).
///
/// An entry is an endpoint, its models and a reference to its access key,
/// never the key: the daemon does not send it, and these types have no field
/// for it. The active entry is what `llm.endpoint`, `llm.models` and
/// `llm.api_key_ref` show; only its origin gets the stored key, and a switch
/// applies to the next session.
library;

import 'diagnostic.dart';

/// One saved server.
final class SavedServer {
  /// Creates an entry.
  const SavedServer({
    required this.name,
    this.endpoint = '',
    this.models = const <String>[],
    this.keyRef = '',
    this.active = false,
    this.profiles = const <String>[],
  });

  /// The name: 1 to 32 characters `a-z`, `0-9`, `-` and `_`.
  final String name;

  /// The endpoint as it stands in the configuration.
  final String endpoint;

  /// Its models, the default one first.
  final List<String> models;

  /// The reference to its access key, such as
  /// `secret-service:humanitl/cloud` or `keychain:humanitl/cloud`; empty
  /// without a key.
  final String keyRef;

  /// True for the entry `llm.active` names.
  final bool active;

  /// The session profiles whose `llm.server` names this entry, sorted by
  /// name (HUM-400). Projects are not listed: the daemon only knows the
  /// project it is starting.
  final List<String> profiles;

  /// The default model, or empty.
  String get model => models.isEmpty ? '' : models.first;

  /// True when a key reference is configured for this server.
  bool get hasKey => keyRef.isNotEmpty;

  @override
  bool operator ==(Object other) =>
      other is SavedServer &&
      other.name == name &&
      other.endpoint == endpoint &&
      _same(other.models, models) &&
      other.keyRef == keyRef &&
      other.active == active &&
      _same(other.profiles, profiles);

  @override
  int get hashCode => Object.hash(
    name,
    endpoint,
    Object.hashAll(models),
    keyRef,
    active,
    Object.hashAll(profiles),
  );
}

/// The saved servers after a request.
final class SavedServers {
  /// Creates a list.
  const SavedServers({
    this.servers = const <SavedServer>[],
    this.active = '',
    this.implicit = false,
    this.diagnostics = const <Diagnostic>[],
  });

  /// No server saved.
  static const SavedServers none = SavedServers();

  /// The entries, sorted by name.
  final List<SavedServer> servers;

  /// The name of the active entry; empty when none is active.
  final String active;

  /// True while the configuration holds only the flat keys and the list
  /// derives the server `default` from them.
  final bool implicit;

  /// What the daemon had to say, such as a key it could not delete.
  final List<Diagnostic> diagnostics;

  /// The active entry, or null.
  SavedServer? get activeServer {
    for (final SavedServer server in servers) {
      if (server.active) {
        return server;
      }
    }
    return null;
  }

  /// True when there is a choice to switch between.
  bool get canSwitch => servers.length > 1;

  @override
  bool operator ==(Object other) =>
      other is SavedServers &&
      _same(other.servers, servers) &&
      other.active == active &&
      other.implicit == implicit &&
      _same(other.diagnostics, diagnostics);

  @override
  int get hashCode => Object.hash(
    Object.hashAll(servers),
    active,
    implicit,
    Object.hashAll(diagnostics),
  );
}

/// The rule for a server name, the same as the daemon's
/// (`humanitl_config::servers::check_name`): checked here only to say so
/// before the request, the daemon decides.
bool isServerName(String name) =>
    name.isNotEmpty &&
    name.length <= 32 &&
    RegExp(r'^[a-z0-9_-]+$').hasMatch(name);

bool _same<T>(List<T> a, List<T> b) {
  if (a.length != b.length) {
    return false;
  }
  for (int i = 0; i < a.length; i++) {
    if (a[i] != b[i]) {
      return false;
    }
  }
  return true;
}
