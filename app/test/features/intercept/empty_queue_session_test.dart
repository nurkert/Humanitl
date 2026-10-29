// Die leere Warteschlange weiß, ob der Agent arbeitet (HUM-306).
//
// Bericht des Besitzers vom 2026-10-29: Intercept bot „Start session" an,
// während pi im Terminal lief. Ohne Session bleibt der Start; läuft der Agent,
// sagt die Warteschlange, dass er arbeitet, und führt zu seinem Terminal.
// Gebaut wird die ganze App, denn das eine Bit kommt über die Shell aus dem
// Sandbox-Feature, und genau diese Leitung ist zu prüfen.

import 'dart:async';

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/agent_session.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/agent_session_feed.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';

import '../../harness/ui_state.dart';
import 'fixtures.dart';
import 'harness.dart';

/// Ein Fake ohne angehaltene Anfrage, dessen Agent je nach [agentRunning]
/// in einer laufenden Sandbox arbeitet oder dessen Sandbox steht.
FakeDaemonClient emptyQueue({required bool agentRunning}) {
  final FakeDaemonClient client = fakeDaemon(const <ScriptedEvent>[]);
  if (agentRunning) {
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      startedAt: testStart,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
  }
  return client;
}

/// Ein Fake, der zählt, wie oft nach dem Schnappschuss gefragt wird, und
/// dessen Start auf Wunsch offen bleibt, bis der Agent endet.
///
/// Der echte Daemon hält den Strom des Starts die ganze Sitzung offen und
/// schließt ihn mit `SandboxEvent.Exit`, ohne danach noch einen `Status` zu
/// schicken. Der Fake schließt ihn schon bei `running`; mit [agentEnd]
/// spielt dieser Client den echten Ablauf nach.
class SessionClient extends FakeDaemonClient {
  /// Erzeugt den Client ohne angehaltene Anfrage.
  SessionClient()
    : super(script: const <ScriptedEvent>[], clock: () => testStart);

  /// Wie oft `Sandbox(Status)` gerufen wurde.
  int statusCalls = 0;

  /// Ist er gesetzt, endet der Agent, sobald er erfüllt wird.
  Completer<void>? agentEnd;

  /// Ein Agent, der läuft, ohne dass dieses Fenster ihn gestartet hat: so
  /// sieht eine Sitzung von der Kommandozeile aus.
  void startElsewhere() => sandbox = sandbox.copyWith(
    state: SandboxState.running,
    agentRunning: true,
    startedAt: testStart,
    sandboxId: FakeDaemonClient.defaultSandbox,
  );

  /// Die Sitzung von der Kommandozeile endet.
  void stopElsewhere() => sandbox = sandbox.copyWith(
    state: SandboxState.stopped,
    agentRunning: false,
    sandboxId: null,
  );

  /// Ist er gesetzt, antwortet `Sandbox(Status)` erst, wenn er erfüllt
  /// wird, und dann mit dem Stand vom Augenblick der Frage.
  Completer<void>? holdStatus;

  /// Ein Befund, den `Sandbox(Status)` mit jedem Schnappschuss wiederholt,
  /// wie der Daemon die Befunde seiner Vorabprüfung.
  Diagnostic? statusFinding;

  @override
  Stream<SandboxUpdate> sandboxStatus() async* {
    statusCalls++;
    final SandboxStatus asked = sandbox;
    final Completer<void>? hold = holdStatus;
    if (hold != null) {
      await hold.future;
      yield SandboxUpdate.status(asked);
      return;
    }
    final Diagnostic? finding = statusFinding;
    if (finding != null) {
      yield SandboxUpdate.diagnostic(finding);
    }
    yield* super.sandboxStatus();
  }

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    WorkMode? workMode,
  }) async* {
    yield* super.startSandbox(profile: profile, workMode: workMode);
    final Completer<void>? end = agentEnd;
    if (end != null) {
      await end.future;
      sandbox = sandbox.copyWith(agentRunning: false);
      yield const SandboxUpdate.exit(0);
    }
  }
}

/// Die Verbindung, wie ein Test sie stellt.
class FakeLink extends Notifier<bool> {
  @override
  bool build() => true;

  /// Setzt die Verbindung auf [live].
  void set({required bool live}) => state = live;
}

/// Der Schalter hinter [linkLiveProvider] in den Tests, die ihn brauchen.
final NotifierProvider<FakeLink, bool> fakeLinkProvider =
    NotifierProvider<FakeLink, bool>(FakeLink.new);

/// Hält einen eigenen Zubringer, wie eine zweite Shell es täte.
class FeedHost extends ConsumerStatefulWidget {
  /// Meldet den Zubringer an [onFeed].
  const FeedHost({required this.onFeed, super.key});

  /// Bekommt den Zubringer, sobald er steht.
  final void Function(AgentSessionFeed feed) onFeed;

  @override
  ConsumerState<FeedHost> createState() => _FeedHostState();
}

class _FeedHostState extends ConsumerState<FeedHost> {
  late final AgentSessionFeed _feed;

  @override
  void initState() {
    super.initState();
    _feed = AgentSessionFeed(ref);
    widget.onFeed(_feed);
  }

  @override
  void dispose() {
    _feed.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}

/// Ein Befund, den der Daemon mit jedem Schnappschuss wiederholt.
const Diagnostic preflightFinding = Diagnostic(
  code: 'SANDBOX_001',
  severity: Severity.blocking,
  title: 'sandbox backend not found',
  why: 'the sandbox backend is not on PATH, so no sandbox can be started.',
);

/// Ein paar Frames: Schnappschuss der Sandbox, Bit in den Kern, Neubau.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 6; i++) {
    await tester.pump();
  }
}

/// Der Leerzustand sagt, dass der Agent arbeitet, und bietet sein Terminal an.
void expectWorking() {
  expect(find.byType(QueueEmptyState), findsOneWidget);
  expect(find.byKey(const Key('queue-start-session')), findsNothing);
  expect(find.byKey(const Key('queue-go-to-terminal')), findsOneWidget);
  expect(find.text('The agent is working'), findsOneWidget);
  expect(
    find.text('Every request that needs you appears here.'),
    findsOneWidget,
  );
  expect(find.text('Go to terminal'), findsOneWidget);
  expect(find.text('The queue is open'), findsNothing);
  expectKeysTaught();
}

/// Der Leerzustand ohne Session, wie vor HUM-306.
void expectIdle() {
  expect(find.byType(QueueEmptyState), findsOneWidget);
  expect(find.byKey(const Key('queue-start-session')), findsOneWidget);
  expect(find.byKey(const Key('queue-go-to-terminal')), findsNothing);
  expect(find.text('The queue is open'), findsOneWidget);
  expect(
    find.text(
      'Once a session runs, every request the agent makes appears here.',
    ),
    findsOneWidget,
  );
  expect(find.text('The agent is working'), findsNothing);
  expectKeysTaught();
}

/// Die Lehrzeile mit den drei Tasten steht in beiden Fällen.
void expectKeysTaught() => expect(
  find.text('`B` block · `Enter` send (final) · `J`/`K` move'),
  findsOneWidget,
);

void main() {
  testWidgets('a running agent turns the start into the way to its terminal', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: emptyQueue(agentRunning: true));
    await settle(tester);
    final ProviderContainer container = containerOf(tester);
    expect(container.read(navigationProvider), Section.intercept);
    expectWorking();

    await tester.tap(find.byKey(const Key('queue-go-to-terminal')));
    await settle(tester);
    expect(
      container.read(navigationProvider),
      Section.sandbox,
      reason: 'the button leads to the section with the terminal',
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('without a session the empty queue still leads to the start', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: emptyQueue(agentRunning: false));
    await settle(tester);
    expectIdle();
    final ProviderContainer container = containerOf(tester);

    await tester.tap(find.byKey(const Key('queue-start-session')));
    await settle(tester);
    expect(container.read(navigationProvider), Section.setup);
  });

  testWidgets('a sandbox whose agent has exited offers the start again', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = emptyQueue(agentRunning: true);
    client.sandbox = client.sandbox.copyWith(agentRunning: false);
    await pumpIntercept(tester, client: client);
    await settle(tester);
    expectIdle();
  });

  testWidgets('the empty state follows the session as it starts and stops', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(tester, client: emptyQueue(agentRunning: false));
    await settle(tester);
    expectIdle();
    final ProviderContainer container = containerOf(tester);

    // Gestartet aus dem Sandbox-Feature, während die Warteschlange zu sehen
    // ist: dieselbe Bahn wie der Start-Knopf der Einrichtung.
    await container.read(sandboxStatusProvider.notifier).start();
    await settle(tester);
    expect(container.read(agentSessionProvider), isTrue);
    expectWorking();

    await container.read(sandboxStatusProvider.notifier).stop();
    await settle(tester);
    expect(container.read(agentSessionProvider), isFalse);
    expectIdle();
    expect(tester.takeException(), isNull);
  });

  testWidgets('an agent the snapshot already knows arrives without a change', (
    WidgetTester tester,
  ) async {
    // Der Schnappschuss der Sandbox lebt länger als die Shell (`keepAlive`).
    // Steht er schon, wenn die Shell entsteht, meldet ihr Zuhörer keine
    // Änderung mehr; der erste Wert muss trotzdem hinüber.
    final ProviderContainer container = ProviderContainer(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(emptyQueue(agentRunning: true)),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
        uiStateOverride(),
      ],
    );
    final SandboxStatus known = await container.read(
      sandboxStatusProvider.future,
    );
    expect(known.agentRunning, isTrue);

    await tester.binding.setSurfaceSize(const Size(1400, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: const HumanitlApp(),
      ),
    );
    await settle(tester);
    expect(container.read(agentSessionProvider), isTrue);
    expectWorking();

    // Der Container gehört dem Test und nicht dem Baum; seine Timer enden
    // mit ihm, bevor der Test seine Invarianten prüft.
    await tester.pumpWidget(const SizedBox());
    container.dispose();
    await tester.pump(const Duration(seconds: 1));
  });

  testWidgets('the end of the agent on the start stream reaches the queue', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient()..agentEnd = Completer<void>();
    await pumpIntercept(tester, client: client);
    await settle(tester);
    final ProviderContainer container = containerOf(tester);
    unawaited(container.read(sandboxStatusProvider.notifier).start());
    await settle(tester);
    expectWorking();

    // Kein Warten auf die nächste Frage: Das `Exit` selbst ändert den Stand.
    client.agentEnd!.complete();
    await settle(tester);
    expect(container.read(agentSessionProvider), isFalse);
    expectIdle();
  });

  testWidgets('a session of the command line appears and ends in the queue', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient();
    await pumpIntercept(tester, client: client);
    await settle(tester);
    expectIdle();

    client.startElsewhere();
    await tester.pump(agentSessionPollInterval);
    await settle(tester);
    expectWorking();

    client.stopElsewhere();
    await tester.pump(agentSessionPollInterval);
    await settle(tester);
    expectIdle();
  });

  testWidgets('the shell asks only while the empty queue is in view', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient();
    await pumpIntercept(tester, client: client);
    await settle(tester);
    final ProviderContainer container = containerOf(tester);
    container.read(navigationProvider.notifier).go(Section.history);
    await settle(tester);
    final int before = client.statusCalls;

    client.startElsewhere();
    await tester.pump(agentSessionPollInterval * 3);
    await settle(tester);
    expect(
      client.statusCalls,
      before,
      reason: 'no question while another section shows',
    );
    expect(container.read(agentSessionProvider), isFalse);

    // Zurück in der Warteschlange: sofort eine Frage, ohne fünf Sekunden.
    container.read(navigationProvider.notifier).go(Section.intercept);
    await settle(tester);
    expect(client.statusCalls, greaterThan(before));
    expectWorking();
  });

  testWidgets('the window coming back to the front asks at once', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient();
    await pumpIntercept(tester, client: client);
    await settle(tester);
    expectIdle();

    client.startElsewhere();
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    await settle(tester);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await settle(tester);
    expectWorking();
  });

  testWidgets('without a link the queue does not claim that the agent works', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: emptyQueue(agentRunning: true),
      overrides: <Override>[
        linkLiveProvider.overrideWith((Ref ref) => ref.watch(fakeLinkProvider)),
      ],
    );
    await settle(tester);
    final ProviderContainer container = containerOf(tester);
    expect(container.read(agentSessionProvider), isTrue);

    container.read(fakeLinkProvider.notifier).set(live: false);
    await settle(tester);
    expect(container.read(agentSessionProvider), isFalse);
    expect(find.text('The agent is working'), findsNothing);

    container.read(fakeLinkProvider.notifier).set(live: true);
    await settle(tester);
    expect(container.read(agentSessionProvider), isTrue);
  });

  testWidgets('an unanswered background question leaves the snapshot alone', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = emptyQueue(agentRunning: true);
    await pumpIntercept(tester, client: client);
    await settle(tester);
    final ProviderContainer container = containerOf(tester);

    // Die Frage im Hintergrund scheitert; die Sandbox-Ansicht soll deshalb
    // nicht rot werden, und was zuletzt galt, gilt weiter.
    client.goOffline();
    await container.read(sandboxStatusProvider.notifier).pollStatus();
    await settle(tester);
    final AsyncValue<SandboxStatus> status = container.read(
      sandboxStatusProvider,
    );
    expect(status.hasError, isFalse);
    expect(status.value?.agentRunning, isTrue);
    client.goOnline();
  });

  testWidgets(
    'a snapshot asked before the end of the agent does not revive it',
    (WidgetTester tester) async {
      final SessionClient client = SessionClient()
        ..agentEnd = Completer<void>();
      await pumpIntercept(tester, client: client);
      await settle(tester);
      final ProviderContainer container = containerOf(tester);
      unawaited(container.read(sandboxStatusProvider.notifier).start());
      await settle(tester);
      expectWorking();

      // Eine Frage geht hinaus, solange der Agent noch läuft, und bleibt
      // unterwegs.
      final Completer<void> late = Completer<void>();
      client.holdStatus = late;
      await tester.pump(agentSessionPollInterval);
      await settle(tester);

      // Der Agent endet, dann kommt die alte Antwort: „läuft".
      client.agentEnd!.complete();
      await settle(tester);
      expectIdle();
      client.holdStatus = null;
      late.complete();
      await settle(tester);
      expect(
        container.read(sandboxStatusProvider).value?.agentRunning,
        isFalse,
      );
      expectIdle();
    },
  );

  testWidgets('background questions leave the findings as they were', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient()
      ..statusFinding = preflightFinding;
    await pumpIntercept(tester, client: client);
    await settle(tester);
    // Ein blockierender Befund öffnet die Einrichtung; zurück in die
    // Warteschlange, wo gefragt wird.
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(HumanitlApp)),
    );
    container.read(navigationProvider.notifier).go(Section.intercept);
    await settle(tester);
    container.read(sandboxCallProvider.notifier).record(SandboxCall.start);
    final int before = client.statusCalls;

    for (int i = 0; i < 3; i++) {
      await tester.pump(agentSessionPollInterval);
      await settle(tester);
    }
    expect(client.statusCalls, greaterThanOrEqualTo(before + 3));
    final List<Diagnostic> findings = container
        .read(sandboxStatusProvider)
        .value!
        .diagnostics;
    expect(findings, hasLength(1), reason: 'no copy per question');
    expect(
      container.read(sandboxCallProvider),
      SandboxCall.start,
      reason: 'a question does not claim the finding',
    );
  });

  testWidgets('one question at a time, and a hanging one gives up', (
    WidgetTester tester,
  ) async {
    final SessionClient client = SessionClient();
    await pumpIntercept(tester, client: client);
    await settle(tester);
    // Ab hier hängt jede Frage.
    client.holdStatus = Completer<void>();
    await tester.pump(agentSessionPollInterval);
    await settle(tester);
    final int hanging = client.statusCalls;

    // Der nächste Takt und ein Fenster, das nach vorn kommt, fragen nicht
    // ein zweites Mal, solange die erste Frage unterwegs ist.
    await tester.pump(agentSessionPollInterval);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.inactive);
    tester.binding.handleAppLifecycleStateChanged(AppLifecycleState.resumed);
    await settle(tester);
    expect(client.statusCalls, hanging, reason: 'questions are coalesced');

    // Nach der Frist gibt sie auf, und der nächste Takt fragt wieder.
    await tester.pump(sandboxPollDeadline);
    await tester.pump(agentSessionPollInterval);
    await settle(tester);
    expect(client.statusCalls, greaterThan(hanging));
    client.holdStatus!.complete();
    await settle(tester);
  });

  testWidgets('a second shell feeds nothing', (WidgetTester tester) async {
    final List<AgentSessionFeed> feeds = <AgentSessionFeed>[];
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          daemonClientProvider.overrideWithValue(SessionClient()),
          connectionHeartbeatProvider.overrideWithValue(null),
          uiStateOverride(),
        ],
        child: Directionality(
          textDirection: TextDirection.ltr,
          child: Column(
            children: <Widget>[
              FeedHost(onFeed: feeds.add),
              FeedHost(onFeed: feeds.add),
            ],
          ),
        ),
      ),
    );
    await settle(tester);
    expect(feeds, hasLength(2));
    expect(feeds.first.idle, isFalse);
    expect(feeds.last.idle, isTrue);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 1));
  });

  test('the sandbox index of the core is the index of the Sandbox section', () {
    // Die Warteschlange darf `Section` nicht kennen und führt über diese
    // Zahl zum Terminal; rückt ein Abschnitt, muss sie mitrücken.
    expect(sandboxSectionIndex, Section.sandbox.index);
  });
}
