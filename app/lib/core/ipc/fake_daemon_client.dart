/// The in-process adapter of [DaemonClient]: no socket, no daemon, a scripted
/// session that replays on `subscribe` and reacts to `decide`.
///
/// Widget tests run against this; so does the app with
/// `--dart-define=HUMANITL_FAKE=<scenario>` for any scenario name other than
/// `1`/`default` (those mean the Rust fake daemon, CONVENTIONS 4.7). The
/// script here is a small cousin of `fixtures/sessions/mixed.jsonl`; a loader
/// for that format is HUM-058.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import '../domain/domain.dart';
import 'client_diagnostics.dart';
import 'daemon_client.dart';

/// One step of a script: after [after] from the start of the subscription,
/// [build] produces the event, or null when the state no longer calls for it
/// (a timeout for a flow that was decided meanwhile).
class ScriptedEvent {
  /// Creates a step.
  const ScriptedEvent(this.after, this.build);

  /// Delay from the start of the subscription.
  final Duration after;

  /// Produces the event from the current state, or nothing.
  final FlowEvent? Function(FakeSessionState state, DateTime now) build;
}

/// What the fake knows: flows, details and bodies.
class FakeSessionState {
  /// Flows by id, in arrival order.
  final Map<FlowId, Flow> flows = <FlowId, Flow>{};

  /// Request details by flow id.
  final Map<FlowId, FlowDetail> details = <FlowId, FlowDetail>{};

  /// Body content by hex digest.
  final Map<String, Uint8List> bodies = <String, Uint8List>{};

  /// The flow with [id], or null.
  Flow? flow(FlowId id) => flows[id];

  /// Replaces the flow with [id] by [update] applied to it.
  void update(FlowId id, Flow Function(Flow flow) update) {
    final Flow? current = flows[id];
    if (current != null) {
      flows[id] = update(current);
      final FlowDetail? detail = details[id];
      if (detail != null) {
        details[id] = detail.copyWith(summary: flows[id]!);
      }
    }
  }
}

/// A decision the fake received, for assertions in tests.
class RecordedDecision {
  /// Creates a record.
  const RecordedDecision(this.flowId, this.decision, this.remember);

  /// Which flow.
  final FlowId flowId;

  /// What was decided.
  final Decision decision;

  /// The rule that came with it, if any.
  final Rule? remember;
}

/// [DaemonClient] without a daemon.
class FakeDaemonClient implements DaemonClient {
  /// Creates a fake that answers `GetInfo` with [info] (or [defaultInfo]),
  /// or throws [infoFailure] when given, and replays [script] (or
  /// [defaultScript]) on `subscribe`.
  FakeDaemonClient({
    DaemonInfo? info,
    this.infoFailure,
    List<ScriptedEvent>? script,
    DateTime Function()? clock,
  }) : info = info ?? defaultInfo,
       script = script ?? defaultScript(),
       _clock = clock ?? DateTime.now;

  /// A fake whose daemon is not running: `GetInfo` throws `DAEMON_001`.
  factory FakeDaemonClient.unavailable({String socketPath = defaultSocket}) =>
      FakeDaemonClient(
        infoFailure: ClientDiagnostics.daemonUnreachable(
          socketPath: socketPath,
          detail: 'connection refused',
          fake: true,
        ),
      );

  /// A fake whose daemon speaks another major of the contract.
  factory FakeDaemonClient.incompatible({
    int protoMajor = 2,
    int protoMinor = 0,
  }) => FakeDaemonClient(
    info: defaultInfo.copyWith(protoMajor: protoMajor, protoMinor: protoMinor),
  );

  /// A fake that never emits an event; the queue stays empty.
  factory FakeDaemonClient.empty() =>
      FakeDaemonClient(script: const <ScriptedEvent>[]);

  /// A fake that holds [count] requests in a row, one every [spacing],
  /// each with a hold budget of [budget]: the load scenario of HUM-020
  /// (`burst`, `burst:200`).
  factory FakeDaemonClient.burst({
    int count = 30,
    Duration spacing = const Duration(milliseconds: 100),
    Duration budget = const Duration(seconds: 300),
    DateTime Function()? clock,
  }) => FakeDaemonClient(
    script: burstScript(count: count, spacing: spacing, budget: budget),
    clock: clock,
  );

  /// A fake whose recorder already holds [count] finished flows: the
  /// scenario of HUM-032 (`history`, `history:10000`).
  ///
  /// Nothing is scripted; the flows are simply there, the way a recorder that
  /// has been running for a while has them. They are laid out deterministically
  /// -- same input, same rows -- so that a golden and a paging test mean the
  /// same thing twice, and they cover all eight visual states, because the
  /// history is the one screen where all eight stand in one column.
  factory FakeDaemonClient.history({
    int count = 200,
    DateTime? start,
    DateTime Function()? clock,
  }) {
    final FakeDaemonClient client = FakeDaemonClient(
      script: const <ScriptedEvent>[],
      clock: clock,
    );
    seedHistory(client.state, count: count, start: start ?? historyEpoch);
    return client;
  }

  /// A fake that holds one request with a body of eight mebibytes: the
  /// `big_body` scenario of HUM-030.
  ///
  /// The body is generated rather than read from
  /// `fixtures/sessions/big_body.jsonl`: eight mebibytes of JSON cost every
  /// checkout more than they explain, and the content is generated anyway.
  /// One finding sits in it, so that the chips and the jump to a finding have
  /// something to do.
  factory FakeDaemonClient.bigBody({DateTime Function()? clock}) =>
      FakeDaemonClient(script: bigBodyScript(), clock: clock);

  /// The fake for a `HUMANITL_FAKE=<name>` scenario. Unknown names replay
  /// the default script, so a typo still shows a working shell.
  factory FakeDaemonClient.scenario(String name) {
    final String key = name.trim().toLowerCase();
    if (key.startsWith('burst')) {
      final int? count = int.tryParse(key.split(':').skip(1).join());
      return FakeDaemonClient.burst(count: count ?? 30);
    }
    if (key.startsWith('history')) {
      final int? count = int.tryParse(key.split(':').skip(1).join());
      return FakeDaemonClient.history(count: count ?? 200);
    }
    return switch (key) {
      'unavailable' || 'missing' || 'down' => FakeDaemonClient.unavailable(),
      'mismatch' || 'incompatible' => FakeDaemonClient.incompatible(),
      'empty' => FakeDaemonClient.empty(),
      'rules:broken' || 'broken-rules' => FakeDaemonClient.brokenRules(),
      'big_body' || 'big-body' => FakeDaemonClient.bigBody(),
      _ => FakeDaemonClient(),
    };
  }

  /// A fake whose `rules.yaml` was edited by hand into something the engine
  /// refuses: every reload answers with the finding, and the rules that were
  /// in force stay in force. The acceptance criterion of HUM-033 walks
  /// through exactly this.
  factory FakeDaemonClient.brokenRules() =>
      FakeDaemonClient()..reloadDiagnostics = <Diagnostic>[brokenFileReport];

  /// The socket the unavailable scenario pretends to have tried.
  static const String defaultSocket = '/run/user/1000/humanitl/daemon.sock';

  /// Die Sandbox, die der Fake startet.
  static const SandboxId defaultSandbox = SandboxId(
    '018f0002-0000-7000-8000-000000000001',
  );

  /// Das Projektverzeichnis, das der Fake einhaengt.
  static const String defaultWorkDir = '/home/nik/clients/acme';

  /// The session every scripted flow belongs to.
  static const SessionId defaultSession = SessionId(
    '018f0001-0000-7000-8000-000000000001',
  );

  /// The bundled rule that blocks `models.dev` (mirror of
  /// `BUNDLED_BLOCK_RULE` in the Rust fake).
  static const RuleId bundledBlockRule = RuleId(
    '018f0000-0000-7000-8000-0000000000a1',
  );

  /// The bundled rule itself: the one the default script decides `models.dev`
  /// with, so the rules screen shows the rule that took that decision.
  static final Rule bundledBlock = Rule(
    id: bundledBlockRule,
    action: RuleAction.block,
    matcher: const RuleMatcher(host: 'models.dev'),
    expires: const RuleExpiry.never(),
    bundled: true,
    note:
        'models.dev is a model index, not a dependency of any project. '
        'The bundled model list covers it.',
    createdAt: DateTime.utc(2026, 1, 1),
  );

  /// What a reload reports when the file was read without a finding.
  static const Diagnostic defaultReloadReport = Diagnostic(
    code: DiagnosticCodes.rulesReloaded,
    severity: Severity.info,
    title: 'Rule set reloaded',
    why: 'rules.yaml reloaded: 0 added, 0 changed, 0 removed',
  );

  /// What a reload reports when the file was edited into something the
  /// engine refuses: the field, the line and the reason, in the shape
  /// `daemon/crates/rules/src/parse.rs` writes them.
  static const Diagnostic brokenFileReport = Diagnostic(
    code: DiagnosticCodes.rulesFileInvalid,
    severity: Severity.error,
    title: 'Rule file invalid',
    why:
        'rules[2].match.host (line 12): host pattern "*npmjs.org" is '
        'invalid: a wildcard must be a whole label',
  );

  /// What `GetInfo` answers by default.
  static const DaemonInfo defaultInfo = DaemonInfo(
    daemonVersion: '0.0.0-fake',
    protoMajor: 1,
    protoMinor: 0,
    capabilities: <String>[
      DaemonInfo.fakeCapability,
      'sandbox.bwrap',
      'proxy.h1',
    ],
    sessionId: '018f0001-0000-7000-8000-000000000001',
  );

  /// The answer to `GetInfo`.
  final DaemonInfo info;

  /// What `GetInfo` throws instead of answering, when set.
  final Diagnostic? infoFailure;

  /// The script replayed on `subscribe`.
  final List<ScriptedEvent> script;

  /// Everything the fake knows so far.
  final FakeSessionState state = FakeSessionState();

  /// Every decision received, oldest first.
  final List<RecordedDecision> decisions = <RecordedDecision>[];

  /// The session rules, in evaluation order. They are evaluated before every
  /// saved rule and never reach a file (CONVENTIONS 4.5).
  final List<Rule> sessionRules = <Rule>[];

  /// The rules that would stand in `rules.yaml`, in evaluation order. A rule
  /// with an end time belongs here too: it is written down, it just stops
  /// applying at some point.
  final List<Rule> savedRules = <Rule>[];

  /// The rules that ship with the product. They are evaluated last, cannot be
  /// changed and are deliberately **not** part of [rules]: they belong to the
  /// product, not to the person, and a test that counts what a decision
  /// remembered must not count them.
  final List<Rule> bundledRules = <Rule>[bundledBlock];

  /// What `Rules(reload)` reports. The default is the `RULES_011` a daemon
  /// answers with when the file was read again; the `rules:broken` scenario
  /// replaces it with a refused file.
  List<Diagnostic> reloadDiagnostics = <Diagnostic>[defaultReloadReport];

  /// Die Momentaufnahme der Sandbox (HUM-040). Sie steht hier und nicht in
  /// [state], weil sie kein Fluss ist und weil ein Test sie ohne Umweg setzen
  /// koennen soll -- etwa auf `failed` mit einem blockierenden Befund.
  SandboxStatus sandbox = defaultSandbox_();

  /// Was ein Start meldet, statt zu laufen. `null` heisst: er laeuft.
  Diagnostic? sandboxStartFailure;

  /// Die drei Garantien, die ein Start und `IsolationCheck` melden.
  ///
  /// Voreingestellt sind die drei gruenen des Rust-Fakes, jede mit einem
  /// Beleg, der ausdruecklich sagt, dass nichts gemessen wurde
  /// (CONVENTIONS 4.7). Ein Test setzt hier eine rote Pruefung ein oder
  /// [fakeIsolationNoReport]: **drei rote Ergebnisse mit `SANDBOX_013`**, was
  /// der echte Daemon bei ausgebliebenem Bericht schickt
  /// (`BwrapBackend::isolation_check`). Er schickt nie null Ereignisse und nie
  /// zwei von drei; ein Fake, der das taete, uebte die Oberflaeche auf eine
  /// Form, die es nicht gibt.
  List<IsolationCheckResult> isolationChecks = fakeIsolationChecks();

  /// The rules the person created, session rules first. `Decide.remember`
  /// adds to it, `Rules(remove)` takes from it.
  List<Rule> get rules => <Rule>[...sessionRules, ...savedRules];

  int _ruleRevision = 0;
  int _ruleCount = 0;

  final DateTime Function() _clock;
  final StreamController<FlowEvent> _live =
      StreamController<FlowEvent>.broadcast();
  bool _offline = false;
  bool _closed = false;
  int _infoCalls = 0;

  /// How often `GetInfo` was called; the connection heartbeat shows here.
  int get infoCalls => _infoCalls;

  /// True after [close].
  bool get isClosed => _closed;

  /// From now on every call fails with `DAEMON_001`, as if the daemon had
  /// been stopped. [goOnline] reverses it.
  void goOffline() => _offline = true;

  /// The daemon is back.
  void goOnline() => _offline = false;

  @override
  Future<DaemonInfo> getInfo() async {
    _infoCalls++;
    _check();
    final Diagnostic? failure = infoFailure;
    if (failure != null) {
      throw DaemonException(failure);
    }
    return info;
  }

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) {
    _check();
    // The script and the live events (decisions taken while it still plays)
    // feed one controller, so a decision during the replay is not lost the
    // way it would be with `yield*` after the loop: a broadcast stream drops
    // what nobody listens to.
    final StreamController<FlowEvent> out = StreamController<FlowEvent>();
    StreamSubscription<FlowEvent>? live;
    final _CancellableDelay delay = _CancellableDelay();
    bool visible(FlowEvent event) =>
        includePassthrough || !_isPassthrough(event);
    out.onListen = () {
      live = _live.stream
          .where(visible)
          .listen(out.add, onError: out.addError, onDone: out.close);
      unawaited(_replay(out, visible, delay));
    };
    out.onCancel = () async {
      // Cancelling stops the replay at once and leaves no timer behind; a
      // widget test checks for pending timers once the tree is gone.
      delay.cancel();
      await live?.cancel();
    };
    return out.stream;
  }

  /// Plays [script] into [out] until it ends or [delay] is cancelled. A
  /// failing [_check] (offline, closed) ends the stream with the error, the
  /// way the transport would.
  Future<void> _replay(
    StreamController<FlowEvent> out,
    bool Function(FlowEvent event) visible,
    _CancellableDelay delay,
  ) async {
    final DateTime start = _clock();
    Duration elapsed = Duration.zero;
    for (final ScriptedEvent step in script) {
      final Duration wait = step.after - elapsed;
      if (wait > Duration.zero) {
        await delay.wait(wait);
      }
      elapsed = step.after;
      if (delay.cancelled || out.isClosed) {
        return;
      }
      try {
        _check();
      } on Object catch (error, stack) {
        out.addError(error, stack);
        await out.close();
        return;
      }
      final FlowEvent? event = step.build(state, start.add(step.after));
      if (event == null) {
        continue;
      }
      _apply(event);
      if (visible(event)) {
        out.add(event);
      }
    }
  }

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    _check();
    final Flow? flow = state.flow(id);
    if (flow == null || !flow.isHeld) {
      throw DaemonException(
        Diagnostic(
          code: DiagnosticCodes.flowNotHeld,
          severity: Severity.warning,
          why: 'flow ${id.value} is not held',
        ),
      );
    }
    // The rule first, the decision second, exactly as the daemon does it: a
    // decision that is refused must not leave a rule behind (HUM-027). The
    // flow above is held, so nothing is refused after this point.
    final Rule? created = _remember(remember);
    decisions.add(RecordedDecision(id, decision, remember));
    final DateTime now = _clock();
    final DecisionKind kind = decision.kind;
    _emit(
      FlowEvent.decided(
        at: now,
        flowId: id,
        kind: kind,
        source: DecisionSource.user,
        blockReason: decision is DecisionBlock ? decision.reason : null,
        note: decision is DecisionBlock ? decision.note ?? '' : '',
      ),
    );
    if (kind == DecisionKind.allow || kind == DecisionKind.allowEdited) {
      _emit(FlowEvent.forwarded(at: now, flowId: id));
      _emit(
        FlowEvent.responseHeaders(
          at: now,
          flowId: id,
          head: const HttpResponseHead(status: 200, version: 'HTTP/1.1'),
        ),
      );
    }
    _emit(FlowEvent.recorded(at: now, flowId: id));
    return created;
  }

  @override
  Future<void> removeRule(RuleId id) async {
    _check();
    _take(id);
    _ruleSetChanged();
  }

  @override
  Future<RuleSet> listRules() async {
    _check();
    return _ruleSet();
  }

  @override
  Future<RuleSet> addRule(Rule rule) async {
    _check();
    if (rule.bundled) {
      // Mirror of `RulesStore::validated`: the flag belongs to the product,
      // and nothing that arrives over the wire carries it.
      throw DaemonException(
        const Diagnostic(
          code: DiagnosticCodes.ruleBundled,
          severity: Severity.error,
          why: 'a rule that arrives over the wire is never bundled',
        ),
      );
    }
    _validate(rule);
    final RuleId? given = rule.id;
    if (given != null && _find(given) != null) {
      throw DaemonException(_duplicateRule(given));
    }
    final Rule stored = rule.copyWith(
      id: given ?? RuleId(_nextRuleId()),
      createdAt: rule.createdAt ?? _clock(),
    );
    _insert(stored, rule.position);
    _ruleSetChanged();
    return _ruleSet();
  }

  @override
  Future<RuleSet> updateRule(Rule rule) async {
    _check();
    final RuleId? id = rule.id;
    final Rule? current = id == null ? null : _find(id);
    if (id == null || current == null) {
      throw DaemonException(_unknownRule(id));
    }
    if (current.bundled) {
      throw DaemonException(_bundled(current, 'changed'));
    }
    _validate(rule);
    final List<Rule> group = _groupOf(current);
    final int at = group.indexWhere((Rule other) => other.id == id);
    final Rule next = rule.copyWith(createdAt: current.createdAt);
    if (_isSession(next) == _isSession(current)) {
      group[at] = next;
    } else {
      // The daemon moves a rule that changes its lifetime into the other
      // group and hangs it at the end: its old place belongs to a list it is
      // no longer in (`rules_store.rs`, `update`).
      group.removeAt(at);
      _groupOf(next).add(next);
    }
    _ruleSetChanged();
    return _ruleSet();
  }

  @override
  Future<RuleSet> reorderRules(List<RuleId> order) async {
    _check();
    _sortByOrder(sessionRules, order);
    _sortByOrder(savedRules, order);
    _ruleSetChanged();
    return _ruleSet();
  }

  @override
  Future<RuleSet> makeRulePermanent(RuleId id) async {
    _check();
    final Rule? current = _find(id);
    if (current == null) {
      throw DaemonException(_unknownRule(id));
    }
    if (current.bundled) {
      throw DaemonException(_bundled(current, 'changed'));
    }
    if (!_isSession(current)) {
      throw DaemonException(
        Diagnostic(
          code: DiagnosticCodes.rulesRequestInvalid,
          severity: Severity.error,
          why: 'the rule ${id.value} is already permanent',
        ),
      );
    }
    sessionRules.removeWhere((Rule rule) => rule.id == id);
    savedRules.add(current.copyWith(expires: const RuleExpiry.never()));
    _ruleSetChanged();
    return _ruleSet();
  }

  @override
  Future<RuleSet> reloadRules() async {
    _check();
    final List<Diagnostic> found = List<Diagnostic>.unmodifiable(
      reloadDiagnostics,
    );
    // A refused file leaves the rules that were in force in force; only a
    // reload without a failure would change anything here, and the fake holds
    // no file to read.
    return _ruleSet(diagnostics: found);
  }

  @override
  Future<DryRun> dryRunRule(Rule rule, {int limit = dryRunScanDefault}) async {
    _check();
    _validate(rule);
    // Zero means "as many as the contract says", not "none": the daemon reads
    // it that way (`daemon/crates/ipc/src/rules.rs`, `dry_run`).
    final int count = limit <= 0 ? dryRunScanDefault : limit;
    final DateTime now = _clock();
    final List<Flow> recorded = state.flows.values.toList()
      ..sort((Flow a, Flow b) => b.receivedAt.compareTo(a.receivedAt));
    final List<Flow> scanned = recorded.take(count).toList();
    return DryRun(
      matches: List<Flow>.unmodifiable(
        scanned.where((Flow flow) => ruleMatchesFlow(rule, flow, now: now)),
      ),
      scanned: scanned.length,
    );
  }

  /// Switches a bundled rule off or back on, the way `RulesStore` does.
  ///
  /// It follows the daemon and not the Rust fake: the store answers
  /// `RULES_010` for every id that names no bundled rule -- an unknown one
  /// and a rule of the person alike, with one sentence for both
  /// (`rules_store.rs`, `set_bundled_disabled`) -- while the Rust fake
  /// (`daemon/crates/ipc/src/fake/mod.rs`) silently ignores such a request
  /// although its comment claims parity. A screen that practised against the
  /// silent variant would learn that a refusal looks like a success.
  @override
  Future<RuleSet> setRuleDisabled(RuleId id, {required bool disabled}) async {
    _check();
    final int at = bundledRules.indexWhere((Rule rule) => rule.id == id);
    if (at < 0) {
      throw DaemonException(_notBundled(id));
    }
    bundledRules[at] = bundledRules[at].copyWith(disabled: disabled);
    _ruleSetChanged();
    return _ruleSet();
  }

  /// Stores [rule] the way the daemon does: an empty id is filled in, the
  /// rule joins its group and the event stream says the rule set changed.
  Rule? _remember(Rule? rule) {
    if (rule == null) {
      return null;
    }
    final Rule stored = rule.id == null
        ? rule.copyWith(id: RuleId(_nextRuleId()), createdAt: _clock())
        : rule;
    _insert(stored, stored.position);
    _ruleSetChanged();
    return stored;
  }

  /// The whole set in evaluation order, each rule carrying its one-based
  /// place inside its own group.
  RuleSet _ruleSet({List<Diagnostic> diagnostics = const <Diagnostic>[]}) {
    final List<Rule> out = <Rule>[];
    for (final List<Rule> group in <List<Rule>>[
      sessionRules,
      savedRules,
      bundledRules,
    ]) {
      for (int i = 0; i < group.length; i++) {
        out.add(group[i].copyWith(position: i + 1));
      }
    }
    return RuleSet(
      rules: List<Rule>.unmodifiable(out),
      diagnostics: diagnostics,
    );
  }

  void _ruleSetChanged() =>
      _emit(FlowEvent.rulesChanged(at: _clock(), revision: ++_ruleRevision));

  /// The rule with [id], from any of the three groups.
  Rule? _find(RuleId id) {
    for (final Rule rule in <Rule>[
      ...sessionRules,
      ...savedRules,
      ...bundledRules,
    ]) {
      if (rule.id == id) {
        return rule;
      }
    }
    return null;
  }

  /// Removes the rule with [id] and answers it, or throws the diagnostic the
  /// daemon would.
  Rule _take(RuleId id) {
    final Rule? current = _find(id);
    if (current == null) {
      throw DaemonException(_unknownRule(id));
    }
    if (current.bundled) {
      throw DaemonException(_bundled(current, 'removed'));
    }
    _groupOf(current).removeWhere((Rule rule) => rule.id == id);
    return current;
  }

  /// Puts [rule] at [position] inside its group; zero means "at the end"
  /// (`rules.proto`).
  void _insert(Rule rule, int position) {
    final List<Rule> group = _groupOf(rule);
    final int at = position <= 0 || position > group.length
        ? group.length
        : position - 1;
    group.insert(at, rule);
  }

  List<Rule> _groupOf(Rule rule) =>
      _isSession(rule) ? sessionRules : savedRules;

  static bool _isSession(Rule rule) => rule.expires is RuleExpirySession;

  /// Sorts [group] by [order]; ids it does not name keep their order and stay
  /// at the end (mirror of `sort_by_order`).
  static void _sortByOrder(List<Rule> group, List<RuleId> order) {
    final List<Rule> rest = List<Rule>.of(group);
    final List<Rule> sorted = <Rule>[];
    for (final RuleId id in order) {
      final int at = rest.indexWhere((Rule rule) => rule.id == id);
      if (at >= 0) {
        sorted.add(rest.removeAt(at));
      }
    }
    group
      ..clear()
      ..addAll(sorted)
      ..addAll(rest);
  }

  /// Refuses what the engine would refuse, with the code and the shape of
  /// `why` the daemon uses: field first, then the reason.
  void _validate(Rule rule) {
    final HostPatternProblem? host = hostPatternProblem(rule.matcher.host);
    if (host != null) {
      throw DaemonException(
        Diagnostic(
          code: DiagnosticCodes.hostPatternInvalid,
          severity: Severity.error,
          why:
              'match.host: host pattern "${rule.matcher.host}" is invalid: '
              '${_hostReason(host)}',
        ),
      );
    }
    if (pathPatternProblem(rule.matcher.path) != null) {
      throw DaemonException(
        Diagnostic(
          code: DiagnosticCodes.pathPatternInvalid,
          severity: Severity.error,
          why:
              'match.path: path regex ${rule.matcher.path} is invalid or '
              'too large',
        ),
      );
    }
  }

  static String _hostReason(HostPatternProblem problem) => switch (problem) {
    HostPatternProblem.empty => 'empty pattern',
    HostPatternProblem.wildcardInLabel => 'a wildcard must be a whole label',
    HostPatternProblem.emptyLabel => 'empty label',
    HostPatternProblem.notAnAddress => 'not an ip address',
    HostPatternProblem.notALabel => 'not a valid label',
  };

  /// Word for word the diagnostics of `daemon/crates/proxy/src/rules_store.rs`
  /// -- code, severity, sentence and fix. A client that learns another
  /// sentence here learns the wrong one.
  static Diagnostic _unknownRule(RuleId? id) => Diagnostic(
    code: DiagnosticCodes.rulesRequestInvalid,
    severity: Severity.error,
    why: 'there is no rule with the id ${id?.value ?? ''}',
  );

  static Diagnostic _duplicateRule(RuleId id) => Diagnostic(
    code: DiagnosticCodes.ruleIdDuplicate,
    severity: Severity.error,
    why: 'the id ${id.value} is already taken; every rule needs its own',
  );

  /// The refusal of a bundled rule carries the way around it as a `FixAction`,
  /// not as a sentence: an own `ask` rule with the same match, to stand in
  /// front of it.
  static Diagnostic _bundled(Rule rule, String verb) => Diagnostic(
    code: DiagnosticCodes.ruleBundled,
    severity: Severity.error,
    why: 'the rule ${rule.id?.value ?? ''} is bundled and cannot be $verb',
    fix: FixAction.addRule(
      rule: Rule(
        action: RuleAction.ask,
        matcher: rule.matcher,
        note: 'overrides the bundled rule above it',
      ),
    ),
  );

  /// The refusal of `Rules(set_disabled)` for anything but a bundled rule.
  ///
  /// One sentence for two cases, exactly as the store has it: an unknown id
  /// and a rule of the person get the same answer, because switching off is
  /// defined for bundled rules alone. No `FixAction` here -- the way around
  /// it is to delete the rule, and deleting is what the row already offers.
  static Diagnostic _notBundled(RuleId id) => Diagnostic(
    code: DiagnosticCodes.ruleBundled,
    severity: Severity.error,
    why:
        'there is no bundled rule with the id ${id.value}; only bundled '
        'rules are disabled instead of removed',
  );

  String _nextRuleId() =>
      '018f0003-0000-7000-8000-${(++_ruleCount).toRadixString(16).padLeft(12, '0')}';

  /// `ListFlows`: filtered, sorted and paged the way the recorder does it.
  ///
  /// The history screen is only honest if it practises against the real
  /// contract: the filter grammar of `daemon/crates/recorder/src/filter.rs`
  /// with its sixteen keys, keyset paging that never duplicates or skips a
  /// row, the four sort keys, and a total that stops counting at
  /// [fakeCountCeiling] and says so in `FlowPage.capped`, exactly as the
  /// recorder does (`backlog/CONVENTIONS.md` 4.14).
  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    _check();
    final FakeFlowFilter parsed = FakeFlowFilter.parse(filter.query, _clock());
    final (FakeSortKey sort, bool descending) = FakeFlowFilter.order(
      filter.orderBy,
    );
    final FlowId? since = filter.since;
    final List<Flow> all =
        state.flows.values
            .where(
              (Flow flow) => filter.includePassthrough || !flow.passthrough,
            )
            .where(parsed.matches)
            .where(
              (Flow flow) =>
                  since == null || flow.id.value.compareTo(since.value) > 0,
            )
            .toList()
          ..sort(
            (Flow a, Flow b) =>
                _compareFlows(sort, a, b) * (descending ? -1 : 1),
          );
    final int start = _cursorStart(all, cursor, sort, descending);
    final int size = limit <= 0 ? 200 : (limit > 1000 ? 1000 : limit);
    final int end = start + size < all.length ? start + size : all.length;
    final List<Flow> rows = all.sublist(start, end);
    final bool capped = all.length > fakeCountCeiling;
    return FlowPage(
      flows: rows,
      nextCursor: end < all.length && rows.isNotEmpty
          ? _encodeCursor(rows.last, sort)
          : '',
      total: capped ? fakeCountCeiling : all.length,
      capped: capped,
    );
  }

  /// Where the page after [cursor] begins.
  ///
  /// The id decides, because it is unique and because the row it names is
  /// still in the list in every normal case. Only when that row has gone --
  /// a retention run, a filter that no longer takes it -- does the keyset
  /// comparison take over, and it lands on the first row beyond the cursor
  /// rather than at the top: a page that started over would repeat rows the
  /// person has already read.
  int _cursorStart(
    List<Flow> rows,
    String? cursor,
    FakeSortKey sort,
    bool descending,
  ) {
    if (cursor == null || cursor.isEmpty) {
      return 0;
    }
    final (int ts, String id) = _decodeCursor(cursor);
    final int index = rows.indexWhere((Flow flow) => flow.id.value == id);
    if (index >= 0) {
      return index + 1;
    }
    final DateTime at = DateTime.fromMillisecondsSinceEpoch(ts);
    for (int i = 0; i < rows.length; i++) {
      final int order =
          rows[i].receivedAt.compareTo(at) * (descending ? -1 : 1);
      if (order > 0) {
        return i;
      }
    }
    return rows.length;
  }

  static String _encodeCursor(Flow flow, FakeSortKey sort) {
    final String sortPart = switch (sort) {
      FakeSortKey.ts => '',
      FakeSortKey.host => 't${flow.host}',
      FakeSortKey.duration => 'i${flow.duration?.inMilliseconds ?? -1}',
      FakeSortKey.size => 'i${flow.requestSize + flow.responseSize}',
    };
    final String text =
        '${flow.receivedAt.millisecondsSinceEpoch}\u001f${flow.id.value}'
        '\u001f$sortPart';
    return base64Url.encode(utf8.encode(text)).replaceAll('=', '');
  }

  (int, String) _decodeCursor(String cursor) {
    try {
      final String padded = cursor.padRight(
        cursor.length + (4 - cursor.length % 4) % 4,
        '=',
      );
      final List<String> parts = utf8
          .decode(base64Url.decode(padded))
          .split('\u001f');
      return (int.parse(parts[0]), parts[1]);
    } on Object {
      throw DaemonException(
        const Diagnostic(
          code: 'IPC_005',
          severity: Severity.error,
          why:
              'that is not a cursor from this daemon; ask again without a '
              'cursor to start at the first page',
        ),
      );
    }
  }

  static int _compareFlows(FakeSortKey sort, Flow a, Flow b) {
    final int primary = switch (sort) {
      FakeSortKey.ts => 0,
      FakeSortKey.host => a.host.compareTo(b.host),
      FakeSortKey.duration => (a.duration?.inMilliseconds ?? -1).compareTo(
        b.duration?.inMilliseconds ?? -1,
      ),
      FakeSortKey.size => (a.requestSize + a.responseSize).compareTo(
        b.requestSize + b.responseSize,
      ),
    };
    if (primary != 0) {
      return primary;
    }
    final int arrival = a.receivedAt.compareTo(b.receivedAt);
    return arrival != 0 ? arrival : a.id.compareTo(b.id);
  }

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    _check();
    final FlowDetail? detail = state.details[id];
    if (detail != null) {
      return detail;
    }
    final Flow? flow = state.flow(id);
    if (flow == null) {
      throw DaemonException(
        Diagnostic(
          code: DiagnosticCodes.flowNotHeld,
          severity: Severity.warning,
          why: 'unknown flow ${id.value}',
        ),
      );
    }
    return FlowDetail(summary: flow);
  }

  @override
  Stream<Uint8List> getBody(BodyRef ref) async* {
    _check();
    final Uint8List? body = state.bodies[_hex(ref.sha256)];
    if (body != null && body.isNotEmpty) {
      yield body;
    }
  }

  // --- Sandbox (HUM-040) ---------------------------------------------------
  //
  // Der Fake spielt genau die Momentaufnahme, die der echte Daemon liefert:
  // dieselben Einhaengungen, dieselbe Umgebung samt einem Geheimnis und
  // dieselbe Kommandozeile. Sie ist als Fake gekennzeichnet, damit niemand
  // eine gemessene Zeile fuer eine gespielte haelt (CONVENTIONS 4.7).

  @override
  Stream<SandboxUpdate> sandboxStatus() async* {
    _check();
    // Wie beim echten Daemon: der Befund ist ein eigenes Ereignis und steckt
    // nicht in der Momentaufnahme. Der Vertrag kennt in `SandboxEvent.Status`
    // kein Feld dafuer, und ein Fake, der eines erfaende, pruefte einen Weg,
    // den es nicht gibt (CONVENTIONS 4.7).
    for (final Diagnostic diagnostic in sandbox.diagnostics) {
      yield SandboxUpdate.diagnostic(diagnostic);
    }
    yield SandboxUpdate.status(sandbox);
  }

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    _check();
    sandbox = sandbox.copyWith(
      profile: profile ?? sandbox.profile,
      workDirHost: workDir ?? sandbox.workDirHost,
      workMode: workMode ?? sandbox.workMode,
    );
    sandbox = sandbox.copyWith(
      mounts: fakeSandboxMounts(
        workDir: sandbox.workDirHost ?? '',
        workMode: sandbox.workMode,
      ),
      argvPreview: fakeSandboxArgv(
        workDir: sandbox.workDirHost ?? '',
        workMode: sandbox.workMode,
      ),
    );
    yield SandboxUpdate.status(sandbox);
  }

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) async* {
    _check();
    await for (final SandboxUpdate update in planSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    )) {
      // Der Plan ist nur die Vorbereitung; gemeldet wird er als `starting`.
      if (update is SandboxUpdateStatus) {
        sandbox = update.status.copyWith(state: SandboxState.starting);
        yield SandboxUpdate.status(sandbox);
      }
    }
    final Diagnostic? refusal = sandboxStartFailure;
    if (refusal != null) {
      sandbox = sandbox.copyWith(
        state: SandboxState.failed,
        agentRunning: false,
        diagnostics: <Diagnostic>[...sandbox.diagnostics, refusal],
      );
      yield SandboxUpdate.diagnostic(refusal);
      yield SandboxUpdate.status(sandbox);
      return;
    }
    // Wie beim echten Daemon: die drei Garantien stehen zwischen `starting`
    // und `running`, sie reisen als eigene Ereignisse und nicht in der
    // Momentaufnahme, und eine rote beendet den Start (HUM-041).
    for (final IsolationCheckResult result in isolationChecks) {
      yield SandboxUpdate.check(result);
    }
    final Diagnostic? isolationFailure = _isolationFailure();
    if (isolationFailure != null) {
      sandbox = sandbox.copyWith(
        state: SandboxState.failed,
        agentRunning: false,
        diagnostics: <Diagnostic>[...sandbox.diagnostics, isolationFailure],
      );
      yield SandboxUpdate.diagnostic(isolationFailure);
      yield SandboxUpdate.status(sandbox);
      return;
    }
    sandbox = sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      startedAt: _clock(),
      sandboxId: defaultSandbox,
    );
    yield SandboxUpdate.log(
      SandboxLogLine(at: _clock(), text: 'sandbox started, pid 4711 (fake)'),
    );
    yield SandboxUpdate.status(sandbox);
  }

  @override
  Stream<SandboxUpdate> stopSandbox() async* {
    _check();
    sandbox = sandbox.copyWith(state: SandboxState.stopping);
    yield SandboxUpdate.status(sandbox);
    sandbox = sandbox.copyWith(
      state: SandboxState.stopped,
      agentRunning: false,
      startedAt: null,
      sandboxId: null,
    );
    yield SandboxUpdate.log(
      SandboxLogLine(at: _clock(), text: 'sandbox stopped (fake)'),
    );
    yield SandboxUpdate.status(sandbox);
  }

  @override
  Stream<SandboxUpdate> checkIsolation() async* {
    _check();
    // Wie beim echten Daemon: ohne laufende Sandbox ist nichts gemessen, und
    // was nicht gemessen wurde, wird nicht als Ergebnis gesendet.
    if (sandbox.state != SandboxState.running) {
      yield SandboxUpdate.status(sandbox);
      return;
    }
    for (final IsolationCheckResult result in isolationChecks) {
      yield SandboxUpdate.check(result);
    }
  }

  /// Der Befund, der einen Start beendet, oder `null`, wenn alle drei
  /// Garantien belegt sind. Eine leere Liste ist kein Durchlauf.
  Diagnostic? _isolationFailure() {
    if (isolationChecks.isEmpty) {
      return const Diagnostic(
        code: DiagnosticCodes.isolationNoReport,
        severity: Severity.blocking,
        title: 'Isolation check without a report',
        why:
            'the sandbox reported no isolation check at all; nothing about '
            'its isolation is proven',
      );
    }
    for (final IsolationCheckResult result in isolationChecks) {
      if (!result.passed) {
        // Rot bleibt rot, auch ohne Befund daneben: geprueft wird `passed`,
        // nie das Vorhandensein des Befunds. Derselbe Boden wie `code_of` im
        // Daemon (`daemon/crates/ipc/src/sandbox.rs`).
        return result.diagnostic ??
            Diagnostic(
              code: _isolationCodeOf(result.check),
              severity: Severity.blocking,
              title: 'Isolation check failed',
              why:
                  '${result.check.name}: ${result.evidence} '
                  '(no finding came with it)',
            );
      }
    }
    return null;
  }

  @override
  Future<void> close() async {
    _closed = true;
    await _live.close();
  }

  void _check() {
    if (_closed) {
      throw StateError('FakeDaemonClient is closed');
    }
    if (_offline) {
      throw DaemonException(
        ClientDiagnostics.daemonUnreachable(
          socketPath: defaultSocket,
          detail: 'connection refused',
          fake: true,
        ),
      );
    }
  }

  void _emit(FlowEvent event) {
    _apply(event);
    _live.add(event);
  }

  bool _isPassthrough(FlowEvent event) {
    final FlowId? id = event.flowId;
    return id != null && (state.flow(id)?.passthrough ?? false);
  }

  /// Keeps [state] in step with the events, the way the daemon's automaton
  /// would; the queue of HUM-020 reads the same events, so the two agree.
  void _apply(FlowEvent event) {
    switch (event) {
      case FlowEventReceived(:final flow):
        state.flows[flow.id] = flow;
      case FlowEventAnalyzed(:final flowId, :final findings):
        state.update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.analyzed,
            findingCount: findings.length,
          ),
        );
        final FlowDetail? detail = state.details[flowId];
        if (detail != null) {
          state.details[flowId] = detail.copyWith(findings: findings);
        }
      case FlowEventHeld(:final flowId, :final deadline):
        state.update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.held, deadline: deadline),
        );
      case FlowEventDecided(
        :final flowId,
        :final kind,
        :final source,
        :final blockReason,
        :final ruleId,
      ):
        state.update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.decided,
            decision: kind,
            decisionSource: source,
            blockReason: blockReason,
            ruleId: ruleId,
            edited: kind == DecisionKind.allowEdited,
            deadline: null,
            status: kind == DecisionKind.block
                ? 403
                : kind == DecisionKind.timedOut
                ? 504
                : flow.status,
          ),
        );
      case FlowEventForwarded(:final flowId):
        state.update(
          flowId,
          (Flow flow) => flow.copyWith(state: FlowState.forwarded),
        );
      case FlowEventResponseHeaders(:final flowId, :final head):
        state.update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.responded, status: head.status),
        );
      case FlowEventRecorded(:final flowId):
        state.update(
          flowId,
          (Flow flow) => flow.copyWith(state: FlowState.recorded),
        );
      case FlowEventTimedOut(:final flowId):
        state.update(
          flowId,
          (Flow flow) => flow.copyWith(
            state: FlowState.decided,
            decision: DecisionKind.timedOut,
            decisionSource: DecisionSource.timeout,
            blockReason: BlockReason.timeout,
            status: 504,
            deadline: null,
          ),
        );
      case FlowEventFailed(:final flowId, :final error):
        state.update(
          flowId,
          (Flow flow) =>
              flow.copyWith(state: FlowState.failed, upstreamError: error),
        );
      case FlowEventResponseChunk() ||
          FlowEventLagged() ||
          FlowEventDiagnostic() ||
          FlowEventRulesChanged() ||
          FlowEventAgentAsk():
        break;
    }
  }

  static String _hex(List<int> bytes) =>
      bytes.map((int b) => b.toRadixString(16).padLeft(2, '0')).join();

  /// [count] held requests to well-known hosts, one every [spacing], each
  /// held for [budget]; the queue of HUM-020 is measured against it.
  static List<ScriptedEvent> burstScript({
    required int count,
    required Duration spacing,
    required Duration budget,
  }) {
    const List<(Method, String, String)> targets = <(Method, String, String)>[
      (Method.get, 'registry.npmjs.org', '/react'),
      (Method.post, 'api.github.com', '/graphql'),
      (Method.get, 'pypi.org', '/simple/requests/'),
      (Method.put, 'storage.googleapis.com', '/bucket/object.json'),
      (Method.get, 'crates.io', '/api/v1/crates/serde'),
      (Method.delete, 'api.example.org', '/v1/items/42'),
    ];
    final List<ScriptedEvent> events = <ScriptedEvent>[];
    for (int i = 0; i < count; i++) {
      final (Method method, String host, String path) =
          targets[i % targets.length];
      final _ScriptedFlow flow = _ScriptedFlow(
        id: FlowId(
          '018f0002-0000-7000-8000-${(i + 1).toRadixString(16).padLeft(12, '0')}',
        ),
        method: method,
        host: host,
        path: '$path?n=$i',
        originTool: 'burst',
        body: method == Method.get ? '' : '{"n": $i}',
      );
      final Duration at = spacing * i;
      events
        ..add(flow.received(at))
        ..add(flow.analyzed(at + const Duration(milliseconds: 5), const []))
        ..add(flow.held(at + const Duration(milliseconds: 10), budget));
    }
    return events;
  }

  /// One held request whose body is eight mebibytes of JSON.
  ///
  /// The point of the scenario is what the surface does with it: the queue has
  /// to stay scrollable while the body is taken apart, and the tree has to
  /// appear (HUM-030 acceptance criteria). The address in the last record is
  /// reported as a finding, so the chips and the jump have a target.
  static List<ScriptedEvent> bigBodyScript() {
    final String body = bigBodyJson();
    final int mail = body.lastIndexOf('big.body@example.org');
    final _ScriptedFlow flow = _ScriptedFlow(
      id: const FlowId('018f0004-0000-7000-8000-000000000001'),
      method: Method.post,
      host: 'api.example.org',
      path: '/v1/import',
      originTool: 'opencode',
      headers: const <Header>[Header(name: 'content-type', value: <int>[])],
      body: body,
    );
    return <ScriptedEvent>[
      flow.received(const Duration(milliseconds: 50)),
      flow.analyzed(const Duration(milliseconds: 80), <Finding>[
        Finding(
          kind: 'email',
          location: FindingLocation.body,
          spanStart: mail,
          spanEnd: mail + 'big.body@example.org'.length,
          tier: FindingTier.regex,
          displayPrefix: 'big.body@',
        ),
      ]),
      flow.held(
        const Duration(milliseconds: 100),
        const Duration(seconds: 300),
      ),
    ];
  }

  /// Eight mebibytes of JSON, deterministic, with one address in the last
  /// record.
  static String bigBodyJson() {
    const int limit = 8 * 1024 * 1024;
    final StringBuffer buffer = StringBuffer('{"records":[');
    int i = 0;
    while (buffer.length < limit - 256) {
      if (i > 0) {
        buffer.write(',');
      }
      buffer.write('{"id":$i,"name":"record $i","tags":["a","b"],"ok":true}');
      i++;
    }
    buffer.write(',{"id":$i,"mail":"big.body@example.org"}]}');
    return buffer.toString();
  }

  /// The default script: a cousin of `fixtures/sessions/mixed.jsonl`.
  ///
  /// Three held requests (GitHub with two findings, httpbin with a JWT, a
  /// WebSocket upgrade), one rule block with a note, one LLM passthrough, one
  /// short hold that times out after five seconds, one `TLS_001` diagnostic.
  static List<ScriptedEvent> defaultScript() {
    const Duration holdBudget = Duration(seconds: 300);
    final _ScriptedFlow github = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000010000'),
      method: Method.post,
      host: 'api.github.com',
      path: '/graphql',
      originTool: 'opencode',
      headers: const <Header>[
        Header(name: 'user-agent', value: <int>[]),
        Header(name: 'content-type', value: <int>[]),
      ],
      body:
          '{"query": "mutation { createIssue }", '
          '"variables": {"body": "Gemeldet von nils.hoffmann@acme-labs.io"}}',
    );
    final _ScriptedFlow models = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000020000'),
      method: Method.get,
      host: 'models.dev',
      path: '/api.json',
      originTool: 'opencode',
    );
    final _ScriptedFlow llm = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000030000'),
      method: Method.post,
      scheme: Scheme.http,
      host: '192.168.1.50',
      port: 11434,
      isIp: true,
      path: '/api/chat',
      passthrough: true,
      body: '{"model":"llama3","messages":[]}',
    );
    final _ScriptedFlow example = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000040000'),
      method: Method.get,
      host: 'example.org',
      path: '/',
      originTool: 'curl',
    );
    final _ScriptedFlow httpbin = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000050000'),
      method: Method.post,
      host: 'httpbin.org',
      path: '/post',
      originTool: 'opencode',
      body: '{"token":"eyJhbGciOiJIUzI1NiJ9.e30.x"}',
    );
    final _ScriptedFlow ws = _ScriptedFlow(
      id: const FlowId('018f0001-0000-7000-8000-000000070000'),
      method: Method.get,
      scheme: Scheme.wss,
      host: 'ws.example.org',
      path: '/agent',
      originTool: 'opencode',
    );

    return <ScriptedEvent>[
      github.received(const Duration(milliseconds: 400)),
      github.analyzed(const Duration(milliseconds: 430), const <Finding>[
        Finding(
          kind: 'api_key.github',
          location: FindingLocation.header,
          headerName: 'authorization',
          spanStart: 7,
          spanEnd: 47,
          tier: FindingTier.checksum,
          displayPrefix: 'ghp_R8kQ',
        ),
        Finding(
          kind: 'email',
          location: FindingLocation.body,
          spanStart: 239,
          spanEnd: 265,
          tier: FindingTier.regex,
          displayPrefix: 'nils.hof',
        ),
      ]),
      github.held(const Duration(milliseconds: 450), holdBudget),
      models.received(const Duration(milliseconds: 1200)),
      models.analyzed(const Duration(milliseconds: 1210), const <Finding>[]),
      ScriptedEvent(
        const Duration(milliseconds: 1260),
        (FakeSessionState _, DateTime now) => FlowEvent.decided(
          at: now,
          flowId: models.id,
          kind: DecisionKind.block,
          source: DecisionSource.rule,
          blockReason: BlockReason.rule,
          ruleId: bundledBlockRule,
          note:
              'models.dev is not needed for this project; use the bundled '
              'model list.',
        ),
      ),
      models.recorded(const Duration(milliseconds: 1300)),
      llm.received(const Duration(milliseconds: 2600)),
      ScriptedEvent(
        const Duration(milliseconds: 2601),
        (FakeSessionState _, DateTime now) => FlowEvent.decided(
          at: now,
          flowId: llm.id,
          kind: DecisionKind.allow,
          source: DecisionSource.passthrough,
        ),
      ),
      llm.forwarded(const Duration(milliseconds: 2602)),
      llm.responded(const Duration(milliseconds: 2900), 200),
      llm.recorded(const Duration(milliseconds: 2901)),
      example.received(const Duration(milliseconds: 3000)),
      example.analyzed(const Duration(milliseconds: 3010), const <Finding>[]),
      example.held(
        const Duration(milliseconds: 3020),
        const Duration(seconds: 5),
      ),
      httpbin.received(const Duration(milliseconds: 4000)),
      httpbin.analyzed(const Duration(milliseconds: 4030), const <Finding>[
        Finding(
          kind: 'jwt',
          location: FindingLocation.body,
          spanStart: 10,
          spanEnd: 38,
          tier: FindingTier.regex,
          displayPrefix: 'eyJhbGci',
        ),
      ]),
      httpbin.held(const Duration(milliseconds: 4050), holdBudget),
      ScriptedEvent(
        const Duration(milliseconds: 5000),
        (FakeSessionState _, DateTime now) => FlowEvent.diagnostic(
          at: now,
          diagnostic: const Diagnostic(
            code: 'TLS_001',
            severity: Severity.warning,
            title: 'CA nicht vertraut',
            why: 'curl in the sandbox does not trust the Humanitl CA yet',
            fix: FixAction.setEnv(
              key: 'CURL_CA_BUNDLE',
              value: '/etc/humanitl/ca.crt',
            ),
          ),
        ),
      ),
      ws.received(const Duration(milliseconds: 6000)),
      ws.analyzed(const Duration(milliseconds: 6010), const <Finding>[]),
      ws.held(const Duration(milliseconds: 6020), holdBudget),
      // The five second hold of example.org expires unless somebody decided.
      ScriptedEvent(const Duration(milliseconds: 8020), (
        FakeSessionState state,
        DateTime now,
      ) {
        final Flow? flow = state.flow(example.id);
        return flow != null && flow.isHeld
            ? FlowEvent.timedOut(at: now, flowId: example.id)
            : null;
      }),
      ScriptedEvent(const Duration(milliseconds: 8030), (
        FakeSessionState state,
        DateTime now,
      ) {
        final Flow? flow = state.flow(example.id);
        return flow != null && flow.decision == DecisionKind.timedOut
            ? FlowEvent.recorded(at: now, flowId: example.id)
            : null;
      }),
    ];
  }
}

/// Builder of the events of one scripted flow.
class _ScriptedFlow {
  _ScriptedFlow({
    required this.id,
    required this.method,
    this.scheme = Scheme.https,
    required this.host,
    int? port,
    this.isIp = false,
    required this.path,
    this.originTool = '',
    this.passthrough = false,
    this.headers = const <Header>[],
    String body = '',
  }) : port = port ?? scheme.defaultPort,
       bodyBytes = Uint8List.fromList(utf8.encode(body));

  final FlowId id;
  final Method method;
  final Scheme scheme;
  final String host;
  final int port;
  final bool isIp;
  final String path;
  final String originTool;
  final bool passthrough;
  final List<Header> headers;
  final Uint8List bodyBytes;

  BodyRef get bodyRef => BodyRef(
    sha256: List<int>.filled(32, id.value.hashCode & 0xff),
    size: bodyBytes.length,
    contentType: bodyBytes.isEmpty ? '' : 'application/json',
  );

  Flow summary(DateTime now) => Flow(
    id: id,
    sessionId: FakeDaemonClient.defaultSession,
    receivedAt: now,
    method: method,
    scheme: scheme,
    authority: Authority(host: host, port: port, isIpLiteral: isIp),
    path: path,
    state: FlowState.received,
    requestSize: bodyBytes.length,
    passthrough: passthrough,
    originTool: originTool,
  );

  ScriptedEvent received(Duration after) =>
      ScriptedEvent(after, (FakeSessionState state, DateTime now) {
        final Flow flow = summary(now);
        state.details[id] = FlowDetail(
          summary: flow,
          request: HttpRequest(
            method: method,
            scheme: scheme,
            authority: flow.authority,
            pathAndQuery: path,
            headers: headers,
            body: bodyRef,
            version: 'HTTP/1.1',
          ),
          bodyPreview: utf8.decode(bodyBytes, allowMalformed: true),
        );
        state.bodies[_hexOf(bodyRef.sha256)] = bodyBytes;
        return FlowEvent.received(at: now, flow: flow);
      });

  ScriptedEvent analyzed(Duration after, List<Finding> findings) =>
      ScriptedEvent(
        after,
        (FakeSessionState _, DateTime now) =>
            FlowEvent.analyzed(at: now, flowId: id, findings: findings),
      );

  ScriptedEvent held(Duration after, Duration budget) => ScriptedEvent(
    after,
    (FakeSessionState state, DateTime now) => FlowEvent.held(
      at: now,
      flowId: id,
      deadline: now.add(budget),
      queueCount: state.flows.values.where((Flow f) => f.isHeld).length + 1,
    ),
  );

  ScriptedEvent forwarded(Duration after) => ScriptedEvent(
    after,
    (FakeSessionState _, DateTime now) =>
        FlowEvent.forwarded(at: now, flowId: id),
  );

  ScriptedEvent responded(Duration after, int status) => ScriptedEvent(
    after,
    (FakeSessionState _, DateTime now) => FlowEvent.responseHeaders(
      at: now,
      flowId: id,
      head: HttpResponseHead(status: status, version: 'HTTP/1.1'),
    ),
  );

  ScriptedEvent recorded(Duration after) => ScriptedEvent(
    after,
    (FakeSessionState _, DateTime now) =>
        FlowEvent.recorded(at: now, flowId: id),
  );

  static String _hexOf(List<int> bytes) =>
      bytes.map((int b) => b.toRadixString(16).padLeft(2, '0')).join();
}

/// A delay that can be cut short: [wait] completes early, and [cancelled]
/// turns true, when [cancel] is called. The timer is cancelled, not left to
/// fire.
class _CancellableDelay {
  Timer? _timer;
  Completer<void>? _completer;
  bool _cancelled = false;

  /// True after [cancel].
  bool get cancelled => _cancelled;

  /// Completes after [duration], or at once when cancelled.
  Future<void> wait(Duration duration) {
    if (_cancelled) {
      return Future<void>.value();
    }
    final Completer<void> completer = Completer<void>();
    _completer = completer;
    _timer = Timer(duration, () {
      _timer = null;
      _completer = null;
      completer.complete();
    });
    return completer.future;
  }

  /// Stops the pending wait, if any, and every later one.
  void cancel() {
    _cancelled = true;
    _timer?.cancel();
    _timer = null;
    final Completer<void>? completer = _completer;
    _completer = null;
    if (completer != null && !completer.isCompleted) {
      completer.complete();
    }
  }
}

/// True when [rule] alone would have matched [flow] at [now].
///
/// The fake evaluates rules because the daemon does; no screen ever calls
/// this. Every line mirrors `RuleSet::evaluate` and `CompiledRule::matches`
/// in `daemon/crates/rules/src/eval.rs`, because a fake that is more
/// permissive than the engine teaches every test a rule the daemon does not
/// keep.
bool ruleMatchesFlow(Rule rule, Flow flow, {required DateTime now}) {
  final RuleMatcher matcher = rule.matcher;
  // A method the contract does not know never matches anything: the engine
  // answers `Verdict::Default` for it before it looks at a single rule.
  if (flow.method == Method.other) {
    return false;
  }
  if (ruleExpiredAt(rule, now)) {
    return false;
  }
  // The upgrade dimension is symmetric in both directions: a rule without
  // `upgrade` never matches an upgrade, a rule with it never matches an
  // ordinary request. A WebSocket is a different thing from a GET to the same
  // host, and ADR-007 wants its own decision for it.
  final bool flowUpgrades =
      flow.scheme == Scheme.ws || flow.scheme == Scheme.wss;
  if (flowUpgrades != (matcher.upgrade == Upgrade.websocket)) {
    return false;
  }
  if (!hostPatternMatches(matcher.host, flow.host)) {
    return false;
  }
  if (matcher.methods.isNotEmpty && !matcher.methods.contains(flow.method)) {
    return false;
  }
  if (matcher.scheme != null && matcher.scheme != flow.scheme) {
    return false;
  }
  if (matcher.port != 0 && matcher.port != flow.authority.port) {
    return false;
  }
  if (matcher.path.isNotEmpty && !pathPatternMatches(matcher.path, flow.path)) {
    return false;
  }
  return true;
}

/// True when the host glob [pattern] matches [host].
///
/// Labels are compared as labels, never as text: `*.github.com` matches
/// neither `evil-github.com` nor `github.com.evil.io`. A pattern that starts
/// with `**` also matches the name without those labels, so `**.example.com`
/// matches `example.com` itself.
bool hostPatternMatches(String pattern, String host) {
  if (pattern.isEmpty) {
    return false;
  }
  if (pattern.startsWith('ip:')) {
    return pattern.substring(3) == host;
  }
  if (pattern.startsWith('cidr:')) {
    // A network needs address arithmetic the fake has no reason to carry;
    // saying "no match" promises less than a guess would.
    return false;
  }
  if (!pattern.contains('*')) {
    return pattern.toLowerCase() == host.toLowerCase();
  }
  return _globMatches(
    pattern.toLowerCase().split('.'),
    host.toLowerCase().split('.'),
  );
}

/// The comparison of `host::glob_matches`, label by label.
bool _globMatches(List<String> pattern, List<String> labels) {
  if (_matchesFrom(pattern, labels)) {
    return true;
  }
  // The apex exception: `**.example.com` matches `example.com`. It holds only
  // for a leading `**` and only when something follows it -- a `**` in the
  // middle of a pattern still wants at least one label.
  return pattern.length > 1 &&
      pattern.first == '**' &&
      _matchesFrom(pattern.sublist(1), labels);
}

bool _matchesFrom(List<String> pattern, List<String> labels) {
  if (pattern.isEmpty) {
    return labels.isEmpty;
  }
  final String head = pattern.first;
  final List<String> rest = pattern.sublist(1);
  if (head == '**') {
    // One or more labels, never zero.
    for (int taken = 1; taken <= labels.length; taken++) {
      if (_matchesFrom(rest, labels.sublist(taken))) {
        return true;
      }
    }
    return false;
  }
  if (labels.isEmpty) {
    return false;
  }
  if (head != '*' && head != labels.first) {
    return false;
  }
  return _matchesFrom(rest, labels.sublist(1));
}

/// True when the path pattern matches [pathAndQuery].
///
/// A pattern that starts with `~` is an unanchored regular expression; every
/// other pattern is a glob in which `*` stops at a `/` and `**` does not.
bool pathPatternMatches(String pattern, String pathAndQuery) {
  final String path = pathAndQuery.split('?').first;
  if (pattern.startsWith('~')) {
    try {
      return RegExp(pattern.substring(1)).hasMatch(path);
    } on FormatException {
      return false;
    }
  }
  final StringBuffer expression = StringBuffer('^');
  for (int i = 0; i < pattern.length; i++) {
    final String char = pattern[i];
    if (char == '*') {
      if (i + 1 < pattern.length && pattern[i + 1] == '*') {
        expression.write('.*');
        i++;
      } else {
        expression.write('[^/]*');
      }
      continue;
    }
    expression.write(RegExp.escape(char));
  }
  expression.write(r'$');
  return RegExp(expression.toString()).hasMatch(path);
}

/// How far the fake counts before `FlowPage.total` becomes a lower bound.
///
/// The same ceiling the recorder uses (`COUNT_CEILING`, ten thousand), so the
/// history screen practises against the number it will really see: at the
/// ceiling the total means "at least this many" (`backlog/CONVENTIONS.md`
/// 4.14).
const int fakeCountCeiling = 10000;

/// Every key of the recorder's filter language, in the order the daemon lists
/// them in its refusal (`daemon/crates/recorder/src/filter.rs`, `KEYS`).
const List<String> fakeFilterKeys = <String>[
  'host',
  'apex',
  'state',
  'method',
  'decision',
  'reason',
  'rule',
  'status',
  'since',
  'until',
  'findings',
  'session',
  'path',
  'edited',
  'passthrough',
  'upgrade',
  // `meta:true` is exactly the requests to `humanitl.internal` the proxy
  // answered itself, `meta:false` exactly the rest; without the term both
  // appear (HUM-103). It stands next to `decision:`, not inside it: nobody
  // decided about a meta request, so `decision:allow` and `decision:block`
  // leave one out on their own.
  'meta',
];

/// What a page is sorted by in the fake; the four keys of `SortKey`.
enum FakeSortKey {
  /// Arrival time.
  ts,

  /// Target host.
  host,

  /// Duration.
  duration,

  /// Request plus response size.
  size,
}

/// The filter expression of `ListFlows`, as far as an in-memory list needs it.
///
/// The grammar is the recorder's, and so is the refusal: an unknown key comes
/// back as `RECORDER_002` naming every valid key, because the surface must
/// practise against the answer it will really get (`backlog/CONVENTIONS.md`
/// 4.12: the fake reports the same codes as the daemon).
class FakeFlowFilter {
  FakeFlowFilter._(this._tests);

  final List<bool Function(Flow flow)> _tests;

  /// A filter that takes every row.
  static FakeFlowFilter get all => FakeFlowFilter._(const []);

  /// Reads [input]; [now] is the reference point of `since:10m`.
  ///
  /// Throws a [DaemonException] with `RECORDER_002` for an unknown key, a
  /// missing value, a number that is none, an unreadable time or a comparison
  /// on a key that takes none.
  static FakeFlowFilter parse(String input, DateTime now) {
    final List<bool Function(Flow flow)> tests = <bool Function(Flow flow)>[];
    for (final String term in _tokenize(input)) {
      tests.add(_translate(term, now));
    }
    return FakeFlowFilter._(tests);
  }

  /// True when [flow] passes every term.
  bool matches(Flow flow) {
    for (final bool Function(Flow flow) test in _tests) {
      if (!test(flow)) {
        return false;
      }
    }
    return true;
  }

  static List<String> _tokenize(String input) {
    final List<String> terms = <String>[];
    final StringBuffer current = StringBuffer();
    bool quoted = false;
    bool started = false;
    for (final int rune in input.runes) {
      final String char = String.fromCharCode(rune);
      if (char == '"') {
        quoted = !quoted;
        started = true;
        current.write(char);
      } else if (!quoted && char.trim().isEmpty) {
        if (started) {
          terms.add(current.toString());
          current.clear();
          started = false;
        }
      } else {
        started = true;
        current.write(char);
      }
    }
    if (started) {
      terms.add(current.toString());
    }
    return terms.where((String term) => term.isNotEmpty).toList();
  }

  static bool Function(Flow flow) _translate(String term, DateTime now) {
    final (String? key, String rest) = _splitKey(term);
    if (key == null) {
      final String word = _unquote(term).toLowerCase();
      return (Flow flow) =>
          flow.host.toLowerCase().contains(word) ||
          flow.path.toLowerCase().contains(word);
    }
    if (!fakeFilterKeys.contains(key)) {
      throw DaemonException(_refuse(_unknownKey(term, key)));
    }
    final (String? cmp, String atom) = _splitCmp(rest);
    final String value = _unquote(atom);
    if (value.isEmpty) {
      throw DaemonException(
        _refuse('the filter term "$term" has no value; write $key:<value>'),
      );
    }
    final String lower = value.toLowerCase();
    switch (key) {
      case 'host':
      case 'apex':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => _suffixMatch(flow.host, lower);
      case 'state':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => _snake(flow.state.name) == lower;
      case 'method':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => flow.methodLabel == value.toUpperCase();
      case 'decision':
        _rejectCmp(cmp, key, term);
        return (Flow flow) =>
            flow.decision != null && _snake(flow.decision!.name) == lower;
      case 'reason':
        _rejectCmp(cmp, key, term);
        return (Flow flow) =>
            flow.blockReason != null && _snake(flow.blockReason!.name) == lower;
      case 'rule':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => flow.ruleId?.value == value;
      case 'session':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => flow.sessionId.value == value;
      case 'path':
        _rejectCmp(cmp, key, term);
        return (Flow flow) => flow.path.toLowerCase().contains(lower);
      case 'upgrade':
        _rejectCmp(cmp, key, term);
        // No flow carries an upgrade in M1; the key parses so that the
        // grammar of the daemon and of the fake stay the same size.
        return (Flow flow) => lower == 'none';
      case 'status':
        final int wanted = _number(value, term);
        return (Flow flow) => _compareInt(flow.status, cmp, wanted);
      case 'findings':
        final int wanted = _number(value, term);
        return (Flow flow) => _compareInt(flow.findingCount, cmp, wanted);
      case 'since':
        _rejectCmp(cmp, key, term);
        final DateTime from = _timestamp(value, now, term);
        return (Flow flow) => !flow.receivedAt.isBefore(from);
      case 'until':
        _rejectCmp(cmp, key, term);
        final DateTime to = _timestamp(value, now, term);
        return (Flow flow) => !flow.receivedAt.isAfter(to);
      case 'edited':
        _rejectCmp(cmp, key, term);
        final bool wanted = _boolean(value, term);
        return (Flow flow) => flow.edited == wanted;
      case 'passthrough':
        _rejectCmp(cmp, key, term);
        final bool wanted = _boolean(value, term);
        return (Flow flow) => flow.passthrough == wanted;
      case 'meta':
        _rejectCmp(cmp, key, term);
        final bool wanted = _boolean(value, term);
        return (Flow flow) => flow.meta == wanted;
      default:
        throw DaemonException(_refuse(_unknownKey(term, key)));
    }
  }

  /// The order of `ListFlows.order_by`; an unknown key is `IPC_005`, never a
  /// silent fallback to another order.
  static (FakeSortKey, bool) order(String orderBy) {
    final List<String> words = orderBy.toLowerCase().split(RegExp(r'\s+'))
      ..removeWhere((String word) => word.isEmpty);
    final String key = words.isEmpty ? 'ts' : words.first;
    final FakeSortKey sort = switch (key) {
      'received_at' || 'ts' || 'time' => FakeSortKey.ts,
      'host' => FakeSortKey.host,
      'duration' => FakeSortKey.duration,
      'size' => FakeSortKey.size,
      _ => throw DaemonException(
        Diagnostic(
          code: 'IPC_005',
          severity: Severity.error,
          why:
              '"$key" is not a sort key; list_flows sorts by received_at, '
              'host, duration or size',
        ),
      ),
    };
    return (sort, !words.skip(1).contains('asc'));
  }

  static String _unknownKey(String term, String key) =>
      'the filter term "$term" uses the unknown key "$key"; valid keys are '
      '${fakeFilterKeys.join(', ')}';

  static Diagnostic _refuse(String why) =>
      Diagnostic(code: 'RECORDER_002', severity: Severity.error, why: why);

  static (String?, String) _splitKey(String term) {
    if (term.startsWith('"')) {
      return (null, term);
    }
    final int index = term.indexOf(':');
    if (index <= 0) {
      return (null, term);
    }
    final String key = term.substring(0, index);
    final bool wordy = key.runes.every(
      (int rune) =>
          (rune >= 0x41 && rune <= 0x5a) ||
          (rune >= 0x61 && rune <= 0x7a) ||
          rune == 0x5f,
    );
    if (!wordy) {
      return (null, term);
    }
    return (key.toLowerCase(), term.substring(index + 1));
  }

  static (String?, String) _splitCmp(String value) {
    for (final String text in <String>['>=', '<=', '>', '<']) {
      if (value.startsWith(text)) {
        return (text, value.substring(text.length));
      }
    }
    return (null, value);
  }

  static void _rejectCmp(String? cmp, String key, String term) {
    if (cmp != null) {
      throw DaemonException(
        _refuse(
          'the filter term "$term" compares with an operator, but $key: takes '
          'a plain value; only status: and findings: accept >, >=, < and <=',
        ),
      );
    }
  }

  static bool _compareInt(int actual, String? cmp, int wanted) => switch (cmp) {
    '>=' => actual >= wanted,
    '<=' => actual <= wanted,
    '>' => actual > wanted,
    '<' => actual < wanted,
    _ => actual == wanted,
  };

  static int _number(String value, String term) {
    final int? parsed = int.tryParse(value);
    if (parsed == null) {
      throw DaemonException(
        _refuse(
          'the filter term "$term" needs a whole number, "$value" is not one',
        ),
      );
    }
    return parsed;
  }

  static bool _boolean(String value, String term) =>
      switch (value.toLowerCase()) {
        'true' || 'yes' || '1' => true,
        'false' || 'no' || '0' => false,
        _ => throw DaemonException(
          _refuse(
            'the filter term "$term" needs true or false, "$value" is neither',
          ),
        ),
      };

  static DateTime _timestamp(String value, DateTime now, String term) {
    final Duration? relative = _relative(value);
    if (relative != null) {
      return now.subtract(relative);
    }
    final DateTime? parsed = DateTime.tryParse(value);
    if (parsed != null) {
      return parsed;
    }
    throw DaemonException(
      _refuse(
        'the filter term "$term" needs an ISO-8601 timestamp '
        '(2026-09-03T10:00:00Z) or a relative duration (30s, 10m, 2h, 1d, '
        '1w), "$value" is neither',
      ),
    );
  }

  static Duration? _relative(String value) {
    if (value.length < 2) {
      return null;
    }
    final String unit = value.substring(value.length - 1).toLowerCase();
    final int? amount = int.tryParse(value.substring(0, value.length - 1));
    if (amount == null) {
      return null;
    }
    return switch (unit) {
      's' => Duration(seconds: amount),
      'm' => Duration(minutes: amount),
      'h' => Duration(hours: amount),
      'd' => Duration(days: amount),
      'w' => Duration(days: amount * 7),
      _ => null,
    };
  }

  /// `host:github.com` matches `github.com` and `api.github.com`, never
  /// `notgithub.com`: the comparison runs on whole labels.
  static bool _suffixMatch(String host, String suffix) {
    final String lower = host.toLowerCase();
    return lower == suffix || lower.endsWith('.$suffix');
  }

  static String _unquote(String value) {
    final String trimmed = value.replaceAll('"', '');
    return trimmed;
  }
}

/// A Dart enum name as the daemon writes it: `allowEdited` is `allow_edited`.
String _snake(String name) => name.replaceAllMapped(
  RegExp('[A-Z]'),
  (Match match) => '_${match.group(0)!.toLowerCase()}',
);

/// Wann die aufgezeichnete History beginnt.
///
/// Ein fester Zeitpunkt, damit derselbe Aufruf zweimal dieselben Zeilen
/// ergibt: ein Golden und ein Paging-Test bedeuten sonst zweimal etwas
/// anderes.
final DateTime historyEpoch = DateTime.utc(2026, 9, 3, 9);

/// Der Abstand zweier aufgezeichneter Anfragen.
///
/// Eine halbe Minute, damit eine Sitzung über Stunden läuft: nur dann sagen
/// `since:10m` und `until:` etwas, und nur dann stehen in der Zeitspalte
/// Stunden, Minuten und Sekunden nebeneinander.
const Duration historyStep = Duration(seconds: 30);

/// Die Sitzung, zu der die aufgezeichneten Flows gehören.
///
/// Nicht [FakeDaemonClient.defaultSession]: die History ist das Archiv
/// vergangener Sitzungen, und eine aufgezeichnete Zeile, die sich als die
/// laufende Sitzung ausgibt, wäre eine Unwahrheit über ihre Herkunft
/// (`backlog/CONVENTIONS.md` 4.13). Der Filter `session:` unterscheidet die
/// beiden damit auch im Fake.
const SessionId historySession = SessionId(
  '018f0004-0000-7000-8000-000000000001',
);

/// Die Regel, die in dieser Aufzeichnung entschieden hat.
///
/// Eine eigene, nicht [FakeDaemonClient.bundledBlockRule]: die gebündelte
/// Regel blockiert, und sie als Quelle einer Freigabe auszugeben behauptete
/// etwas, das ihr Regelsatz nicht hergibt.
const RuleId historyRule = RuleId('018f0005-0000-7000-8000-0000000000b7');

/// Die Ziele, die der Recorder gesehen hat, in fester Reihenfolge.
///
/// Acht, und alle sechs Methoden, die ein Badge hat: die Methodenspalte wird
/// sonst nie breit genug getestet.
const List<(Method, String, String)> _historyTargets =
    <(Method, String, String)>[
      (Method.get, 'registry.npmjs.org', '/react/-/react-19.2.0.tgz'),
      (Method.post, 'api.github.com', '/graphql'),
      (Method.get, 'pypi.org', '/simple/requests/'),
      (Method.put, 'storage.googleapis.com', '/bucket/object.json'),
      (Method.patch, 'gitlab.com', '/api/v4/projects/9/merge_requests/3'),
      (Method.delete, 'api.example.org', '/v1/items/42'),
      (Method.head, 'example.org', '/'),
      (Method.post, 'telemetry.vendor.io', '/v2/collect'),
    ];

/// Legt [count] aufgezeichnete Flows in [state], ab [start], einen alle
/// [historyStep].
///
/// Deterministisch: derselbe Aufruf ergibt dieselben Zeilen, sonst bedeuten
/// ein Golden und ein Paging-Test zweimal etwas anderes. Zwölf Zeilen decken
/// alle acht Sichtzustände ab und geben dabei die Mischung wieder, die eine
/// echte Sitzung hat — die meisten Anfragen gehen durch, wenige werden
/// blockiert, eine wartet noch.
void seedHistory(
  FakeSessionState state, {
  required int count,
  required DateTime start,
}) {
  for (int i = 0; i < count; i++) {
    final _SeededFlow seeded = _SeededFlow(index: i, start: start);
    state.flows[seeded.id] = seeded.summary;
    state.details[seeded.id] = seeded.detail;
    if (seeded.bodyBytes.isNotEmpty) {
      state.bodies[_hexOfBytes(seeded.bodyRef.sha256)] = seeded.bodyBytes;
    }
    if (seeded.responseBytes.isNotEmpty) {
      state.bodies[_hexOfBytes(seeded.responseRef.sha256)] =
          seeded.responseBytes;
    }
    if (seeded.summary.edited) {
      state.bodies[_hexOfBytes(seeded.editedRef.sha256)] = seeded.editedBytes;
    }
  }
}

/// Eine aufgezeichnete Zeile, aus ihrem Index abgeleitet.
class _SeededFlow {
  _SeededFlow({required this.index, required DateTime start})
    : receivedAt = start.add(historyStep * index);

  final int index;
  final DateTime receivedAt;

  (Method, String, String) get _target =>
      _historyTargets[index % _historyTargets.length];

  Method get method => passthrough ? Method.post : _target.$1;
  String get host => _target.$2;
  String get path => _target.$3;

  /// Welche der zwölf Zeilenformen diese ist.
  ///
  /// Zwölf und nicht acht: acht Formen ergäben genau eine Freigabe je acht
  /// Zeilen, und eine Liste, in der jeder achte Flow blockiert wurde, sieht
  /// aus wie ein Angriff und nicht wie ein Arbeitstag.
  int get _shape => index % 12;

  bool get passthrough => _shape == 10;

  FlowId get id => FlowId(
    '018f0004-0000-7000-8000-${(index + 1).toRadixString(16).padLeft(12, '0')}',
  );

  bool get _carriesBody =>
      method == Method.post || method == Method.put || method == Method.patch;

  Uint8List get bodyBytes => _carriesBody
      ? Uint8List.fromList(
          utf8.encode('{"flow": "${id.value}", "path": "$_pathAndQuery"}'),
        )
      : Uint8List(0);

  /// Der Pfad mit einer Abfrage, die sich je Zeile ändert.
  ///
  /// Die Pfadspalte kürzt in der Mitte; ohne wechselnde Länge prüft das
  /// niemand.
  String get _pathAndQuery => passthrough ? '/api/chat' : '$path?page=$index';

  BodyRef get bodyRef => BodyRef(
    sha256: _digest('request'),
    size: bodyBytes.length,
    contentType: bodyBytes.isEmpty ? '' : 'application/json',
  );

  /// Ein Schlüssel, der zu genau einem Rumpf gehört.
  ///
  /// Aus der Flow-Id und der Rolle, nicht aus dem Index: ein einzelnes Byte
  /// hat 256 Werte, und ab der 257. Zeile lieferte `GetBody` die Bytes eines
  /// fremden Flows, während die Größe daneben etwas anderes behauptete. Die
  /// Aufzeichnung von zehntausend Zeilen ist genau der Fall, für den dieses
  /// Szenario da ist.
  ///
  /// Jedes Ausgabebyte faltet die **ganze** Zeichenkette (`FNV-1a`, je Byte
  /// mit anderem Startwert). Zwei Flow-Ids unterscheiden sich nur am Ende;
  /// ein Verfahren, das die ersten zweiunddreißig Zeichen abtastet, gäbe
  /// allen denselben Schlüssel.
  List<int> _digest(String role) {
    final List<int> seed = utf8.encode('${id.value}#$role');
    return List<int>.generate(32, (int i) {
      int hash = (0x811c9dc5 ^ (i * 0x01000193)) & 0xffffffff;
      for (final int byte in seed) {
        hash = ((hash ^ byte) * 0x01000193) & 0xffffffff;
      }
      return (hash >> 8) & 0xff;
    });
  }

  /// Der Status, den diese Zeilenform trägt.
  ///
  /// Steht vor [summary], damit [responseBytes] und [_responseSize] auf
  /// dieselbe Zahl zeigen: eine Zeile, die 900 Byte meldet und einen leeren
  /// Rumpf mitgibt, widerspricht sich selbst (`backlog/CONVENTIONS.md` 4.13).
  int get _status => switch (_shape) {
    0 || 11 => 0,
    4 => 204,
    6 || 7 => 403,
    9 => 504,
    _ => 200,
  };

  /// Der Rumpf der Antwort, wo eine ankam.
  ///
  /// Auch eine Ablehnung hat einen: die 403 schreibt der Proxy selbst, und
  /// der Export zeigt sie als `content.text` (`backlog/sprint-2.md`,
  /// HAR-Mapping). Die Füllung wächst mit dem Index, damit die Größenspalte
  /// und die Sortierung nach Größe etwas zu unterscheiden haben.
  Uint8List get responseBytes {
    if (_status == 0 || _status == 204) {
      return Uint8List(0);
    }
    final String head = switch (_status) {
      403 => '{"error": "blocked", "flow": "${id.value}"',
      504 => '{"error": "timeout", "flow": "${id.value}"',
      _ => '{"ok": true, "flow": "${id.value}"',
    };
    final String pad = List<String>.filled(index % 31, 'x').join();
    return Uint8List.fromList(utf8.encode('$head, "pad": "$pad"}'));
  }

  BodyRef get responseRef => BodyRef(
    sha256: _digest('response'),
    size: responseBytes.length,
    contentType: responseBytes.isEmpty ? '' : 'application/json',
  );

  /// Die Anfrage, wie der Mensch sie vor dem Senden geändert hat.
  ///
  /// Nur bei der bearbeiteten Form. Eine Zeile mit `edited: true` und einem
  /// leeren „Edited"-Tab behauptete eine Bearbeitung, die es nicht gab
  /// (`backlog/CONVENTIONS.md` 4.13).
  Uint8List get editedBytes => Uint8List.fromList(
    utf8.encode('{"flow": "${id.value}", "note": "redacted by hand"}'),
  );

  BodyRef get editedRef => BodyRef(
    sha256: _digest('request_edited'),
    size: editedBytes.length,
    contentType: 'application/json',
  );

  /// Die Funde, die [Flow.findingCount] zählt.
  ///
  /// Genau so viele, wie die Zeile behauptet: eine Zahl ohne Fundliste
  /// dahinter ist der Widerspruch, den 4.13 verbietet, und im JSONL-Export
  /// stünde `findings_count: 2` neben `findings: []`.
  List<Finding> get findings => _findingCount == 0
      ? const <Finding>[]
      : <Finding>[
          const Finding(
            kind: 'api_key.github',
            location: FindingLocation.header,
            headerName: 'authorization',
            spanStart: 7,
            spanEnd: 47,
            tier: FindingTier.checksum,
            displayPrefix: 'ghp_R8kQ',
          ),
          const Finding(
            kind: 'email',
            location: FindingLocation.body,
            spanStart: 12,
            spanEnd: 38,
            tier: FindingTier.regex,
            displayPrefix: 'nils.hof',
          ),
        ].take(_findingCount).toList(growable: false);

  /// Wie viele Funde diese Zeile trägt.
  int get _findingCount => index % 7 == 0 ? 2 : 0;

  /// Größen und Dauern bleiben beschränkt.
  ///
  /// Eine Dauer, die mit dem Index wächst, wäre bei zehntausend Zeilen eine
  /// Anfrage von einer Stunde, und die Sortierung nach Dauer wäre dieselbe
  /// wie die nach Zeit — ein Test, der beide unterscheiden soll, unterschiede
  /// dann nichts.
  int get _requestSize => 180 + (index % 17) * 64;

  /// Genau die Länge dessen, was aufgezeichnet wurde.
  int get _responseSize => responseBytes.length;
  Duration get _duration => Duration(milliseconds: 40 + (index % 23) * 17);

  Flow get summary {
    final Flow base = Flow(
      id: id,
      sessionId: historySession,
      receivedAt: receivedAt,
      method: method,
      scheme: passthrough ? Scheme.http : Scheme.https,
      authority: passthrough
          ? const Authority(
              host: '192.168.1.50',
              port: 11434,
              isIpLiteral: true,
            )
          : Authority(host: host, port: 443),
      path: _pathAndQuery,
      state: FlowState.recorded,
      status: _status,
      requestSize: _requestSize,
      responseSize: _responseSize,
      duration: _duration,
      // Findings folgen nicht der Zeilenform, damit ein Filter
      // `findings:>0` quer durch die Zustände schneidet und nicht einen
      // einzigen von ihnen auswählt.
      findingCount: _findingCount,
      originTool: 'opencode',
      decidedAt: receivedAt.add(const Duration(seconds: 2)),
    );
    return switch (_shape) {
      // Wartet noch: keine Entscheidung, kein Status, eine Frist.
      0 => base.copyWith(
        state: FlowState.held,
        duration: null,
        deadline: receivedAt.add(const Duration(minutes: 5)),
        heldAt: receivedAt,
        decidedAt: null,
      ),
      1 || 2 || 3 => base.copyWith(
        decision: DecisionKind.allow,
        decisionSource: DecisionSource.user,
      ),
      // Freigegeben, aber ohne Inhalt in der Antwort.
      4 => base.copyWith(
        decision: DecisionKind.allow,
        decisionSource: DecisionSource.user,
      ),
      5 => base.copyWith(
        decision: DecisionKind.allowEdited,
        decisionSource: DecisionSource.user,
        edited: true,
      ),
      6 => base.copyWith(
        decision: DecisionKind.block,
        decisionSource: DecisionSource.user,
        blockReason: BlockReason.user,
      ),
      // Von einer Regel blockiert, der häufigste automatische Fall.
      7 => base.copyWith(
        decision: DecisionKind.block,
        decisionSource: DecisionSource.rule,
        blockReason: BlockReason.rule,
        ruleId: historyRule,
      ),
      8 => base.copyWith(
        decision: DecisionKind.allow,
        decisionSource: DecisionSource.rule,
        ruleId: historyRule,
      ),
      9 => base.copyWith(
        decision: DecisionKind.timedOut,
        decisionSource: DecisionSource.timeout,
        blockReason: BlockReason.timeout,
      ),
      10 => base.copyWith(
        decision: DecisionKind.allow,
        decisionSource: DecisionSource.passthrough,
        passthrough: true,
      ),
      _ => base.copyWith(
        state: FlowState.failed,
        decision: DecisionKind.allow,
        decisionSource: DecisionSource.user,
        upstreamError: UpstreamError.connect,
      ),
    };
  }

  FlowDetail get detail {
    final Flow flow = summary;
    return FlowDetail(
      summary: flow,
      findings: findings,
      editedRequest: flow.edited
          ? HttpRequest(
              method: method,
              scheme: flow.scheme,
              authority: flow.authority,
              pathAndQuery: flow.path,
              headers: <Header>[
                Header(name: 'accept', value: utf8.encode('application/json')),
                Header(
                  name: 'content-type',
                  value: utf8.encode('application/json'),
                ),
                Header(name: 'host', value: utf8.encode(flow.host)),
              ],
              body: editedRef,
              version: 'HTTP/1.1',
            )
          : null,
      responseBody: responseBytes.isEmpty ? null : responseRef,
      request: HttpRequest(
        method: method,
        scheme: flow.scheme,
        authority: flow.authority,
        pathAndQuery: flow.path,
        headers: <Header>[
          Header(name: 'accept', value: utf8.encode('application/json')),
          if (_carriesBody)
            Header(
              name: 'content-type',
              value: utf8.encode('application/json'),
            ),
          Header(name: 'host', value: utf8.encode(flow.host)),
          Header(name: 'user-agent', value: utf8.encode('opencode/0.4.2')),
        ],
        body: bodyRef,
        version: 'HTTP/1.1',
      ),
      response: flow.status == 0
          ? null
          : HttpResponseHead(
              status: flow.status,
              headers: <Header>[
                Header(
                  name: 'content-type',
                  value: utf8.encode('application/json'),
                ),
                Header(
                  name: 'content-length',
                  value: utf8.encode('${flow.responseSize}'),
                ),
              ],
              version: 'HTTP/1.1',
            ),
      bodyPreview: utf8.decode(bodyBytes, allowMalformed: true),
    );
  }
}

String _hexOfBytes(List<int> bytes) =>
    bytes.map((int b) => b.toRadixString(16).padLeft(2, '0')).join();

// --- Die gespielte Sandbox (HUM-040) ----------------------------------------
//
// Dieselbe Momentaufnahme, die der Rust-Fake liefert
// (`daemon/crates/ipc/src/fake/mod.rs`). Beide Seiten muessen dasselbe zeigen,
// sonst sieht der Bildschirm unter `--fake-client` anders aus als unter
// `humanitld --fake` (CONVENTIONS 4.7).

/// Der Zustand, mit dem der Fake beginnt: nichts laeuft, und der Bildschirm
/// zeigt, was ein Start taete.
SandboxStatus defaultSandbox_() => SandboxStatus(
  profile: 'default',
  sessionId: FakeDaemonClient.defaultSession,
  workDirHost: FakeDaemonClient.defaultWorkDir,
  llmEndpoint: 'http://192.168.1.50:11434',
  mounts: fakeSandboxMounts(
    workDir: FakeDaemonClient.defaultWorkDir,
    workMode: WorkMode.rw,
  ),
  env: fakeSandboxEnv(),
  argvPreview: fakeSandboxArgv(
    workDir: FakeDaemonClient.defaultWorkDir,
    workMode: WorkMode.rw,
  ),
);

/// Was der Daemon meldet, wenn der Shim keinen Bericht geliefert hat.
///
/// Drei rote Ergebnisse, jedes mit `SANDBOX_013` und derselben Evidenz --
/// genau die Form aus `BwrapBackend::isolation_check`. Der Daemon schickt
/// niemals gar nichts: „kein Bericht" ist selbst ein Ergebnis, und es ist rot.
List<IsolationCheckResult> fakeIsolationNoReport() => <IsolationCheckResult>[
  for (final IsolationCheck check in IsolationCheck.values)
    IsolationCheckResult(
      check: check,
      passed: false,
      evidence: _fakeNoReportEvidence,
      diagnostic: Diagnostic(
        code: DiagnosticCodes.isolationNoReport,
        severity: Severity.blocking,
        title: 'Isolation check without a report',
        why: '${check.name}: $_fakeNoReportEvidence',
      ),
    ),
];

/// Dieselbe Evidenz, die `bwrap.rs` bei ausgebliebenem Bericht schreibt.
const String _fakeNoReportEvidence =
    'no CHECK line from the shim within 5s; the report pipe is closed';

/// Der registrierte Code einer Garantie, als Boden fuer ein Ergebnis ohne
/// eigenen Befund. Die Zuordnung steht im Daemon (`codes.rs`, `bwrap.rs`).
String _isolationCodeOf(IsolationCheck check) => switch (check) {
  IsolationCheck.noNetworkInterface =>
    DiagnosticCodes.isolationNoNetworkInterface,
  IsolationCheck.singleSocket => DiagnosticCodes.isolationSingleSocket,
  IsolationCheck.seccompActive => DiagnosticCodes.isolationSeccompActive,
};

/// Die drei Garantien, wie der Rust-Fake sie meldet
/// (`daemon/crates/ipc/src/fake/mod.rs`, `isolation_checks`).
///
/// Gruen, und jede Evidenz sagt ausdruecklich, dass nichts gemessen wurde und
/// welcher Befehl es messen wuerde. Ein Fake, der einen Beleg vortaeuschte,
/// waere genau die Luege, die dieser Bildschirm nicht haben darf.
List<IsolationCheckResult> fakeIsolationChecks() => <IsolationCheckResult>[
  const IsolationCheckResult(
    check: IsolationCheck.noNetworkInterface,
    passed: true,
    evidence: '$_fakeNothingMeasured (would run: ip link)',
  ),
  const IsolationCheckResult(
    check: IsolationCheck.singleSocket,
    passed: true,
    evidence: '$_fakeNothingMeasured (would run: ss -x)',
  ),
  const IsolationCheckResult(
    check: IsolationCheck.seccompActive,
    passed: true,
    evidence:
        '$_fakeNothingMeasured (would run: grep Seccomp /proc/<agent>/status)',
  ),
];

/// Derselbe Satz wie `NOTHING_MEASURED` im Rust-Fake.
const String _fakeNothingMeasured = 'fake daemon: nothing was measured';

/// Jeder Pfad, den der Agent in der gespielten Sandbox saehe.
List<MountEntry> fakeSandboxMounts({
  required String workDir,
  required WorkMode workMode,
}) => <MountEntry>[
  const MountEntry(dst: '/usr', src: '/usr', mode: MountMode.ro),
  const MountEntry(dst: '/etc/ssl', src: '/etc/ssl', mode: MountMode.ro),
  const MountEntry(
    dst: '/etc/alternatives',
    src: '/etc/alternatives',
    mode: MountMode.ro,
  ),
  const MountEntry(dst: '/bin', mode: MountMode.symlink, linkTarget: 'usr/bin'),
  const MountEntry(dst: '/proc', mode: MountMode.proc),
  const MountEntry(dst: '/dev', mode: MountMode.dev),
  const MountEntry(dst: '/tmp', mode: MountMode.tmpfs),
  const MountEntry(dst: '/dev/shm', mode: MountMode.tmpfs),
  MountEntry(
    dst: '/work',
    src: workDir,
    mode: workMode == WorkMode.ro ? MountMode.ro : MountMode.rw,
    origin: ValueOrigin.session,
  ),
  const MountEntry(dst: '/work/.git/hooks', mode: MountMode.tmpfs),
  const MountEntry(dst: '/work/.envrc', mode: MountMode.masked),
  const MountEntry(dst: '/work/.git/config', mode: MountMode.masked),
  const MountEntry(
    dst: '/run/humanitl/proxy.sock',
    src: r'$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock',
    mode: MountMode.ro,
    origin: ValueOrigin.session,
  ),
  const MountEntry(
    dst: '/etc/humanitl/ca.crt',
    src: r'$XDG_DATA_HOME/humanitl/ca/ca.crt',
    mode: MountMode.ro,
    origin: ValueOrigin.session,
  ),
  const MountEntry(
    dst: '/run/humanitl/humanitl-shim',
    src: '/usr/lib/humanitl/humanitl-shim',
    mode: MountMode.ro,
    origin: ValueOrigin.session,
  ),
  const MountEntry(
    dst: '/etc/humanitl/AGENTS.md',
    mode: MountMode.masked,
    origin: ValueOrigin.adapter,
  ),
];

/// Die Umgebung der gespielten Sandbox, alphabetisch.
///
/// Zwei Werte sind zurueckgehalten, und beide heissen mit Absicht so, dass
/// keine Regel ueber verdaechtige Endungen sie faende: `AWS_ACCESS_KEY_ID`
/// endet auf `_ID`, `DATABASE_URL` traegt das Passwort in der URL. So zeigt
/// auch der Fake, dass die Vorgabe „zurueckgehalten" lautet und nicht
/// „sichtbar, ausser der Name klingt verdaechtig" (CONVENTIONS 4.17).
List<EnvEntry> fakeSandboxEnv() => <EnvEntry>[
  const EnvEntry(
    key: 'AWS_ACCESS_KEY_ID',
    origin: ValueOrigin.user,
    withheld: true,
  ),
  const EnvEntry(key: 'DATABASE_URL', origin: ValueOrigin.user, withheld: true),
  const EnvEntry(key: 'HOME', value: '/home/agent'),
  const EnvEntry(key: 'HTTPS_PROXY', value: 'http://127.0.0.1:3128'),
  const EnvEntry(key: 'HTTP_PROXY', value: 'http://127.0.0.1:3128'),
  const EnvEntry(
    key: 'HUMANITL_SESSION',
    value: '018f0001-0000-7000-8000-000000000001',
    origin: ValueOrigin.session,
  ),
  const EnvEntry(key: 'NO_PROXY'),
  const EnvEntry(
    key: 'OPENCODE_CONFIG',
    value: '/etc/humanitl/opencode.json',
    origin: ValueOrigin.adapter,
  ),
  const EnvEntry(key: 'PATH', value: '/usr/bin:/bin'),
  const EnvEntry(key: 'SSL_CERT_FILE', value: '/etc/humanitl/ca.crt'),
  const EnvEntry(key: 'TERM', value: 'xterm-256color'),
  const EnvEntry(key: 'USER', value: 'agent'),
];

/// Die Kommandozeile der gespielten Sandbox als eine Zeile.
String fakeSandboxArgv({required String workDir, required WorkMode workMode}) {
  final String work = workMode == WorkMode.ro ? '--ro-bind' : '--bind';
  return 'bwrap --unshare-user --unshare-ipc --unshare-pid --unshare-net '
      '--unshare-uts --unshare-cgroup --die-with-parent --new-session '
      '--cap-drop ALL --disable-userns --hostname sandbox '
      '--ro-bind /usr /usr --ro-bind /etc/ssl /etc/ssl '
      '--ro-bind /etc/alternatives /etc/alternatives --proc /proc --dev /dev '
      '--tmpfs /tmp --tmpfs /dev/shm $work $workDir /work '
      '--tmpfs /work/.git/hooks '
      r'--ro-bind $XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock '
      '/run/humanitl/proxy.sock '
      r'--ro-bind $XDG_DATA_HOME/humanitl/ca/ca.crt /etc/humanitl/ca.crt '
      '--ro-bind /usr/lib/humanitl/humanitl-shim /run/humanitl/humanitl-shim '
      '--ro-bind-data 15 /work/.envrc --ro-bind-data 16 /work/.git/config '
      '--ro-bind-data 40 /etc/humanitl/AGENTS.md --clearenv '
      '--setenv HTTP_PROXY http://127.0.0.1:3128 '
      '--setenv HTTPS_PROXY http://127.0.0.1:3128 '
      // Wie beim echten Daemon: ein zurueckgehaltener Wert steht auch in der
      // Zeile nicht, die man kopieren kann (CONVENTIONS 4.17).
      "--setenv AWS_ACCESS_KEY_ID '<withheld>' "
      "--setenv DATABASE_URL '<withheld>' "
      '--setenv SSL_CERT_FILE /etc/humanitl/ca.crt --chdir /work '
      '-- /run/humanitl/humanitl-shim --proxy-port 3128 -- opencode';
}
