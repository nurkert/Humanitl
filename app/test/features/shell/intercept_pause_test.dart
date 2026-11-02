// Die Pause des Abfangens in der Kopfleiste (HUM-407): die Pille als Knopf,
// die Rückfrage mit vorgewähltem Abbrechen, das Absperrband in jeder Sektion
// und das Einschalten ohne Rückfrage.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/intercept_pill.dart';

import '../../harness/app_harness.dart';

const Key pill = Key('header-intercept-badge');
const Key tape = Key('shell-intercept-tape');
const Key modal = Key('intercept-pause-modal');

String pillText(WidgetTester tester) =>
    tester.widget<HBadge>(find.byKey(pill)).text;

/// Der Fake eines Daemons, der pausieren kann.
FakeDaemonClient pausable() =>
    FakeDaemonClient(info: FakeIntercept.interceptInfo);

/// Bis zur Pause: Pille, Rückfrage, der bewusste Druck.
Future<void> pauseThroughTheDialog(WidgetTester tester) async {
  await tester.tap(find.byKey(pill));
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(const Key('intercept-pause-confirm')));
  await tester.pumpAndSettle();
}

/// Ein Fake, dessen `GetInfo` der Test zwischen zwei Herzschlägen ändert
/// und dessen Antwort auf einen Wechsel er vorgeben kann.
class _Beating extends FakeDaemonClient {
  _Beating(DaemonInfo first) : current = first, super(info: first);

  /// Was der nächste Herzschlag hört.
  DaemonInfo current;

  /// Wenn gesetzt, die Antwort auf `Intercept(pause)`.
  InterceptStatus? pauseReply;

  @override
  Future<DaemonInfo> getInfo() async {
    await super.getInfo();
    return current;
  }

  @override
  Future<InterceptStatus> intercept(InterceptOp op) async {
    if (op == InterceptOp.pause && pauseReply != null) {
      interceptCalls.add(op);
      return pauseReply!;
    }
    return super.intercept(op);
  }
}

const Duration beat = Duration(seconds: 2);
const String oldDaemon = '018f0001-0000-7000-8000-0000000004b0';
const String newDaemon = '018f0001-0000-7000-8000-0000000004b1';

DaemonInfo infoOf(String instance, InterceptStatus? status) => FakeIntercept
    .interceptInfo
    .copyWith(sessionId: instance, intercept: status);

InterceptStatus pausedAt(int revision, String instance) => InterceptStatus(
  paused: true,
  pausedSince: DateTime.utc(2026, 11, 1, 9),
  revision: revision,
  instance: instance,
);

/// Der nächste Herzschlag, und was er auslöst.
Future<void> nextBeat(WidgetTester tester) async {
  await tester.pump(beat);
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('without the capability the pill only shows', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpApp(tester, client: client);
    expect(pillText(tester), 'Intercept ON');
    expect(tester.widget<HBadge>(find.byKey(pill)).onTap, isNull);
    await tester.tap(find.byKey(pill), warnIfMissed: false);
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsNothing);
    expect(
      client.interceptCalls,
      isEmpty,
      reason: 'an old daemon is not asked',
    );
  });

  testWidgets('the question opens with Cancel preselected and pauses nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = pausable();
    await pumpApp(tester, client: client);
    expect(tester.widget<HBadge>(find.byKey(pill)).onTap, isNotNull);

    await tester.tap(find.byKey(pill));
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsOneWidget);
    expect(find.byType(InterceptPauseDialog), findsOneWidget);

    // Enter trifft, was den Fokus hat: Abbrechen. Kein Druck auf die Taste
    // pausiert etwas.
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsNothing);
    expect(client.interceptCalls, isNot(contains(InterceptOp.pause)));
    expect(find.byKey(tape), findsNothing);

    // Escape bricht ebenso ab.
    await tester.tap(find.byKey(pill));
    await tester.pumpAndSettle();
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsNothing);
    expect(client.interceptCalls, isNot(contains(InterceptOp.pause)));
  });

  testWidgets('a deliberate press pauses, and the tape runs in every section', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = pausable();
    await pumpApp(tester, client: client);
    await pauseThroughTheDialog(tester);

    expect(client.interceptCalls, contains(InterceptOp.pause));
    expect(pillText(tester), 'Intercept PAUSED');
    final HTokens tokens = HTheme.of(tester.element(find.byKey(pill)));
    expect(
      tester.widget<HBadge>(find.byKey(pill)).color,
      tokens.colors.warning,
    );
    // In jeder Sektion, über die ganze Breite des Fensters.
    for (final Section section in Section.values) {
      await pressCtrl(
        tester,
        LogicalKeyboardKey(LogicalKeyboardKey.digit1.keyId + section.index),
      );
      expect(find.byKey(tape), findsOneWidget, reason: section.name);
      expect(
        tester.getSize(find.byKey(tape)).width,
        1280,

        reason: section.name,
      );
    }
  });

  testWidgets('a click on the paused pill resumes without a question', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = pausable();
    await pumpApp(tester, client: client);
    await pauseThroughTheDialog(tester);
    expect(find.byKey(tape), findsOneWidget);

    await tester.tap(find.byKey(pill));
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsNothing, reason: 'no question to resume');
    expect(client.interceptCalls.last, InterceptOp.resume);
    expect(pillText(tester), 'Intercept ON');
    expect(find.byKey(tape), findsNothing);
  });

  testWidgets('a pause of another client turns pill and tape on here', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = pausable();
    await pumpApp(tester, client: client);
    expect(find.byKey(tape), findsNothing);

    // Wie `humanitl intercept pause` an der Kommandozeile: Der Wechsel kommt
    // nur über den Ereignisstrom.
    await client.intercept(InterceptOp.pause);
    await tester.pumpAndSettle();
    expect(pillText(tester), 'Intercept PAUSED');
    expect(find.byKey(tape), findsOneWidget);
  });

  testWidgets('the pill is a button the keyboard reaches', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = pausable();
    await pumpApp(tester, client: client);
    final SemanticsHandle semantics = tester.ensureSemantics();
    expect(
      find.bySemanticsLabel('Intercept ON. Pause intercepting …'),
      findsOneWidget,
    );
    semantics.dispose();

    // Fokus auf die Pille, dann Enter: Die Rückfrage öffnet sich wie mit der
    // Maus, und das Abbrechen hat den Fokus.
    // Der Fokusknoten ist der von `HControl`, über dem Wort der Pille.
    Focus.of(
      tester.element(
        find.descendant(of: find.byKey(pill), matching: find.byType(Text)),
      ),
    ).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(find.byKey(modal), findsNothing);
    expect(client.interceptCalls, isNot(contains(InterceptOp.pause)));
  });

  // Review HUM-407: Der Herzschlag ist die Autorität. Er meldet eine Pause,
  // von der kein Ereignis kam; die Pille und das Band zeigen sie danach.
  // Mutationsbeweis: Nimmt `_heartbeat` den gemeldeten Stand nicht an,
  // bleibt die Pille ON.
  testWidgets('a heartbeat with a pause shows it without any event', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(
      infoOf(oldDaemon, InterceptStatus(instance: oldDaemon)),
    );
    await pumpApp(tester, client: client, heartbeat: beat);
    expect(pillText(tester), 'Intercept ON');

    client.current = infoOf(oldDaemon, pausedAt(1, oldDaemon));
    await nextBeat(tester);
    expect(pillText(tester), 'Intercept PAUSED');
    expect(find.byKey(tape), findsOneWidget);
  });

  // Review HUM-407: Ein neuer Daemon ersetzt mit dem Herzschlag alles auf
  // einmal, auch wenn seine Revision kleiner ist als die des alten.
  testWidgets('a heartbeat from a new daemon replaces the old state', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(
      infoOf(oldDaemon, InterceptStatus(revision: 4, instance: oldDaemon)),
    );
    await pumpApp(tester, client: client, heartbeat: beat);
    expect(pillText(tester), 'Intercept ON');

    client.current = infoOf(newDaemon, pausedAt(1, newDaemon));
    await nextBeat(tester);
    expect(pillText(tester), 'Intercept PAUSED');
    expect(find.byKey(tape), findsOneWidget);
  });

  // Review HUM-407: Eine Antwort eines anderen Daemons gilt nicht, auch mit
  // höherer Revision.
  // Mutationsbeweis: Prüft `_offer` die Instanz nicht, steht danach PAUSED.
  testWidgets('a reply from another daemon is ignored', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(
      infoOf(newDaemon, InterceptStatus(instance: newDaemon)),
    )..pauseReply = pausedAt(9, oldDaemon);
    await pumpApp(tester, client: client);
    await pauseThroughTheDialog(tester);
    expect(client.interceptCalls, contains(InterceptOp.pause));
    expect(pillText(tester), 'Intercept ON');
    expect(find.byKey(tape), findsNothing);
  });

  // Review HUM-407: Ein Ereignis ohne Instanz gilt nicht.
  testWidgets('an event without an instance is ignored', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(
      infoOf(newDaemon, InterceptStatus(instance: newDaemon)),
    );
    await pumpApp(tester, client: client);
    client.announceIntercept(pausedAt(5, ''));
    await tester.pumpAndSettle();
    expect(pillText(tester), 'Intercept ON');
    expect(find.byKey(tape), findsNothing);
  });

  // Review HUM-407: Eine Antwort des Daemons, den der letzte Herzschlag
  // nannte, wirkt sofort, ohne auf den nächsten zu warten.
  testWidgets('a reply from the current daemon applies between heartbeats', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(
      infoOf(newDaemon, InterceptStatus(instance: newDaemon)),
    )..pauseReply = pausedAt(1, newDaemon);
    await pumpApp(tester, client: client);
    await pauseThroughTheDialog(tester);
    expect(pillText(tester), 'Intercept PAUSED');
    expect(find.byKey(tape), findsOneWidget);
  });

  // Review HUM-407: Die Fähigkeit kommt mit dem Herzschlag, und erst dann
  // wird die Pille ein Knopf.
  testWidgets('a capability that appears with a heartbeat enables the pill', (
    WidgetTester tester,
  ) async {
    final DaemonInfo without = FakeDaemonClient.defaultInfo.copyWith(
      sessionId: newDaemon,
    );
    final _Beating client = _Beating(without);
    await pumpApp(tester, client: client, heartbeat: beat);
    expect(tester.widget<HBadge>(find.byKey(pill)).onTap, isNull);

    client.current = infoOf(newDaemon, InterceptStatus(instance: newDaemon));
    await nextBeat(tester);
    expect(tester.widget<HBadge>(find.byKey(pill)).onTap, isNotNull);
  });

  // Codex, Review HUM-407 Runde 6: Verliert der Daemon des Herzschlags die
  // Fähigkeit, ist er nicht pausiert, gleich welche Revision vorher galt.
  // Mutationsbeweis: Ohne `!_pausable` in `_heartbeat` bleibt das Band.
  testWidgets('a heartbeat without the capability clears a shown pause', (
    WidgetTester tester,
  ) async {
    final _Beating client = _Beating(infoOf(newDaemon, pausedAt(1, newDaemon)));
    await pumpApp(tester, client: client, heartbeat: beat);
    expect(pillText(tester), 'Intercept PAUSED');
    expect(find.byKey(tape), findsOneWidget);

    client.current = FakeDaemonClient.defaultInfo.copyWith(
      sessionId: newDaemon,
    );
    await nextBeat(tester);
    expect(pillText(tester), 'Intercept ON');
    expect(find.byKey(tape), findsNothing);
  });
}
