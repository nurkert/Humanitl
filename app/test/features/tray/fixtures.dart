// Bausteine der Tray-Tests: Flows, Fake-Ports und ein Container über der
// Attention-Maschine. Bewusst eigenständig, damit diese Tests nicht an den
// Fixtures eines anderen Screens hängen.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

/// Die Session, zu der jeder Test-Flow gehört.
const SessionId traySession = SessionId('018f0001-0000-7000-8000-000000000009');

/// Die Id des n-ten Test-Flows.
FlowId trayFlowId(int n) =>
    FlowId('018f0034-0000-7000-8000-${n.toString().padLeft(12, '0')}');

/// Ein angehaltener Flow.
///
/// Die Zeiten hängen an `DateTime.now()`, weil die Maschine ihre Fristen
/// selbst liest: `fake_async` fälscht Timer, nicht die Uhr.
Flow trayHeldFlow({
  required int n,
  Duration waited = Duration.zero,
  Duration remaining = const Duration(minutes: 5),
  String host = 'api.github.com',
  String path = '/graphql',
  Method method = Method.get,
  int findings = 0,
}) {
  final DateTime now = DateTime.now();
  final DateTime heldAt = now.subtract(waited);
  return Flow(
    id: trayFlowId(n),
    sessionId: traySession,
    receivedAt: heldAt,
    method: method,
    scheme: Scheme.https,
    authority: Authority(host: host, port: 443),
    path: path,
    state: FlowState.held,
    findingCount: findings,
    deadline: now.add(remaining),
    heldAt: heldAt,
  );
}

/// Was am Schreibtisch der Reihe nach passiert ist.
///
/// Ein einziges Protokoll für alle drei Ports, weil die Reihenfolge zwischen
/// ihnen die Aussage ist: aufräumen, dann das Fenster zerstören.
class DesktopLog {
  /// Die Einträge, in der Reihenfolge, in der sie geschrieben wurden.
  final List<String> entries = <String>[];

  /// Schreibt [entry].
  void add(String entry) => entries.add(entry);
}

/// Ein Fenster, dessen Fokuswechsel der Test selbst schickt.
class FakeWindow implements WindowPort {
  /// Erzeugt ein Fenster, das nach [log] schreibt.
  FakeWindow({DesktopLog? log}) : log = log ?? DesktopLog();

  /// Das gemeinsame Protokoll.
  final DesktopLog log;

  final StreamController<bool> _focus = StreamController<bool>.broadcast();

  /// Die Titel, die gesetzt wurden, in der Reihenfolge.
  final List<String> titles = <String>[];

  /// Wie oft das Fenster nach vorn geholt wurde.
  int reveals = 0;

  /// Wie oft das Programm beendet werden sollte.
  int quits = 0;

  /// Ob der Port freigegeben wurde.
  bool disposed = false;

  @override
  Stream<bool> get focus => _focus.stream;

  /// Schickt einen Fokuswechsel.
  void emit({required bool focused}) => _focus.add(focused);

  @override
  Future<void> setTitle(String title) async => titles.add(title);

  @override
  Future<void> reveal() async => reveals++;

  @override
  Future<void> quit() async {
    quits++;
    log.add('window.quit');
  }

  @override
  Future<void> dispose() async {
    disposed = true;
    log.add('window.dispose');
    await _focus.close();
  }
}

/// Eine Notification, die sich merkt, was sie zeigen sollte.
class FakeNotifications implements NotificationPort {
  /// Erzeugt eine Notification, die nach [log] schreibt.
  FakeNotifications({DesktopLog? log}) : log = log ?? DesktopLog();

  /// Das gemeinsame Protokoll.
  final DesktopLog log;

  final StreamController<NotificationAnswer> _actions =
      StreamController<NotificationAnswer>.broadcast();

  /// Jede Meldung, die gezeigt wurde.
  final List<DesktopNotification> posts = <DesktopNotification>[];

  /// Wie oft eine Meldung zurückgenommen wurde.
  int withdrawals = 0;

  /// Ob der Port freigegeben wurde.
  bool disposed = false;

  @override
  Stream<NotificationAnswer> get actions => _actions.stream;

  /// Schickt einen Druck auf eine der Schaltflächen der stehenden Meldung.
  void press(NotificationActionKind kind) =>
      _actions.add(NotificationAnswer(kind: kind, flowId: posts.last.flowId));

  /// Schickt einen Druck auf eine Meldung, die der Dienst nie ersetzt hat.
  void pressOn(NotificationActionKind kind, FlowId flowId) =>
      _actions.add(NotificationAnswer(kind: kind, flowId: flowId));

  @override
  Future<void> post(DesktopNotification notification) async =>
      posts.add(notification);

  @override
  Future<void> withdraw() async => withdrawals++;

  @override
  Future<void> dispose() async {
    disposed = true;
    log.add('notifications.dispose');
    await _actions.close();
  }
}

/// Ein Tray, das sich merkt, was es zeichnen sollte.
class FakeTray implements TrayPort {
  /// Erzeugt ein Tray, das beim Start [missing] meldet.
  FakeTray({this.missing, DesktopLog? log}) : log = log ?? DesktopLog();

  /// Das gemeinsame Protokoll.
  final DesktopLog log;

  /// Was `start` zurückgibt: null für ein Tray, das es gibt.
  final Diagnostic? missing;

  final StreamController<TrayCommand> _commands =
      StreamController<TrayCommand>.broadcast();

  /// Jedes Gesicht, das gezeichnet wurde.
  final List<TrayFace> faces = <TrayFace>[];

  /// Wie oft `start` aufgerufen wurde.
  int starts = 0;

  /// Ob der Port freigegeben wurde.
  bool disposed = false;

  /// Das zuletzt gezeichnete Gesicht.
  TrayFace get face => faces.last;

  @override
  Stream<TrayCommand> get commands => _commands.stream;

  /// Schickt einen Befehl aus dem Menü oder vom Icon.
  void send(TrayCommand command) => _commands.add(command);

  @override
  Future<Diagnostic?> start() async {
    starts++;
    return missing;
  }

  @override
  Future<void> show(TrayFace face) async => faces.add(face);

  @override
  Future<void> dispose() async {
    disposed = true;
    log.add('tray.dispose');
    await _commands.close();
  }
}

/// Die drei Fakes zusammen.
class FakeDesktop {
  /// Erzeugt einen Schreibtisch, dessen Tray beim Start [missing] meldet.
  FakeDesktop({Diagnostic? missing}) : log = DesktopLog() {
    window = FakeWindow(log: log);
    notifications = FakeNotifications(log: log);
    tray = FakeTray(missing: missing, log: log);
  }

  /// Was der Reihe nach passiert ist, über alle drei Ports hinweg.
  final DesktopLog log;

  /// Das Fenster.
  late final FakeWindow window;

  /// Die Notification.
  late final FakeNotifications notifications;

  /// Das Tray.
  late final FakeTray tray;

  /// Die Ports, wie der Provider sie erwartet.
  DesktopPorts get ports =>
      DesktopPorts(window: window, notifications: notifications, tray: tray);
}

/// Ein Container über der Attention-Maschine.
ProviderContainer trayContainer({
  FakeDesktop? desktop,
  bool notifications = true,
}) {
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      if (desktop != null)
        desktopPortsProvider.overrideWithValue(desktop.ports),
      notificationsEnabledProvider.overrideWithValue(notifications),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

/// Ein Skript für den Fake-Daemon: [count] Anfragen kommen an und werden
/// angehalten, im Abstand [spacing], mit dem Budget [budget].
List<ScriptedEvent> trayHoldScript(
  int count, {
  Duration spacing = const Duration(milliseconds: 10),
  Duration budget = const Duration(minutes: 5),
}) {
  final List<ScriptedEvent> script = <ScriptedEvent>[];
  for (int i = 1; i <= count; i++) {
    final FlowId id = trayFlowId(i);
    final Duration at = spacing * i;
    script
      ..add(
        ScriptedEvent(at, (FakeSessionState state, DateTime now) {
          final Flow flow = Flow(
            id: id,
            sessionId: traySession,
            receivedAt: now,
            method: Method.get,
            scheme: Scheme.https,
            authority: const Authority(host: 'api.github.com', port: 443),
            path: '/graphql',
            state: FlowState.received,
          );
          state.details[id] = FlowDetail(summary: flow);
          return FlowEvent.received(at: now, flow: flow);
        }),
      )
      ..add(
        ScriptedEvent(
          at + const Duration(milliseconds: 1),
          (FakeSessionState state, DateTime now) =>
              FlowEvent.held(at: now, flowId: id, deadline: now.add(budget)),
        ),
      );
  }
  return script;
}

/// Ein Skript, in dem der Fund erst nach dem Halten eintrifft.
///
/// Analyse und Halten sind zwei Ereignisse, und das zweite kann dem ersten
/// folgen: die Meldung geht mit null Funden hinaus und steht noch, wenn der
/// Fund ankommt.
///
/// Das zweite `held` am Ende ist kein Schmuck. `FakeDaemonClient._apply`
/// setzt einen Flow bei `Analyzed` bedingungslos auf `analyzed`, während der
/// Daemon einen gehaltenen Flow gehalten lässt; ohne das zweite Ereignis
/// hielte der Fake den Flow für nicht mehr wartend und wiese jede
/// Entscheidung mit `IPC_003` ab.
List<ScriptedEvent> trayLateFindingScript() {
  final FlowId id = trayFlowId(1);
  return <ScriptedEvent>[
    ScriptedEvent(const Duration(milliseconds: 10), (
      FakeSessionState state,
      DateTime now,
    ) {
      final Flow flow = Flow(
        id: id,
        sessionId: traySession,
        receivedAt: now,
        method: Method.post,
        scheme: Scheme.https,
        authority: const Authority(host: 'api.github.com', port: 443),
        path: '/graphql',
        state: FlowState.received,
      );
      state.details[id] = FlowDetail(summary: flow);
      return FlowEvent.received(at: now, flow: flow);
    }),
    ScriptedEvent(
      const Duration(milliseconds: 11),
      (FakeSessionState state, DateTime now) => FlowEvent.held(
        at: now,
        flowId: id,
        deadline: now.add(const Duration(minutes: 5)),
      ),
    ),
    ScriptedEvent(
      const Duration(milliseconds: 200),
      (FakeSessionState state, DateTime now) => FlowEvent.analyzed(
        at: now,
        flowId: id,
        findings: const <Finding>[
          Finding(
            kind: 'aws_access_key',
            location: FindingLocation.body,
            spanStart: 0,
            spanEnd: 20,
            tier: FindingTier.checksum,
          ),
        ],
      ),
    ),
    ScriptedEvent(
      const Duration(milliseconds: 210),
      (FakeSessionState state, DateTime now) => FlowEvent.held(
        at: now,
        flowId: id,
        deadline: now.add(const Duration(minutes: 5)),
      ),
    ),
  ];
}

/// Ein Fake, dessen Daemon [count] Anfragen hält, bevor die App startet.
///
/// Kein Skript: der Daemon hielt sie schon, als niemand zuhörte. `Subscribe`
/// beginnt "ab jetzt", also erfährt die App davon nur über die
/// Neusynchronisation der ersten Verbindung.
FakeDaemonClient trayDaemonAlreadyHolding(int count) {
  final FakeDaemonClient client = FakeDaemonClient(
    script: const <ScriptedEvent>[],
  );
  final DateTime now = DateTime.now();
  for (int i = 1; i <= count; i++) {
    final FlowId id = trayFlowId(i);
    final Flow flow = Flow(
      id: id,
      sessionId: traySession,
      receivedAt: now,
      method: Method.get,
      scheme: Scheme.https,
      authority: const Authority(host: 'api.github.com', port: 443),
      path: '/graphql',
      state: FlowState.held,
      deadline: now.add(const Duration(minutes: 5)),
      heldAt: now,
    );
    client.state.flows[id] = flow;
    client.state.details[id] = FlowDetail(summary: flow);
  }
  return client;
}

/// Eine stehende Uhr: kein Ticker, damit ein Widget-Test keinen laufenden
/// Timer hinterlässt.
class TrayFixedNow extends Now {
  /// Startet bei [_at].
  TrayFixedNow(this._at);

  final DateTime _at;

  @override
  DateTime build() => _at;
}
