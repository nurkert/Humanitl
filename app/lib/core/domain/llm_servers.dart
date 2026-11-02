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

/// A session profile and the saved server it chooses, mirror of
/// `LlmProfileUse` (HUM-405).
final class ProfileUse {
  /// Creates a row.
  const ProfileUse({
    required this.name,
    this.server = '',
    this.builtin = false,
    this.description = '',
  });

  /// The profile's name.
  final String name;

  /// The saved server its `llm.server` names; empty for the default.
  final String server;

  /// True for a bundled profile without a file of its own.
  final bool builtin;

  /// One sentence about what it is for, or empty.
  final String description;

  @override
  bool operator ==(Object other) =>
      other is ProfileUse &&
      other.name == name &&
      other.server == server &&
      other.builtin == builtin &&
      other.description == description;

  @override
  int get hashCode => Object.hash(name, server, builtin, description);
}

/// The project folder of the configuration and the server its project
/// profile chooses, mirror of `LlmProjectUse` (HUM-405).
final class ProjectUse {
  /// Creates the row.
  const ProjectUse({
    required this.dir,
    this.server = '',
    this.confirmed = false,
    this.effectiveServer = '',
    this.viaProfile = '',
  });

  /// The project folder, as `sandbox.work_dir` names it.
  final String dir;

  /// The saved server `.humanitl/profile.toml` itself names; empty without
  /// a line of its own. An assignment edits exactly this line.
  final String server;

  /// True when [effectiveServer] is the default or was confirmed for this
  /// folder, so a start needs no question.
  final bool confirmed;

  /// The server the next start really gets from the project: its own line,
  /// or the line of the bundled profile it names (HUM-405); empty for none.
  final String effectiveServer;

  /// The profile [effectiveServer] comes through when the project has no
  /// line of its own; empty otherwise.
  final String viaProfile;

  /// The effective server, falling back to the project's own line.
  String get chosen => effectiveServer.isEmpty ? server : effectiveServer;

  @override
  bool operator ==(Object other) =>
      other is ProjectUse &&
      other.dir == dir &&
      other.server == server &&
      other.confirmed == confirmed &&
      other.effectiveServer == effectiveServer &&
      other.viaProfile == viaProfile;

  @override
  int get hashCode =>
      Object.hash(dir, server, confirmed, effectiveServer, viaProfile);
}

/// The saved servers after a request.
final class SavedServers {
  /// Creates a list.
  const SavedServers({
    this.servers = const <SavedServer>[],
    this.active = '',
    this.implicit = false,
    this.diagnostics = const <Diagnostic>[],
    this.profiles = const <ProfileUse>[],
    this.project,
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

  /// Every session profile except `default`, with the server it chooses
  /// (HUM-405).
  final List<ProfileUse> profiles;

  /// The project folder of the configuration with its server, or null
  /// without a folder (HUM-405).
  final ProjectUse? project;

  /// The saved server [name], or null.
  SavedServer? named(String name) {
    for (final SavedServer server in servers) {
      if (server.name == name) {
        return server;
      }
    }
    return null;
  }

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
      _same(other.diagnostics, diagnostics) &&
      _same(other.profiles, profiles) &&
      other.project == project;

  @override
  int get hashCode => Object.hash(
    Object.hashAll(servers),
    active,
    implicit,
    Object.hashAll(diagnostics),
    Object.hashAll(profiles),
    project,
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
