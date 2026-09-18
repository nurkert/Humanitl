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

  /// `Rules(set_disabled)`: switches a bundled rule off or back on.
  ///
  /// Any other rule is refused by the daemon with `RULES_010`: a rule of the
  /// person is deleted, not switched off (`rules_store.rs`,
  /// `set_bundled_disabled`). The answer carries the whole set, so the screen
  /// never has to guess what the new state is.
  Future<RuleSet> setRuleDisabled(RuleId id, {required bool disabled});

  /// `ListFlows`: one page of the history.
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  });

  /// `GetFlow`: everything about one flow.
  Future<FlowDetail> getFlow(FlowId id);

  /// `GetBody`: the content behind a body reference, in chunks.
  ///
  /// The bytes as the recording holds them. Use [getBodyChunks] when the
  /// reference asks the daemon to unpack: only a chunk says what is still on
  /// the bytes that arrive.
  Stream<Uint8List> getBody(BodyRef ref);

  /// `GetBody`, with what each chunk says about the encoding.
  ///
  /// The same RPC as [getBody]; the difference is only that the answer keeps
  /// `BodyChunk.encoding_left`. A reference built with [BodyRef.asking] comes
  /// back unpacked and with an empty [BodyChunk.encodingLeft]; one whose
  /// encoding the daemon cannot take off comes back as it was recorded, with
  /// the name of that encoding (HUM-119).
  Stream<BodyChunk> getBodyChunks(BodyRef ref);

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

  /// `Sandbox(IsolationCheck)`: the three guarantees, measured inside the
  /// running sandbox.
  ///
  /// The stream carries one [SandboxUpdate.check] per guarantee. When no
  /// sandbox runs it carries none: nothing was measured, and three grey
  /// results on the wire could not be told apart from three measured ones
  /// (CONVENTIONS 4.13).
  Stream<SandboxUpdate> checkIsolation();

  /// `Doctor`: one line per precondition of the machine the daemon runs on
  /// (HUM-075).
  ///
  /// The eleven checks live in `humanitl_sandbox::doctor` and nowhere else;
  /// this call fetches their verdict. It contacts nothing on the network:
  /// `rpc Doctor(Empty)` has no field in which a client could ask for a
  /// connection, so opening a screen never opens one either. The `llm` line
  /// therefore always arrives as "not contacted" and is measured with
  /// [probeLlm] when a person asks.
  Future<DoctorReport> doctor();

  /// `ProbeLlm`: asks exactly the endpoint the caller names (HUM-039).
  ///
  /// Two GET requests on two fixed paths, no redirect, no credentials,
  /// nothing through the sandbox. **This is the one call of the application
  /// that reaches a machine on the network, and it runs only on an explicit
  /// gesture** -- never while somebody types, because the name would go into
  /// DNS before anybody decided on it (HUM-044).
  ///
  /// An endpoint the daemon refuses (`LLM_007`) or one outside a private
  /// network (`LLM_006`) arrives as a finding, in the answer or as a
  /// [DaemonException]; neither is invented here.
  Future<LlmProbe> probeLlm(String endpoint, {Duration? timeout});

  /// `DiscoverLlm`: searches the local network for LLM servers (HUM-076).
  ///
  /// The second call of this application that reaches machines on the network,
  /// and like [probeLlm] it runs only on an explicit gesture. It is a scan:
  /// one connection attempt per address and port in the local /24, four ports,
  /// nothing written and no credentials sent. What it costs and where it goes
  /// stands above the button that starts it; this method never runs on its
  /// own.
  ///
  /// Servers arrive as they answer, so a list can fill while the search runs.
  /// Cancelling the subscription ends the search in the daemon — that is what
  /// the sheet's close button does.
  ///
  /// [subnet] overrides the network, in CIDR notation and never wider than a
  /// /24; without it the daemon takes the local /24 of its default route.
  /// [ports] overrides the four default ports.
  Stream<LlmServer> discoverLlm({String? subnet, List<int> ports});

  /// `SetConfig`: writes one value into `config.toml` (HUM-151).
  ///
  /// The daemon accepts exactly one kind of write for now: `sandbox.env.<NAME>`
  /// set to the certificate path it mounts in the sandbox
  /// (`/etc/humanitl/ca.crt`), which is what a `TLS_001` finding proposes.
  /// Anything else arrives as a [DaemonException] carrying `CONFIG_014`; the
  /// settings screen widens the call (HUM-069).
  ///
  /// The running session keeps its environment, which stood when it started;
  /// the value applies to sessions started afterwards. The daemon answers with
  /// a fresh `ConfigSnapshot`, and this method drops it on purpose: there is no
  /// domain type for it yet, and HUM-069 brings one.
  Future<void> setConfig(String key, String value);

  /// `Terminal`: the output of the running session, and the keys on their way
  /// back (HUM-042).
  ///
  /// The first and only method of this port with a stream on both sides. The
  /// first message of [input] is a [TerminalOpen] -- by construction, not by
  /// convention: the daemon reads nothing before it.
  ///
  /// One client writes, any number watch. A second [TerminalOpen] with
  /// `readOnly: false` gets a [TerminalFinding] carrying `TERM_001` and its
  /// stream ends; the keys and resizes of a reader are dropped in the daemon,
  /// which is the side that can enforce it.
  ///
  /// The bytes in [TerminalOutput] are filtered in the daemon. This app adds
  /// no filter and registers no OSC handler on the emulator: two filters
  /// would be two promises (`docs/SECURITY.md` 3.3).
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input);

  /// `Audit(head)`: the end of the chain, without reading it end to end
  /// (HUM-051).
  ///
  /// The cheap half of the audit screen: how many records there are, what the
  /// last one hashes to, how many anchors stand beside them and when the last
  /// of those was set. It proves nothing on its own — the head hash of a chain
  /// somebody rewrote is the head hash of the rewritten chain — and that is why
  /// [auditVerify] exists next to it.
  Future<AuditHead> auditHead();

  /// `Audit(verify)`: the chain, checked record by record (HUM-050, HUM-051).
  ///
  /// Runs in the daemon and can take seconds over a long log, so a caller
  /// shows that it is waiting rather than blocking a frame on it. The answer
  /// says either that every record holds, or the first sequence number that
  /// does not and why; a chain that holds can still carry warnings about what
  /// the check could **not** prove.
  Future<AuditReport> auditVerify();

  /// `Audit(query)`: one page of records, newest first (HUM-051).
  ///
  /// Paged over [AuditPage.nextCursor], never fetched whole: the chain grows
  /// without a ceiling, and a table that loaded it all would be a table that
  /// stops working the longer the product is used.
  Future<AuditPage> auditQuery(
    AuditFilter filter, {
    int limit = auditPageSize,
    String? cursor,
  });

  /// `Audit(export)`: the daemon writes the export to [outPath] (HUM-051).
  ///
  /// **The daemon writes the file, not this application.** The chain lives
  /// beside the daemon, the app only names a path, and a JSONL export that has
  /// to be byte-identical with the source cannot travel through a second
  /// encoder on the way. The caller therefore asks a person for a path first
  /// and passes it here.
  ///
  /// [from] and [to] are the range of the table the person is looking at; both
  /// ends are inclusive, and null means no bound on that side.
  Future<AuditExport> auditExport({
    required AuditExportFormat format,
    required String outPath,
    DateTime? from,
    DateTime? to,
  });

  /// Releases the transport. The client is unusable afterwards.
  Future<void> close();
}

/// How many audit records one page asks for. The wire default of `Query`.
const int auditPageSize = 200;

/// Which of the two documents an audit export writes.
enum AuditExportFormat {
  /// Every record as the chain holds it, one canonical JSON line each.
  ///
  /// This is the form the acceptance of HUM-051 measures: for a range, the
  /// file is byte-identical with the corresponding lines of `audit.jsonl`.
  jsonl,

  /// One row per record with the twelve columns of HUM-050.
  csv;

  /// The value the `Export.format` field of the contract carries.
  String get wireName => switch (this) {
    AuditExportFormat.jsonl => 'jsonl',
    AuditExportFormat.csv => 'csv',
  };

  /// The extension of the file the save dialog offers.
  String get fileExtension => switch (this) {
    AuditExportFormat.jsonl => 'jsonl',
    AuditExportFormat.csv => 'csv',
  };
}

/// The end of the audit chain (`Audit(head)`).
final class AuditHead {
  /// Creates a head.
  const AuditHead({
    this.seq = 0,
    this.hash = '',
    this.records = 0,
    this.anchors,
    this.lastAnchorAt,
  });

  /// An empty chain: nothing written, nothing anchored.
  static const AuditHead empty = AuditHead();

  /// The sequence number of the last record; 0 for an empty chain.
  final int seq;

  /// The hash of the last record as lowercase hex; empty for an empty chain.
  ///
  /// The same string `humanitl audit verify --json | jq .head` prints.
  final String hash;

  /// How many records the chain holds.
  final int records;

  /// How many anchors stand beside it, in `audit_anchors`, or null when the
  /// daemon did not report them.
  ///
  /// Null and zero are two different answers. Zero means the daemon looked
  /// and found no anchor, so every record could be cut off unnoticed; null
  /// means nobody said, which is what a daemon answers that predates the
  /// field (`AuditResponse.anchors_reported`, HUM-051).
  final int? anchors;

  /// When the last anchor was set; null when there is none or when [anchors]
  /// was not reported.
  final DateTime? lastAnchorAt;

  @override
  bool operator ==(Object other) =>
      other is AuditHead &&
      other.seq == seq &&
      other.hash == hash &&
      other.records == records &&
      other.anchors == anchors &&
      other.lastAnchorAt == lastAnchorAt;

  @override
  int get hashCode => Object.hash(seq, hash, records, anchors, lastAnchorAt);
}

/// Why a chain broke, in the words of `humanitl_audit::BreakReason`.
///
/// The wire carries `snake_case`; an unknown value becomes
/// [AuditBreakReason.unknown] rather than an exception, because a daemon that
/// learned a new reason must not take down the screen that reports it.
enum AuditBreakReason {
  /// A sequence number is missing or out of place.
  seqGap,

  /// `prev` is not the hash of the record before it.
  prevMismatch,

  /// The hash does not match the fields.
  hashMismatch,

  /// The MAC does not match the hash: the chain was rebuilt without the key.
  macMismatch,

  /// The line is not a record, or not its own canonical form.
  nonCanonicalLine,

  /// The anchor for this number names another hash.
  anchorMismatch,

  /// The file ends below an anchor.
  truncatedBelowAnchor,

  /// The daemon named a reason this app does not know.
  unknown;

  /// Reads the `snake_case` name off the wire.
  static AuditBreakReason parse(String wire) => switch (wire) {
    'seq_gap' => AuditBreakReason.seqGap,
    'prev_mismatch' => AuditBreakReason.prevMismatch,
    'hash_mismatch' => AuditBreakReason.hashMismatch,
    'mac_mismatch' => AuditBreakReason.macMismatch,
    'non_canonical_line' => AuditBreakReason.nonCanonicalLine,
    'anchor_mismatch' => AuditBreakReason.anchorMismatch,
    'truncated_below_anchor' => AuditBreakReason.truncatedBelowAnchor,
    _ => AuditBreakReason.unknown,
  };
}

/// What the check could not prove, although the chain holds.
final class AuditWarning {
  /// Creates a warning.
  const AuditWarning({required this.kind, this.records = 0});

  /// `no_hmac_key` or `unanchored_tail`.
  final String kind;

  /// How many records stand behind the last anchor; only for
  /// `unanchored_tail`.
  final int records;

  @override
  bool operator ==(Object other) =>
      other is AuditWarning && other.kind == kind && other.records == records;

  @override
  int get hashCode => Object.hash(kind, records);
}

/// The result of `Audit(verify)`.
final class AuditReport {
  /// Creates a report.
  const AuditReport({
    required this.ok,
    this.records = 0,
    this.firstBadSeq = 0,
    this.reason = AuditBreakReason.unknown,
    this.warnings = const <AuditWarning>[],
    this.diagnostic,
  });

  /// True when every record holds and no anchor lies beyond the end.
  final bool ok;

  /// How many records passed the check.
  final int records;

  /// The first sequence number that does not hold; only when [ok] is false.
  final int firstBadSeq;

  /// Why it does not hold; only when [ok] is false.
  final AuditBreakReason reason;

  /// What the check could not prove, although the chain holds.
  final List<AuditWarning> warnings;

  /// The finding of the daemon for a broken chain (`AUDIT_001`), or null.
  ///
  /// The screen shows this one and invents none of its own: the sentence that
  /// says what happened belongs to the side that read the file
  /// (`docs/UX.md` 4.4).
  final Diagnostic? diagnostic;

  @override
  bool operator ==(Object other) =>
      other is AuditReport &&
      other.ok == ok &&
      other.records == records &&
      other.firstBadSeq == firstBadSeq &&
      other.reason == reason &&
      _same(other.warnings, warnings) &&
      other.diagnostic == diagnostic;

  @override
  int get hashCode => Object.hash(
    ok,
    records,
    firstBadSeq,
    reason,
    Object.hashAll(warnings),
    diagnostic,
  );
}

/// Which records a query asks for.
final class AuditFilter {
  /// Creates a filter. Every field left out means "no bound".
  const AuditFilter({
    this.kindPrefix = '',
    this.session = '',
    this.from,
    this.to,
  });

  /// Nothing filtered.
  static const AuditFilter none = AuditFilter();

  /// Prefix of the kind, such as `flow.` or `flow.decided`.
  final String kindPrefix;

  /// The session as UUID text; empty means every session.
  final String session;

  /// Earliest timestamp, inclusive.
  final DateTime? from;

  /// Latest timestamp, inclusive.
  final DateTime? to;

  /// A copy with the named fields replaced; [clearFrom] and [clearTo] remove a
  /// bound, because null means "unchanged" here as everywhere else.
  AuditFilter copyWith({
    String? kindPrefix,
    String? session,
    DateTime? from,
    DateTime? to,
    bool clearFrom = false,
    bool clearTo = false,
  }) => AuditFilter(
    kindPrefix: kindPrefix ?? this.kindPrefix,
    session: session ?? this.session,
    from: clearFrom ? null : (from ?? this.from),
    to: clearTo ? null : (to ?? this.to),
  );

  @override
  bool operator ==(Object other) =>
      other is AuditFilter &&
      other.kindPrefix == kindPrefix &&
      other.session == session &&
      other.from == from &&
      other.to == to;

  @override
  int get hashCode => Object.hash(kindPrefix, session, from, to);
}

/// One record of the chain, as the table and the sheet show it.
final class AuditRecordRow {
  /// Creates a row.
  const AuditRecordRow({
    required this.seq,
    required this.ts,
    required this.kind,
    this.session = '',
    this.dataJson = '{}',
    this.line = '',
  });

  /// The sequence number.
  final int seq;

  /// The timestamp in the format of the log: UTC, microseconds, always `Z`.
  ///
  /// Text and not a [DateTime], because these very characters are what the
  /// hash covers. [time] is the parsed form for anybody who needs to compare.
  final String ts;

  /// The kind, such as `flow.decided`.
  final String kind;

  /// The session as UUID text, or `-` for a record that belongs to none.
  final String session;

  /// `data` of the record as a JSON object.
  final String dataJson;

  /// The complete canonical line, byte for byte as it stands in the file.
  final String line;

  /// [ts] parsed, or null when it is not a timestamp this app can read.
  DateTime? get time => DateTime.tryParse(ts)?.toUtc();

  @override
  bool operator ==(Object other) =>
      other is AuditRecordRow &&
      other.seq == seq &&
      other.ts == ts &&
      other.kind == kind &&
      other.session == session &&
      other.dataJson == dataJson &&
      other.line == line;

  @override
  int get hashCode => Object.hash(seq, ts, kind, session, dataJson, line);
}

/// One page of `Audit(query)`.
final class AuditPage {
  /// Creates a page.
  const AuditPage({
    this.rows = const <AuditRecordRow>[],
    this.nextCursor = '',
    this.total = 0,
  });

  /// Nothing loaded.
  static const AuditPage empty = AuditPage();

  /// The records, newest first.
  final List<AuditRecordRow> rows;

  /// Where the next page begins; empty when there is none.
  final String nextCursor;

  /// How many records the filter matches, as the daemon counted them.
  final int total;

  @override
  bool operator ==(Object other) =>
      other is AuditPage &&
      _same(other.rows, rows) &&
      other.nextCursor == nextCursor &&
      other.total == total;

  @override
  int get hashCode => Object.hash(Object.hashAll(rows), nextCursor, total);
}

/// What an export wrote.
final class AuditExport {
  /// Creates a result.
  const AuditExport({required this.path, this.records = 0});

  /// The file the daemon wrote.
  final String path;

  /// How many records went into it.
  final int records;

  @override
  bool operator ==(Object other) =>
      other is AuditExport && other.path == path && other.records == records;

  @override
  int get hashCode => Object.hash(path, records);
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

/// One chunk of `GetBody`.
///
/// [encodingLeft] is the same in every chunk of one answer: empty when the
/// bytes are what the findings of the daemon point into — either nothing was
/// packed or the daemon took the packing off — and otherwise the name of the
/// encoding that is still on them, such as `zstd` or the chain `gzip, br`
/// (HUM-119). A view that places a finding must look at it: a span into
/// unpacked bytes lands anywhere in packed ones.
final class BodyChunk {
  /// Creates a chunk.
  const BodyChunk({
    required this.data,
    required this.last,
    this.encodingLeft = '',
  });

  /// The bytes of this chunk.
  final Uint8List data;

  /// True for the chunk that ends the body (`BodyChunk.last`).
  ///
  /// Required, not defaulted: a stream that stops without one stopped in the
  /// middle, and a client that forgets to say so turns a body cut short on
  /// the wire into a body that merely looks complete. Every answer of the
  /// daemon carries exactly one, an empty body included.
  final bool last;

  /// The encoding still on [data], empty when there is none.
  final String encodingLeft;
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
