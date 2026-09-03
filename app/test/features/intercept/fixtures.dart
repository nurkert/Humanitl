// Bausteine der Intercept-Tests: Flows, Details, Ereignisse und ein
// DaemonClient, dessen Stream der Test selbst füttert.

import 'dart:async';
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

/// Das Detail zu [flow], mit [headers] und [bodyPreview].
FlowDetail detailFor(
  Flow flow, {
  List<Header> headers = const <Header>[],
  String bodyPreview = '',
  String contentType = '',
}) => FlowDetail(
  summary: flow,
  request: HttpRequest(
    method: flow.method,
    scheme: flow.scheme,
    authority: flow.authority,
    pathAndQuery: flow.path,
    headers: headers,
    body: BodyRef(
      sha256: List<int>.filled(32, 7),
      size: bodyPreview.length,
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
  Future<void> decide(FlowId id, Decision decision, {Rule? remember}) async {
    final Diagnostic? failure = decideFailure;
    if (failure != null) {
      throw DaemonException(failure);
    }
    decisions.add((id, decision));
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
  Stream<Uint8List> getBody(BodyRef ref) => const Stream<Uint8List>.empty();

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
