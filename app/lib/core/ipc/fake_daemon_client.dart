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

  /// The fake for a `HUMANITL_FAKE=<name>` scenario. Unknown names replay
  /// the default script, so a typo still shows a working shell.
  factory FakeDaemonClient.scenario(String name) {
    final String key = name.trim().toLowerCase();
    if (key.startsWith('burst')) {
      final int? count = int.tryParse(key.split(':').skip(1).join());
      return FakeDaemonClient.burst(count: count ?? 30);
    }
    return switch (key) {
      'unavailable' || 'missing' || 'down' => FakeDaemonClient.unavailable(),
      'mismatch' || 'incompatible' => FakeDaemonClient.incompatible(),
      'empty' => FakeDaemonClient.empty(),
      _ => FakeDaemonClient(),
    };
  }

  /// The socket the unavailable scenario pretends to have tried.
  static const String defaultSocket = '/run/user/1000/humanitl/daemon.sock';

  /// The session every scripted flow belongs to.
  static const SessionId defaultSession = SessionId(
    '018f0001-0000-7000-8000-000000000001',
  );

  /// The bundled rule that blocks `models.dev` (mirror of
  /// `BUNDLED_BLOCK_RULE` in the Rust fake).
  static const RuleId bundledBlockRule = RuleId(
    '018f0000-0000-7000-8000-0000000000a1',
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
  Future<void> decide(FlowId id, Decision decision, {Rule? remember}) async {
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
  }

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    _check();
    final List<Flow> all = state.flows.values.toList()
      ..sort((Flow a, Flow b) => b.receivedAt.compareTo(a.receivedAt));
    final List<Flow> selected = all
        .where((Flow flow) => filter.includePassthrough || !flow.passthrough)
        .take(limit)
        .toList();
    return FlowPage(flows: selected, total: all.length);
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
