// Die Naht zwischen Queue und Schreibtisch (HUM-034): was das Tray gezeichnet
// bekommt, was der Fenstertitel trägt, was ein Druck auf eine Schaltfläche der
// Notification entscheidet und was passiert, wenn der Weg zurück ins Leere
// führt.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_events.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/shell/widgets/tray_host.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';
import 'package:humanitl/features/tray/providers/notice.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';
import 'package:humanitl/features/tray/widgets/return_banner.dart';

import 'fixtures.dart';

/// Lässt die Fenster ablaufen, die eine Entscheidung hinterlässt: das
/// Rückgängig-Fenster und das Bestätigungsfenster der Zeile.
Future<void> drainDecision(WidgetTester tester) async {
  await tester.pump(const Duration(seconds: 11));
  await tester.pump();
}

/// Baut die App über [client] und [desktop] und lässt das Gate durch.
Future<ProviderContainer> pumpDesktop(
  WidgetTester tester, {
  required FakeDaemonClient client,
  required FakeDesktop desktop,
  bool focused = true,
}) async {
  await tester.binding.setSurfaceSize(const Size(1400, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      connectionHeartbeatProvider.overrideWithValue(null),
      desktopPortsProvider.overrideWithValue(desktop.ports),
      // Eine stehende Uhr: der 250-ms-Ticker der Queue wuerde den Test mit
      // einem laufenden Timer beenden.
      nowProvider.overrideWith(() => TrayFixedNow(DateTime.now())),
    ],
  );
  addTearDown(container.dispose);
  await tester.pumpWidget(
    UncontrolledProviderScope(container: container, child: const HumanitlApp()),
  );
  await tester.pump();
  await tester.pump();
  if (!focused) {
    // Der Fokus geht, bevor die erste Anfrage eintrifft: nur dann ist der
    // Uebergang von null auf eins einer, den ein Mensch nicht sieht.
    desktop.window.emit(focused: false);
    await tester.pump();
  }
  // Das Skript des Fakes laeuft, und die Armierung aus `HMotion.rearm` ist
  // danach abgelaufen (`docs/UX.md` 5.4).
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  return container;
}

void main() {
  testWidgets('the_host_wraps_the_gate_and_draws_the_idle_tray', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient.empty(),
      desktop: desktop,
    );

    expect(find.byType(TrayHost), findsOneWidget);
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(desktop.tray.starts, 1);
    expect(desktop.tray.face.state, TrayIconState.idle);
    expect(desktop.tray.face.title, 'The queue is open');
    expect(desktop.window.titles.last, 'Humanitl');
  });

  testWidgets('window_title_and_tray_carry_the_count', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient(script: trayHoldScript(2)),
      desktop: desktop,
    );

    expect(desktop.window.titles.last, '(2) Humanitl');
    expect(desktop.tray.face.state, TrayIconState.held);
    expect(desktop.tray.face.count, 2);
    expect(desktop.tray.face.title, '2 requests held');
  });

  testWidgets('tray_init_failure_single_diagnostic', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop(
      missing: TrayDiagnostics.trayUnavailable('no watcher on the session bus'),
    );
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: FakeDaemonClient.empty(),
      desktop: desktop,
    );

    expect(find.byType(AttentionNoticeCard), findsOneWidget);
    expect(find.text('This desktop has no tray'), findsOneWidget);
    expect(find.text(DiagnosticCodes.noTray), findsOneWidget);

    // Dismissed once is dismissed for good; the same cause never comes back.
    await tester.tap(find.byKey(const Key('attention-notice-dismiss')));
    await tester.pump();
    expect(find.byType(AttentionNoticeCard), findsNothing);

    container
        .read(attentionNoticeProvider.notifier)
        .showOnce(TrayDiagnostics.trayUnavailable('again'));
    await tester.pump();
    expect(find.byType(AttentionNoticeCard), findsNothing);
  });

  testWidgets('action_allow_decides_without_showing_the_window', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: client,
      desktop: desktop,
      focused: false,
    );

    final HeldNotice notice = container.read(attentionProvider).notice!;
    expect(desktop.notifications.posts, hasLength(1));
    expect(desktop.notifications.posts.single.summary, 'api.github.com');

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    expect(client.decisions, hasLength(1));
    expect(client.decisions.single.flowId, notice.flowId);
    expect(client.decisions.single.decision, const Decision.allow());
    // The whole point: the request is decided and the window stays where it
    // was (HUM-034 acceptance).
    expect(desktop.window.reveals, 0);
    expect(container.read(attentionProvider).notice, isNull);
    await drainDecision(tester);
  });

  testWidgets('action_block_decides_the_named_request', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    await pumpDesktop(tester, client: client, desktop: desktop, focused: false);

    desktop.notifications.press(NotificationActionKind.block);
    await tester.pump();
    await tester.pump();

    expect(client.decisions.single.decision, isA<DecisionBlock>());
    expect(desktop.window.reveals, 0);
    await drainDecision(tester);
  });

  testWidgets('a_message_that_outlived_its_request_decides_nothing', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: client,
      desktop: desktop,
      focused: false,
    );
    final FlowId named = container.read(attentionProvider).notice!.flowId;

    // The request leaves the queue before the button is pressed.
    await client.decide(named, const Decision.block());
    await tester.pump();
    await drainDecision(tester);
    client.decisions.clear();

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions,
      isEmpty,
      reason: 'nothing is decided in its place',
    );
    expect(desktop.window.reveals, 1);
    expect(
      container.read(attentionNoticeProvider)?.code,
      DiagnosticCodes.flowNotHeld,
    );
  });

  testWidgets('a_finding_that_arrived_after_the_message_stops_allow', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(
      script: trayLateFindingScript(),
    );
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: client,
      desktop: desktop,
      focused: false,
    );

    // Die Meldung ging hinaus, als es noch keinen Fund gab, und bot deshalb
    // "Allow" an.
    final HeldNotice notice = container.read(attentionProvider).notice!;
    expect(notice.findings, 0);
    expect(notice.mayAllow, isTrue);
    // Der Fund kam danach.
    expect(container.read(flowsProvider)[notice.flowId]!.findingCount, 1);

    desktop.notifications.press(NotificationActionKind.allow);
    await tester.pump();
    await tester.pump();

    expect(
      client.decisions,
      isEmpty,
      reason: 'eine Anfrage mit Fund verlaesst die Queue nicht per Meldung',
    );
    expect(
      container.read(attentionNoticeProvider)?.code,
      DiagnosticCodes.decideRequestInvalid,
    );
    // Statt zu senden kommt das Fenster nach vorn, wo die Halte-Bestaetigung
    // und der Satz stehen (`docs/UX.md` 4.7).
    expect(desktop.window.reveals, 1);
    await tester.pump();
    expect(find.text('The request carries a finding'), findsOneWidget);
    await drainDecision(tester);
  });

  testWidgets('a_finding_that_arrived_after_the_message_still_blocks', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(
      script: trayLateFindingScript(),
    );
    await pumpDesktop(tester, client: client, desktop: desktop, focused: false);

    // Der Riegel gilt dem Senden, nicht dem Verweigern: Blockieren bleibt.
    desktop.notifications.press(NotificationActionKind.block);
    await tester.pump();
    await tester.pump();

    expect(client.decisions.single.decision, isA<DecisionBlock>());
    expect(desktop.window.reveals, 0);
    await drainDecision(tester);
  });

  testWidgets('quit_releases_the_desktop_before_it_destroys_the_window', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient.empty(),
      desktop: desktop,
    );

    desktop.tray.send(TrayCommand.quit);
    await tester.pump();
    await tester.pump();
    await tester.pump();

    expect(desktop.window.disposed, isTrue);
    expect(desktop.notifications.disposed, isTrue);
    expect(desktop.tray.disposed, isTrue);
    // Die Reihenfolge ist die Aussage: `quit` zerstoert das Fenster, und was
    // danach kaeme, kaeme nie.
    expect(desktop.log.entries, <String>[
      'window.dispose',
      'notifications.dispose',
      'tray.dispose',
      'window.quit',
    ]);
  });

  testWidgets('the_tray_waits_for_the_first_answer_before_it_claims_a_count', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    // Der Daemon haelt drei Anfragen, bevor die App ueberhaupt startet.
    await pumpDesktop(
      tester,
      client: trayDaemonAlreadyHolding(3),
      desktop: desktop,
    );

    // Ohne die Neusynchronisation der ersten Verbindung stuende hier Ruhe und
    // "The queue is open", obwohl drei Anfragen warten.
    expect(desktop.tray.face.state, TrayIconState.held);
    expect(desktop.tray.face.count, 3);
    expect(desktop.window.titles.last, '(3) Humanitl');

    // Und das erste Gesicht, das gezeichnet wurde, hat nie Ruhe behauptet.
    expect(
      desktop.tray.faces.first.state,
      TrayIconState.offline,
      reason: 'vor der ersten Antwort ist die Zahl unbekannt, nicht null',
    );
    expect(
      desktop.tray.faces.any(
        (TrayFace face) => face.state == TrayIconState.idle,
      ),
      isFalse,
    );
  });

  testWidgets('a_gap_in_the_stream_takes_the_count_off_the_tray', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: FakeDaemonClient(script: trayHoldScript(2)),
      desktop: desktop,
    );
    expect(desktop.tray.face.count, 2);
    expect(desktop.window.titles.last, '(2) Humanitl');

    // Der Ereignisstrom verbindet sich neu und schiebt ein `Lagged` ein. Die
    // Verbindung selbst gilt weiter als verbunden: `GetInfo` und der Strom
    // haengen nicht aneinander.
    container.read(attentionProvider.notifier).streamGapped();
    await tester.pump();
    await tester.pump();

    expect(container.read(connectionStateProvider), isA<ConnectionConnected>());
    expect(desktop.tray.face.state, TrayIconState.offline);
    expect(desktop.tray.face.count, 0);
    expect(desktop.window.titles.last, 'Humanitl');
  });

  testWidgets('the_host_answers_the_lagged_event_of_the_stream', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: trayDaemonAlreadyHolding(2),
      desktop: desktop,
    );
    expect(desktop.tray.face.count, 2);

    // Der Strom verbindet sich neu. Das ist der echte Weg, nicht der aus dem
    // Test: `flowEventsProvider` schiebt bei jeder Verbindung ein
    // synthetisches `Lagged` ein, und der Host muss es weiterreichen.
    final int drawn = desktop.tray.faces.length;
    container.invalidate(flowEventsProvider);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    await tester.pump();

    // Zwischen Luecke und Antwort stand das Symbol auf unbekannt. Ohne den
    // `Lagged`-Zweig im Host bliebe es die ganze Zeit bei der alten Zahl.
    expect(
      desktop.tray.faces
          .skip(drawn)
          .any((TrayFace face) => face.state == TrayIconState.offline),
      isTrue,
      reason: 'nach der Luecke ist die Zahl unbestaetigt',
    );
    // Die Verbindung selbst galt dabei durchgehend als verbunden.
    expect(container.read(connectionStateProvider), isA<ConnectionConnected>());

    // Und die Neusynchronisation bringt die Zahl zurueck.
    expect(desktop.tray.face.state, TrayIconState.held);
    expect(desktop.tray.face.count, 2);
  });

  testWidgets('a_press_on_a_message_the_service_never_replaced_is_answered', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final FakeDaemonClient client = FakeDaemonClient(script: trayHoldScript(1));
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: client,
      desktop: desktop,
      focused: false,
    );
    expect(desktop.notifications.posts.single.flowId, trayFlowId(1));

    // Ein Dienst, der `replaces_id` ignoriert, laesst die alte Meldung stehen.
    // Der Druck darauf traegt die alte Anfrage, die es nie gab.
    desktop.notifications.pressOn(NotificationActionKind.allow, trayFlowId(99));
    await tester.pump();
    await tester.pump();

    expect(client.decisions, isEmpty);
    // Ein Grund, kein Schweigen.
    expect(
      container.read(attentionNoticeProvider)?.code,
      DiagnosticCodes.flowNotHeld,
    );
    expect(desktop.window.reveals, 1);

    // Und die stehende Meldung wurde nicht an ihrer Stelle entschieden.
    expect(container.read(attentionProvider).notice, isNotNull);
    await drainDecision(tester);
  });

  testWidgets('tray_menu_shows_and_quits', (WidgetTester tester) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpDesktop(
      tester,
      client: FakeDaemonClient.empty(),
      desktop: desktop,
    );

    desktop.tray.send(TrayCommand.show);
    await tester.pump();
    await tester.pump();
    expect(desktop.window.reveals, 1);

    desktop.tray.send(TrayCommand.quit);
    await tester.pump();
    await tester.pump();
    expect(desktop.window.quits, 1);
  });

  testWidgets('the_banner_leads_to_the_request_that_waited_longest', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: FakeDaemonClient(script: trayHoldScript(2)),
      desktop: desktop,
    );
    expect(find.byType(ReturnBanner), findsNothing);

    // Away, and back a while later: the oldest request has waited long enough.
    desktop.window.emit(focused: false);
    await tester.pump();
    container.read(attentionProvider.notifier).heldChanged(<Flow>[
      trayHeldFlow(n: 9, waited: const Duration(minutes: 3)),
    ]);
    desktop.window.emit(focused: true);
    await tester.pump();
    await tester.pump();

    expect(find.byType(ReturnBanner), findsOneWidget);
    expect(find.text('The agent has been waiting 3 minutes'), findsOneWidget);

    await tester.tap(find.byKey(const Key('return-banner-dismiss')));
    await tester.pump();
    expect(find.byType(ReturnBanner), findsNothing);
  });

  testWidgets('the_tray_says_unknown_when_the_daemon_stops_answering', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    final ProviderContainer container = await pumpDesktop(
      tester,
      client: FakeDaemonClient(script: trayHoldScript(2)),
      desktop: desktop,
    );
    expect(desktop.tray.face.count, 2);

    container
        .read(attentionProvider.notifier)
        .connectionChanged(connected: false);
    await tester.pump();
    await tester.pump();

    expect(desktop.tray.face.state, TrayIconState.offline);
    expect(desktop.tray.face.count, 0);
    expect(desktop.window.titles.last, 'Humanitl');
  });
}
