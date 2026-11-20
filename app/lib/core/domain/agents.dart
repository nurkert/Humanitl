/// The agent depot (HUM-518 to HUM-521, ADR-0023): per known agent where
/// sessions take its program from, what the depot holds, and the last
/// maintenance run.
///
/// Plain value types, read-only views of an answer of `Agents`. Which
/// operation a client may offer comes from the daemon ([AgentActions]); the
/// app decides none of it itself.
library;

import 'diagnostic.dart';

/// The two sources of an agent's program (`agents.<id>.source`).
abstract final class AgentSource {
  /// The program installed on this machine.
  static const String host = 'host';

  /// The copy in Humanitl's depot.
  static const String depot = 'depot';
}

/// The states of a maintenance run (`AgentMaintenance.state`).
abstract final class AgentRunState {
  /// The run goes on.
  static const String running = 'running';

  /// The result was taken over.
  static const String done = 'done';

  /// Nothing was taken over.
  static const String failed = 'failed';

  /// A configuration run waits for Accept or Discard.
  static const String confirm = 'confirm';

  /// The person discarded a configuration run (HUM-521).
  static const String discarded = 'discarded';
}

/// The tasks of a maintenance run (`AgentMaintenance.task`).
abstract final class AgentRunTask {
  /// The first install.
  static const String install = 'install';

  /// A new version, or back to one the depot holds.
  static const String update = 'update';

  /// A login with the depot's program.
  static const String login = 'login';

  /// A configuration run on a writable copy.
  static const String config = 'config';
}

/// What `Agents` is asked to do.
enum AgentsOpKind {
  /// Read the state.
  status,

  /// `agents.<id>.source`: [AgentsOp.value] is `host` or `depot`.
  setSource,

  /// Remove the depot of one agent.
  remove,

  /// The first install; [AgentsOp.value] is the version, empty for the
  /// vendor's current one.
  install,

  /// A new version or back to a held one; [AgentsOp.value] as for install.
  update,

  /// A login run.
  login,

  /// A configuration run.
  config,

  /// Take over the waiting configuration.
  configAccept,

  /// Discard the waiting configuration.
  configDiscard,

  /// The files of `config/`, read only.
  configShow,

  /// A pasted callback URL into the running login; [AgentsOp.value] is it.
  deliverCallback,
}

/// One request to `Agents`: the operation, the agent, and the one value
/// some operations take.
final class AgentsOp {
  const AgentsOp._(this.kind, {this.agent = '', this.value = ''});

  /// Read the state.
  static const AgentsOp status = AgentsOp._(AgentsOpKind.status);

  /// Set the source of [agent] to [source].
  const AgentsOp.setSource(String agent, String source)
    : this._(AgentsOpKind.setSource, agent: agent, value: source);

  /// Remove the depot of [agent].
  const AgentsOp.remove(String agent)
    : this._(AgentsOpKind.remove, agent: agent);

  /// Install [agent] in [version], empty for the vendor's current one.
  const AgentsOp.install(String agent, {String version = ''})
    : this._(AgentsOpKind.install, agent: agent, value: version);

  /// Update [agent] to [version], empty for the vendor's current one.
  const AgentsOp.update(String agent, {String version = ''})
    : this._(AgentsOpKind.update, agent: agent, value: version);

  /// Log [agent] in.
  const AgentsOp.login(String agent) : this._(AgentsOpKind.login, agent: agent);

  /// A configuration run of [agent].
  const AgentsOp.config(String agent)
    : this._(AgentsOpKind.config, agent: agent);

  /// Take over the waiting configuration of [agent].
  const AgentsOp.configAccept(String agent)
    : this._(AgentsOpKind.configAccept, agent: agent);

  /// Discard the waiting configuration of [agent].
  const AgentsOp.configDiscard(String agent)
    : this._(AgentsOpKind.configDiscard, agent: agent);

  /// The files of `config/` of [agent].
  const AgentsOp.configShow(String agent)
    : this._(AgentsOpKind.configShow, agent: agent);

  /// Deliver the pasted [url] into the running login of [agent].
  const AgentsOp.deliverCallback(String agent, String url)
    : this._(AgentsOpKind.deliverCallback, agent: agent, value: url);

  /// The operation.
  final AgentsOpKind kind;

  /// The agent's id, empty for [AgentsOpKind.status].
  final String agent;

  /// The source, the version or the callback URL; empty otherwise.
  final String value;

  @override
  bool operator ==(Object other) =>
      other is AgentsOp &&
      other.kind == kind &&
      other.agent == agent &&
      other.value == value;

  @override
  int get hashCode => Object.hash(kind, agent, value);

  @override
  String toString() => 'AgentsOp(${kind.name}, $agent, $value)';
}

/// The operations the daemon accepts for one agent now, mirror of
/// `AgentActions`.
final class AgentActions {
  /// Creates the set.
  const AgentActions({
    this.install = false,
    this.update = false,
    this.login = false,
    this.config = false,
    this.configShow = false,
    this.remove = false,
    this.sourceHost = false,
    this.sourceDepot = false,
  });

  /// Nothing; what a daemon before contract 1.54 implies.
  static const AgentActions none = AgentActions();

  /// `install`.
  final bool install;

  /// `update`, also back to a held version.
  final bool update;

  /// `login`.
  final bool login;

  /// `config`.
  final bool config;

  /// `config_show`.
  final bool configShow;

  /// `remove`.
  final bool remove;

  /// `set_source` to `host`; false under a company lock.
  final bool sourceHost;

  /// `set_source` to `depot`.
  final bool sourceDepot;
}

/// A URL a login run printed, mirror of `AgentLoginUrl`.
final class AgentLoginUrl {
  /// Creates a URL.
  const AgentLoginUrl({required this.url, this.openable = false});

  /// As the agent printed it.
  final String url;

  /// True only for HTTPS to a login host of the adapter; every other URL
  /// is shown, never opened.
  final bool openable;
}

/// A changed file of a configuration run, or a file of `config/`, mirror of
/// `AgentConfigChange`.
final class AgentConfigChange {
  /// Creates a change.
  const AgentConfigChange({
    required this.path,
    this.change = 'added',
    this.keys = const <String>[],
    this.commandKeys = const <String>[],
    this.keysOmitted = 0,
  });

  /// Relative to the configuration directory.
  final String path;

  /// `added`, `changed` or `removed`.
  final String change;

  /// The changed keys, those that run commands first.
  final List<String> keys;

  /// The keys that run commands; a client highlights them.
  final List<String> commandKeys;

  /// How many further keys [keys] leaves out.
  final int keysOmitted;
}

/// The last maintenance run of an agent, mirror of `AgentMaintenance`.
final class AgentMaintenance {
  /// Creates a run.
  const AgentMaintenance({
    required this.task,
    required this.state,
    this.sandboxId = '',
    this.version = '',
    this.previous = '',
    this.diagnostics = const <Diagnostic>[],
    this.loginUrls = const <AgentLoginUrl>[],
    this.callbackPaste = false,
    this.configChanges = const <AgentConfigChange>[],
  });

  /// The sandbox of the run; empty when no run was needed.
  final String sandboxId;

  /// One of [AgentRunTask].
  final String task;

  /// One of [AgentRunState].
  final String state;

  /// The version `current` names after the run; empty before and after a
  /// failure.
  final String version;

  /// The version before; empty at the first install.
  final String previous;

  /// What the daemon had to say.
  final List<Diagnostic> diagnostics;

  /// The URLs a login printed so far.
  final List<AgentLoginUrl> loginUrls;

  /// Whether a client shows the field for a callback URL.
  final bool callbackPaste;

  /// The changes waiting for Accept or Discard.
  final List<AgentConfigChange> configChanges;

  /// Whether the run goes on.
  bool get running => state == AgentRunState.running;
}

/// One agent in the depot, mirror of `AgentDepot`.
final class AgentDepot {
  /// Creates an entry.
  const AgentDepot({
    required this.agent,
    required this.displayName,
    this.source = AgentSource.host,
    this.origin = 'default',
    this.lockedBy = '',
    this.depotSupported = false,
    this.version = '',
    this.signature = '',
    this.loggedIn = false,
    this.lastWriteback = '',
    this.sizeBytes = 0,
    this.inUse = false,
    this.diagnostics = const <Diagnostic>[],
    this.maintenance,
    this.configFiles = const <AgentConfigChange>[],
    this.versions = const <String>[],
    this.actions = AgentActions.none,
    this.credentialWriteback = true,
  });

  /// The id, as in `agents.<id>`.
  final String agent;

  /// The name to show.
  final String displayName;

  /// The effective source, [AgentSource.host] or [AgentSource.depot].
  final String source;

  /// Where it comes from: `default`, `agents.source`, `agents.<id>.source`
  /// or `policy`.
  final String origin;

  /// The policy that locks the source; empty without a lock.
  final String lockedBy;

  /// Whether the adapter brings a recipe for the depot.
  final bool depotSupported;

  /// The installed version; empty without.
  final String version;

  /// The result of the signature check: `verified` or `unsigned`.
  final String signature;

  /// Whether the depot holds a login.
  final bool loggedIn;

  /// `<RFC 3339> session <id>` of the last write-back; empty without.
  final String lastWriteback;

  /// The bytes of every file of the depot.
  final int sizeBytes;

  /// Whether a session uses the depot now.
  final bool inUse;

  /// What the daemon has to say about the depot.
  final List<Diagnostic> diagnostics;

  /// The last maintenance run in this daemon, or null.
  final AgentMaintenance? maintenance;

  /// The files of `config/`, only in the answer to `config_show`.
  final List<AgentConfigChange> configFiles;

  /// The versions the depot holds, newest first.
  final List<String> versions;

  /// What a client may offer now.
  final AgentActions actions;

  /// The effective `agents.<id>.credential_writeback`, as the daemon reads
  /// it.
  final bool credentialWriteback;

  /// The time of [lastWriteback], or null when it names none the app can
  /// read; the daemon writes `<RFC 3339> session <id>`.
  DateTime? get lastWritebackAt =>
      DateTime.tryParse(lastWriteback.split(' session ').first);

  /// The session of [lastWriteback]; empty without.
  String get lastWritebackSession {
    final int at = lastWriteback.indexOf(' session ');
    return at < 0 ? '' : lastWriteback.substring(at + ' session '.length);
  }

  /// Whether a company policy locks the source.
  bool get locked => lockedBy.isNotEmpty;

  /// Whether a version is installed.
  bool get installed => version.isNotEmpty;

  /// Whether the vendor's signature was checked at install.
  bool get signatureVerified => signature == 'verified';

  /// The held versions other than the installed one: the way back.
  List<String> get olderVersions => <String>[
    for (final String held in versions)
      if (held != version) held,
  ];
}

/// The state of every known agent, mirror of `AgentsStatus`.
final class AgentsStatus {
  /// Creates the state.
  const AgentsStatus({this.agents = const <AgentDepot>[]});

  /// No agent known.
  static const AgentsStatus none = AgentsStatus();

  /// In the order of `agents.<id>`.
  final List<AgentDepot> agents;

  /// The entry of [agent], or null.
  AgentDepot? of(String agent) {
    for (final AgentDepot entry in agents) {
      if (entry.agent == agent) {
        return entry;
      }
    }
    return null;
  }
}
