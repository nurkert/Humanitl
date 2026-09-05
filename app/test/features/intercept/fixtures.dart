// Bausteine der Intercept-Tests: Flows, Details, Ereignisse und ein
// DaemonClient, dessen Stream der Test selbst füttert.

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';

/// Die Session, zu der jeder Test-Flow gehört.
const SessionId testSession = SessionId('018f0001-0000-7000-8000-000000000001');

/// Der Startzeitpunkt aller Tests; feste Uhr statt `DateTime.now()`.
final DateTime testStart = DateTime.utc(2026, 9, 3, 12);

/// Die Id des n-ten Test-Flows.
FlowId testFlowId(int n) =>
    FlowId('018f0020-0000-7000-8000-${n.toString().padLeft(12, '0')}');

/// Ein angehaltener Flow mit Frist [deadline].
Flow heldFlow({
  required int n,
  required DateTime deadline,
  DateTime? receivedAt,
  Method method = Method.get,
  String host = 'api.github.com',
  String path = '/graphql',
  int requestSize = 0,
}) {
  final DateTime at = receivedAt ?? testStart;
  return Flow(
    id: testFlowId(n),
    sessionId: testSession,
    receivedAt: at,
    method: method,
    scheme: Scheme.https,
    authority: Authority(host: host, port: 443),
    path: path,
    state: FlowState.held,
    requestSize: requestSize,
    deadline: deadline,
    heldAt: at,
  );
}

/// Ein Fund, wie ihn der Daemon meldet: `kind` trägt Art und Parameter.
Finding testFinding({
  String kind = 'api_key:github',
  FindingLocation location = FindingLocation.body,
  String headerName = '',
  bool resolved = false,
}) => Finding(
  kind: kind,
  location: location,
  headerName: headerName,
  spanStart: 0,
  spanEnd: 8,
  tier: FindingTier.checksum,
  resolved: resolved,
);

/// Das Detail zu [flow], mit [headers] und [bodyPreview].
///
/// [apex] ist die registrierbare Domain, die der Katalog des Daemons kennt;
/// die Oberfläche rät sie nie (CONVENTIONS 4.13). [findings] sind die Funde,
/// die der Detektor gemeldet hat.
FlowDetail detailFor(
  Flow flow, {
  List<Header> headers = const <Header>[],
  String bodyPreview = '',
  String contentType = '',
  String apex = '',
  List<Finding> findings = const <Finding>[],
}) => FlowDetail(
  summary: flow,
  findings: findings,
  domain: apex.isEmpty ? null : DomainInfo(apex: apex),
  request: HttpRequest(
    method: flow.method,
    scheme: flow.scheme,
    authority: flow.authority,
    pathAndQuery: flow.path,
    headers: headers,
    body: BodyRef(
      sha256: List<int>.filled(32, 7),
      size: utf8.encode(bodyPreview).length,
      contentType: contentType,
    ),
    version: 'HTTP/1.1',
  ),
  bodyPreview: bodyPreview,
);

/// Ein Header mit Klartextwert.
Header header(String name, String value) =>
    Header(name: name, value: value.codeUnits);

/// Ein [DaemonClient], dessen Ereignisstrom der Test steuert.
///
/// Jeder `subscribe`-Aufruf legt einen neuen Controller an; [emit] schickt in
/// den jüngsten, [breakStream] lässt ihn scheitern. So sind Reconnect und
/// Resync ohne echte Zeit prüfbar.
class TestDaemonClient implements DaemonClient {
  /// Jeder bisher ausgegebene Strom, ältester zuerst.
  final List<StreamController<FlowEvent>> streams =
      <StreamController<FlowEvent>>[];

  /// Die Details, die `getFlow` beantwortet.
  final Map<FlowId, FlowDetail> details = <FlowId, FlowDetail>{};

  /// Jede angenommene Entscheidung, älteste zuerst.
  final List<(FlowId, Decision)> decisions = <(FlowId, Decision)>[];

  /// Jede Regel, die `decide` anlegen sollte, älteste zuerst.
  final List<Rule> remembered = <Rule>[];

  /// Jede Regel-Id, die `removeRule` löschen sollte.
  final List<RuleId> removed = <RuleId>[];

  /// Wie oft `listFlows` aufgerufen wurde.
  int listFlowsCalls = 0;

  /// Was `listFlows` beantwortet.
  FlowPage page = const FlowPage();

  /// Was `decide` wirft, wenn gesetzt.
  Diagnostic? decideFailure;

  /// Der jüngste Strom.
  StreamController<FlowEvent> get current => streams.last;

  /// Schickt [event] in den jüngsten Strom.
  void emit(FlowEvent event) => current.add(event);

  /// Lässt den jüngsten Strom scheitern, wie ein beendeter Daemon.
  void breakStream() =>
      current.addError(StateError('daemon gone'), StackTrace.empty);

  @override
  Future<DaemonInfo> getInfo() async => const DaemonInfo(
    daemonVersion: '0.0.0-test',
    protoMajor: 1,
    protoMinor: 0,
    sessionId: '018f0001-0000-7000-8000-000000000001',
  );

  @override
  Stream<FlowEvent> subscribe({
    FlowId? since,
    bool includePassthrough = false,
  }) {
    final StreamController<FlowEvent> controller =
        StreamController<FlowEvent>();
    streams.add(controller);
    return controller.stream;
  }

  @override
  Future<Rule?> decide(FlowId id, Decision decision, {Rule? remember}) async {
    final Diagnostic? failure = decideFailure;
    if (failure != null) {
      throw DaemonException(failure);
    }
    decisions.add((id, decision));
    if (remember == null) {
      return null;
    }
    final Rule created = remember.copyWith(
      id: RuleId(
        '018f0030-0000-7000-8000-${remembered.length.toString().padLeft(12, '0')}',
      ),
    );
    remembered.add(created);
    return created;
  }

  @override
  Future<void> removeRule(RuleId id) async {
    removed.add(id);
  }

  @override
  Future<FlowPage> listFlows(
    FlowFilter filter, {
    int limit = 200,
    String? cursor,
  }) async {
    listFlowsCalls++;
    return page;
  }

  @override
  Future<FlowDetail> getFlow(FlowId id) async {
    final FlowDetail? detail = details[id];
    if (detail == null) {
      throw DaemonException(
        const Diagnostic(
          code: DiagnosticCodes.flowNotHeld,
          severity: Severity.warning,
          why: 'unknown flow',
        ),
      );
    }
    return detail;
  }

  @override
  Stream<Uint8List> getBody(BodyRef ref) {
    // Der Doppelgänger liefert den Rumpf, den `detailFor` angekündigt hat.
    // Ohne ihn kämen null Bytes zu einem Verweis, der mehr nennt, und jede
    // Rumpf-Ansicht sagte zu Recht "es kam weniger an als angekündigt" -- eine
    // Aussage über den Test, nicht über das Programm (HUM-030).
    for (final FlowDetail detail in details.values) {
      if (detail.request?.body == ref && detail.bodyPreview.isNotEmpty) {
        return Stream<Uint8List>.value(
          Uint8List.fromList(utf8.encode(detail.bodyPreview)),
        );
      }
    }
    return const Stream<Uint8List>.empty();
  }

  // Der Regel-Teil des Ports (HUM-033). Diese Tests fahren ihn nicht; die
  // Antworten sind der leere Regelsatz, damit nichts behauptet wird, was
  // dieser Doppelgänger nicht weiß.

  @override
  Future<RuleSet> listRules() async => RuleSet.empty;

  @override
  Future<RuleSet> addRule(Rule rule) async {
    remembered.add(rule);
    return RuleSet(rules: <Rule>[rule]);
  }

  @override
  Future<RuleSet> updateRule(Rule rule) async => RuleSet(rules: <Rule>[rule]);

  @override
  Future<RuleSet> reorderRules(List<RuleId> order) async => RuleSet.empty;

  @override
  Future<RuleSet> makeRulePermanent(RuleId id) async => RuleSet.empty;

  @override
  Future<RuleSet> reloadRules() async => RuleSet.empty;

  @override
  Future<DryRun> dryRunRule(Rule rule, {int limit = dryRunScanDefault}) async =>
      DryRun.empty;

  // Der Schalter mitgelieferter Regeln (HUM-105) gehört dem Rules-Screen; der
  // Intercept-Screen fährt ihn nicht.
  @override
  Future<RuleSet> setRuleDisabled(RuleId id, {required bool disabled}) async =>
      RuleSet.empty;

  // Der Sandbox-Teil des Ports (HUM-040). Diese Tests fahren ihn nicht; der
  // Sandbox-Bildschirm hat sein eigenes Gerüst mit dem FakeDaemonClient.
  @override
  Stream<SandboxUpdate> sandboxStatus() => const Stream<SandboxUpdate>.empty();

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) => const Stream<SandboxUpdate>.empty();

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) => const Stream<SandboxUpdate>.empty();

  @override
  Stream<SandboxUpdate> stopSandbox() => const Stream<SandboxUpdate>.empty();

  @override
  Stream<SandboxUpdate> checkIsolation() => const Stream<SandboxUpdate>.empty();

  @override
  Stream<TerminalFrame> terminal(Stream<TerminalCommand> input) =>
      const Stream<TerminalFrame>.empty();

  @override
  Future<void> close() async {
    for (final StreamController<FlowEvent> controller in streams) {
      if (!controller.isClosed) {
        await controller.close();
      }
    }
  }
}

/// Eine stehende Uhr: kein Timer, dafür ein Zeitpunkt, den der Test setzt.
class FixedNow extends Now {
  /// Startet bei [_at].
  FixedNow(this._at);

  DateTime _at;

  @override
  DateTime build() => _at;

  /// Setzt die Uhr auf [at].
  void moveTo(DateTime at) {
    _at = at;
    state = at;
  }
}

/// Eine Queue mit festem Inhalt, ohne Ereignisstrom.
class FixedFlows extends Flows {
  /// Zeigt [_flows].
  FixedFlows(this._flows);

  final Map<FlowId, Flow> _flows;

  @override
  Map<FlowId, Flow> build() => _flows;
}

/// Wartet, bis Mikrotasks und Stromereignisse durch sind.
Future<void> settle() async {
  for (int i = 0; i < 8; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

/// Ein Skript für den [FakeDaemonClient]: jedes Detail wird empfangen und
/// sofort angehalten, im Abstand [spacing], mit dem Budget [budget].
List<ScriptedEvent> holdScript(
  List<FlowDetail> details, {
  Duration budget = const Duration(minutes: 5),
  Duration spacing = const Duration(milliseconds: 10),
}) {
  final List<ScriptedEvent> script = <ScriptedEvent>[];
  for (int i = 0; i < details.length; i++) {
    final FlowDetail detail = details[i];
    final Duration at = spacing * i;
    script
      ..add(
        ScriptedEvent(at, (FakeSessionState state, DateTime now) {
          state.details[detail.summary.id] = detail;
          return FlowEvent.received(
            at: now,
            flow: detail.summary.copyWith(
              state: FlowState.received,
              deadline: null,
              heldAt: null,
              receivedAt: now,
            ),
          );
        }),
      )
      ..add(
        ScriptedEvent(
          at + const Duration(milliseconds: 1),
          (FakeSessionState state, DateTime now) => FlowEvent.held(
            at: now,
            flowId: detail.summary.id,
            deadline: now.add(budget),
          ),
        ),
      );
  }
  return script;
}

/// Hängt an [script] einen Ablauf für den Flow mit [id] nach [after].
ScriptedEvent timeoutAfter(FlowId id, Duration after) => ScriptedEvent(
  after,
  (FakeSessionState state, DateTime now) =>
      FlowEvent.timedOut(at: now, flowId: id),
);
