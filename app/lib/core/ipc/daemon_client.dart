/// The port through which the app talks to the daemon (ARCHITECTURE 5).
///
/// Two adapters implement it: [GrpcDaemonClient] over the Unix socket and
/// [FakeDaemonClient] in process. Screens are built against this interface
/// and never see either adapter; widget tests always run against the fake.
///
/// [GrpcDaemonClient]: grpc_daemon_client.dart
/// [FakeDaemonClient]: fake_daemon_client.dart
library;

import 'dart:typed_data';

import '../domain/domain.dart';

/// Everything a client can ask the daemon (`service Humanitl`, MVP subset).
///
/// Every failure surfaces as a [DaemonException] carrying a [Diagnostic]
/// with a registered code; callers never see a raw transport error.
abstract class DaemonClient {
  /// `GetInfo`: version, protocol and capabilities of the daemon.
  Future<DaemonInfo> getInfo();

  /// `Subscribe`: the event stream, from now on or from [since].
  ///
  /// The stream ends with an error when the daemon goes away; reconnecting
  /// with backoff is the job of the provider that owns the stream (HUM-020).
  Stream<FlowEvent> subscribe({FlowId? since, bool includePassthrough = false});

  /// `Decide`: decides one held flow, optionally creating [remember] first.
  ///
  /// Returns the rule the daemon created, with the id it assigned, or null
  /// when the decision carried none. The id is what "undo" needs: undo takes
  /// back the rule, never the request (`docs/UX.md` 4.5).
  ///
  /// Throws a [DaemonException] with the daemon's diagnostic when the flow was
  /// not decided (`IPC_003` once it is no longer held). The rule is created
  /// first and taken back again when nothing was decided, so a refused
  /// decision never leaves a rule behind (`backlog/sprint-2.md`, HUM-027).
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember});

  /// `Rules(remove)`: deletes the rule with [id].
  ///
  /// Answers nothing on purpose. The intercept screen uses it for the undo of
  /// a rule it has just created and has no list to refresh; the rules screen
  /// calls [listRules] afterwards, which is one round trip more and keeps this
  /// call the way its one other caller needs it.
  Future<void> removeRule(RuleId id);

  /// `Rules(list)`: the whole rule set, in the order it is evaluated.
  Future<RuleSet> listRules();

  /// `Rules(add)`: creates [rule].
  ///
  /// [Rule.position] is the wish of the client, one-based inside the group the
  /// rule belongs to, and zero means "at the end" (`rules.proto`). A rule the
  /// engine refuses arrives as a [DaemonException] whose diagnostic carries
  /// the field and, for a rule read from `rules.yaml`, the line.
  Future<RuleSet> addRule(Rule rule);

  /// `Rules(update)`: replaces the rule with the same id.
  Future<RuleSet> updateRule(Rule rule);

  /// `Rules(reorder)`: the new order, as the complete list of rule ids.
  ///
  /// The daemon sorts every group by this list, so it is sent whole; a group
  /// is the unit a position counts in (CONVENTIONS 4.5).
  Future<RuleSet> reorderRules(List<RuleId> order);

  /// `Rules(make_permanent)`: moves a session rule into `rules.yaml`.
  Future<RuleSet> makeRulePermanent(RuleId id);

  /// `Rules(reload)`: reads `rules.yaml` again.
  ///
  /// A file the engine refuses is not a failure of the call: the rules that
  /// were in force stay in force and the findings arrive in
  /// [RuleSet.diagnostics].
  Future<RuleSet> reloadRules();

  /// `Rules(dry_run)`: what [rule] alone would have matched among the last
  /// [limit] recorded requests.
  Future<DryRun> dryRunRule(Rule rule, {int limit = dryRunScanDefault});

  /// `ListFlows`: one page of the history.
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  });

  /// `GetFlow`: everything about one flow.
  Future<FlowDetail> getFlow(FlowId id);

  /// `GetBody`: the content behind a body reference, in chunks.
  Stream<Uint8List> getBody(BodyRef ref);

  /// `Sandbox(Status)`: what the agent gets right now.
  ///
  /// The stream carries one snapshot and ends. It is a stream and not a
  /// future because the contract has one `Sandbox` RPC and every operation
  /// answers on it; a start reports twice, a status once.
  Stream<SandboxUpdate> sandboxStatus();

  /// `Sandbox(Plan)`: the snapshot a start with these settings would have,
  /// without starting anything.
  ///
  /// This is how the project directory picker shows what it is about to do:
  /// mounts, environment and command line come back from the daemon for a
  /// directory that does not apply yet. Every argument left out keeps the
  /// value the daemon already has.
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  });

  /// `Sandbox(Start)`: starts the sandbox and reports `starting`, then
  /// `running`, or a diagnostic and `failed`.
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  });

  /// `Sandbox(Stop)`: stops the sandbox and reports `stopping`, then
  /// `stopped`.
  Stream<SandboxUpdate> stopSandbox();

  /// Releases the transport. The client is unusable afterwards.
  Future<void> close();
}

/// How many recorded requests a dry run looks at when the caller says
/// nothing. The number the contract itself uses (`rules.proto`).
const int dryRunScanDefault = 500;

/// The whole rule set after a `Rules` operation.
///
/// Every answer to `Rules` carries the complete set, in the order it is
/// evaluated: session rules first, then the persistent ones, then the bundled
/// ones (CONVENTIONS 4.5). [diagnostics] is what the daemon reported about the
/// operation itself -- a reload that changed something, a file it refused --
/// and is empty when there was nothing to say.
class RuleSet {
  /// Wraps [rules] and [diagnostics].
  const RuleSet({
    this.rules = const <Rule>[],
    this.diagnostics = const <Diagnostic>[],
  });

  /// An answer with no rules and nothing to report.
  static const RuleSet empty = RuleSet();

  /// Every rule, in evaluation order.
  final List<Rule> rules;

  /// What the daemon reported about the operation.
  final List<Diagnostic> diagnostics;

  /// The first diagnostic, the one a client with room for one shows.
  Diagnostic? get first => diagnostics.isEmpty ? null : diagnostics.first;

  @override
  bool operator ==(Object other) =>
      other is RuleSet &&
      _same<Rule>(rules, other.rules) &&
      _same<Diagnostic>(diagnostics, other.diagnostics);

  @override
  int get hashCode =>
      Object.hash(Object.hashAll(rules), Object.hashAll(diagnostics));
}

/// The answer to `Rules(dry_run)`.
///
/// [scanned] is how many recorded requests the daemon looked at, [matches]
/// the ones this rule alone would have matched. Both are counted, never
/// estimated: a dry run that guessed would promise more than it knows
/// (CONVENTIONS 4.13).
class DryRun {
  /// Wraps [matches] out of [scanned].
  const DryRun({this.matches = const <Flow>[], this.scanned = 0});

  /// Nothing scanned, nothing matched.
  static const DryRun empty = DryRun();

  /// The recorded requests the rule would have matched.
  final List<Flow> matches;

  /// How many recorded requests were looked at.
  final int scanned;

  @override
  bool operator ==(Object other) =>
      other is DryRun &&
      other.scanned == scanned &&
      _same<Flow>(matches, other.matches);

  @override
  int get hashCode => Object.hash(scanned, Object.hashAll(matches));
}

/// Element-wise equality; `package:collection` is not a dependency of the app
/// and `listEquals` would pull `package:flutter` into the port.
bool _same<T>(List<T> a, List<T> b) {
  if (identical(a, b)) {
    return true;
  }
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

/// A daemon call failed. [diagnostic] says why in a form a person can read.
class DaemonException implements Exception {
  /// Wraps [diagnostic].
  const DaemonException(this.diagnostic);

  /// The cause, with a registered code.
  final Diagnostic diagnostic;

  /// The code of [diagnostic], for quick matching.
  String get code => diagnostic.code;

  @override
  String toString() => 'DaemonException(${diagnostic.code}: ${diagnostic.why})';
}
