// Die Karte, mit der der Daemon einen Befund meldet (HUM-045, HUM-106).
//
// Bis zu diesem Issue sah ein Mensch eine Anfrage scheitern und erfuhr nicht,
// warum, obwohl der Daemon es wusste und sogar sagen konnte, was zu tun ist.
// Die Tests hier prüfen deshalb nicht nur, dass die Karte erscheint, sondern
// auch, dass der Satz auf ihr der des Daemons ist, dass der angebotene Fix
// wirklich etwas tut und dass die Karte nichts zeichnet, was sie nicht hat.
//
// Jede Zusicherung, die eine Schutzmaßnahme prüft, nennt in ihrem Kommentar
// die Änderung, die sie rot macht; die Proben sind von Hand gefahren worden.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';
import 'package:humanitl/features/intercept/widgets/diagnostic_card.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';

import 'harness.dart';

/// Der Satz, den das Standard-Szenario des Fakes mit `TLS_001` schickt.
const String scenarioWhy =
    'curl in the sandbox does not trust the Humanitl CA yet';

/// Ein Skript mit genau einem Befund nach 100 ms.
List<ScriptedEvent> diagnosticScript(
  Diagnostic diagnostic, {
  FlowId? flowId,
}) => <ScriptedEvent>[
  ScriptedEvent(
    const Duration(milliseconds: 100),
    (FakeSessionState state, DateTime now) =>
        FlowEvent.diagnostic(at: now, diagnostic: diagnostic, flowId: flowId),
  ),
];

/// Fängt ab, was in die Zwischenablage geschrieben wird.
List<String> captureClipboard(WidgetTester tester) {
  final List<String> written = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (MethodCall call) async {
      if (call.method == 'Clipboard.setData') {
        written.add(
          (call.arguments as Map<Object?, Object?>)['text']! as String,
        );
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return written;
}

void main() {
  testWidgets('tls_card_shows_code_why_and_export_command', (
    WidgetTester tester,
  ) async {
    final List<String> clipboard = captureClipboard(tester);
    // Das Standard-Szenario des Fakes: nach 5 s genau ein `TLS_001`.
    final FakeDaemonClient client = fakeDaemon();
    await pumpIntercept(tester, client: client);
    expect(find.byType(DiagnosticCard), findsNothing);

    await playScript(tester, const Duration(seconds: 6));
    // Das Einblenden abwarten, damit der Kopierknopf greifbar ist.
    await tester.pump(HMotion.arrive);
    await tester.pump();

    expect(find.byType(DiagnosticCard), findsOneWidget);
    expect(find.text('TLS_001'), findsOneWidget);
    // Der Titel ist der Rahmen der Anwendung, der Satz gehört dem Daemon
    // (`docs/UX.md` 4.4). Rot, sobald jemand den Satz umformuliert oder
    // übersetzt.
    expect(find.text('The daemon reports'), findsOneWidget);
    expect(find.text(scenarioWhy), findsOneWidget);
    expect(find.text('Set CURL_CA_BUNDLE'), findsOneWidget);
    expect(
      find.text('export CURL_CA_BUNDLE=/etc/humanitl/ca.crt'),
      findsOneWidget,
    );

    await tester.tap(
      find.byKey(const ValueKey<String>('intercept-diagnostic-copy-0')),
    );
    await tester.pump();
    expect(clipboard, <String>['export CURL_CA_BUNDLE=/etc/humanitl/ca.crt']);
    // Das Rückmeldefenster ablaufen lassen, sonst bleibt ein Timer offen.
    await tester.pump(HMotion.copyFeedback);
    await tester.pump();
  });

  testWidgets('dismiss_hides_the_card', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(
          code: 'TLS_001',
          severity: Severity.warning,
          why: 'the first one',
          fix: FixAction.setEnv(
            key: 'CURL_CA_BUNDLE',
            value: '/etc/humanitl/ca.crt',
          ),
        ),
        flowId: const FlowId('018f0001-0000-7000-8000-000000060000'),
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await tester.pump(HMotion.arrive);
    await tester.pump();
    expect(find.byType(DiagnosticCard), findsOneWidget);

    await tester.tap(
      find.byKey(const ValueKey<String>('intercept-diagnostic-dismiss-0')),
    );
    await tester.pump();

    expect(find.byType(DiagnosticCard), findsNothing);
    // Der Streifen zeichnet gar nichts mehr, statt eine leere Zeile zu lassen.
    expect(find.byType(HDiagnosticCard), findsNothing);
  });

  testWidgets('a_diagnostic_without_fix_has_no_fix_row', (
    WidgetTester tester,
  ) async {
    // `TLS_003`: ein Handschlag ohne SNI. Er trägt keine Flusskennung und
    // keinen Vorschlag, und er darf trotzdem nicht verschwinden.
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(
          code: 'TLS_003',
          severity: Severity.info,
          why: 'a handshake arrived without SNI; there is no name to decide on',
        ),
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await tester.pump(HMotion.arrive);
    await tester.pump();

    expect(find.byType(DiagnosticCard), findsOneWidget);
    expect(find.text('TLS_003'), findsOneWidget);
    expect(
      find.text(
        'a handshake arrived without SNI; there is no name to decide on',
      ),
      findsOneWidget,
    );
    // Kein leerer Slot: Ohne Vorschlag steht dort kein Control.
    // Rot, sobald die Karte `FixControl(fix: null)` immer einhängt.
    expect(
      find.descendant(
        of: find.byType(DiagnosticCard),
        matching: find.byType(FixControl),
      ),
      findsNothing,
    );
  });

  testWidgets('the_daemons_sentence_is_plain_text', (
    WidgetTester tester,
  ) async {
    // Im Satz steckt Material von außen: der Hostname aus dem Netz. Er wird
    // als reiner Text gezeichnet, nie als Markdown, nie als Verweis, nie in
    // einer Spanne mit Erkenner. Rot, sobald jemand `Text.rich` daraus macht.
    const String why =
        'handshake with **evil**.example failed: see [here](https://evil.io)';
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(code: 'TLS_001', severity: Severity.warning, why: why),
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await tester.pump(HMotion.arrive);
    await tester.pump();

    final Finder drawn = find.text(why);
    expect(drawn, findsOneWidget);
    final Text text = tester.widget<Text>(drawn);
    expect(text.data, why);
    expect(text.textSpan, isNull);
    final RichText painted = tester.widget<RichText>(
      find.descendant(of: drawn, matching: find.byType(RichText)),
    );
    final InlineSpan span = painted.text;
    expect(span, isA<TextSpan>());
    expect((span as TextSpan).recognizer, isNull);
    expect(span.children, isNull);
    expect(span.toPlainText(), why);
  });

  testWidgets('the_strip_builds_only_what_fits', (WidgetTester tester) async {
    // `LLM_005`, `PROXY_002` und `PROXY_005` entstoert der Daemon nicht, und
    // der Agent loest sie selbst aus. Der Streifen darf deshalb weder alles
    // halten noch alles bauen noch sich etwas je Befund merken.
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      for (int i = 0; i < 600; i++)
        ScriptedEvent(
          Duration(milliseconds: 100 + i),
          (FakeSessionState state, DateTime now) => FlowEvent.diagnostic(
            at: now,
            diagnostic: Diagnostic(
              code: 'LLM_005',
              severity: Severity.warning,
              why: 'finding number $i',
            ),
          ),
        ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 2));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    // Literale, nicht `maxSessionDiagnostics`: Eine Zusicherung gegen die
    // Konstante, die sie prueft, bleibt gruen, wenn jemand sie auf 5000 setzt.
    final ProviderContainer container = containerOf(tester);
    expect(container.read(diagnosticsProvider), hasLength(200));
    expect(container.read(diagnosticsProvider.notifier).dropped, 400);
    // Gebaut sind nur die Karten des Ausschnitts plus der Vorrat des
    // Scrollbereichs, nie die zweihundert des Puffers.
    // Rot, sobald `ListView.builder` wieder eine `Column` wird.
    expect(
      tester.widgetList<DiagnosticCard>(find.byType(DiagnosticCard)).length,
      lessThan(20),
    );
    // Und die Menge der offenen Ankuenfte waechst nicht mit dem Strom: Eine
    // Id, deren Karte nie gebaut wurde, wird nur ueber diese Liste wieder
    // los. Rot, sobald `_fresh.retainWhere` faellt — dann liegen hier ueber
    // 580 Ids, und der gebaute-Karten-Test daneben bliebe gruen.
    final DiagnosticStripState strip = tester.state<DiagnosticStripState>(
      find.byType(DiagnosticStrip),
    );
    expect(strip.pendingArrivals, lessThanOrEqualTo(200));
  });

  testWidgets('the_dropped_line_survives_dismissing_every_card', (
    WidgetTester tester,
  ) async {
    // Wer alles weggeklickt hat, hat den Verlust nicht gesehen. Ihn dann
    // verschwinden zu lassen waere wieder stilles Wegwerfen.
    // Rot, sobald die Bedingung wieder nur auf `found.isEmpty` steht.
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      for (int i = 0; i < 205; i++)
        ScriptedEvent(
          Duration(milliseconds: 100 + i),
          (FakeSessionState state, DateTime now) => FlowEvent.diagnostic(
            at: now,
            diagnostic: Diagnostic(
              code: 'LLM_005',
              severity: Severity.warning,
              why: 'finding number $i',
            ),
          ),
        ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    final ProviderContainer container = containerOf(tester);
    expect(container.read(diagnosticsProvider.notifier).dropped, 5);
    expect(
      find.byKey(const Key('intercept-diagnostic-dropped')),
      findsOneWidget,
    );

    // Alle Karten ausblenden.
    final Diagnostics notifier = container.read(diagnosticsProvider.notifier);
    for (final SessionDiagnostic entry in <SessionDiagnostic>[
      ...container.read(diagnosticsProvider),
    ]) {
      notifier.dismiss(entry.id);
    }
    await tester.pump();

    expect(container.read(diagnosticsProvider), isEmpty);
    expect(find.byType(DiagnosticCard), findsNothing);
    expect(
      find.byKey(const Key('intercept-diagnostic-dropped')),
      findsOneWidget,
    );
  });

  testWidgets('the_two_strips_share_one_budget', (WidgetTester tester) async {
    // Bitte des Agenten und Befund treten zusammen auf. Mit je eigener
    // Schranke schoeben sie die Warteschlange fast vom Schirm.
    // Rot, sobald der Streifen sein Budget wieder allein nimmt.
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      for (int i = 0; i < 5; i++)
        ScriptedEvent(
          Duration(milliseconds: 100 + i),
          (FakeSessionState state, DateTime now) => FlowEvent.agentAsk(
            at: now,
            askId: 'ask-$i',
            text: 'bitte etwas freischalten, Nummer $i',
          ),
        ),
      for (int i = 0; i < 5; i++)
        ScriptedEvent(
          Duration(milliseconds: 200 + i),
          (FakeSessionState state, DateTime now) => FlowEvent.diagnostic(
            at: now,
            diagnostic: Diagnostic(
              code: 'LLM_005',
              severity: Severity.warning,
              why: 'finding number $i',
              fix: const FixAction.setEnv(
                key: 'CURL_CA_BUNDLE',
                value: '/etc/humanitl/ca.crt',
              ),
            ),
          ),
        ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    expect(find.byType(AgentAskStrip), findsOneWidget);
    expect(find.byType(DiagnosticStrip), findsOneWidget);
    final double asks = tester.getSize(find.byType(AgentAskStrip)).height;
    final double found = tester.getSize(find.byType(DiagnosticStrip)).height;
    expect(asks, greaterThan(0));
    expect(found, greaterThan(0));
    // Literal, nicht `interceptStripsMaxHeight`: sonst bliebe der Test gruen,
    // wenn jemand die Konstante auf 4200 setzt.
    expect(interceptStripsMaxHeight, 420);
    expect(asks + found, lessThanOrEqualTo(420));
    // Und der Platz, den eine kurze Bitte uebrig laesst, verfaellt nicht: Der
    // Befund-Streifen ist das einzige `Flexible` und nimmt den Rest.
    expect(asks + found, greaterThan(420 - agentAskMaxHeight));
  });

  testWidgets('a_card_fades_in_once, not on every scroll', (
    WidgetTester tester,
  ) async {
    // Eine Karte, die beim Scrollen wieder in den Ausschnitt kommt, ist keine
    // Ankunft; nichts bewegt sich unter einem lesenden Auge
    // (`docs/UX.md` 2.8).
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      for (int i = 0; i < 12; i++)
        ScriptedEvent(
          Duration(milliseconds: 100 + i),
          (FakeSessionState state, DateTime now) => FlowEvent.diagnostic(
            at: now,
            diagnostic: Diagnostic(
              code: 'TLS_003',
              severity: Severity.info,
              why: 'finding number $i',
            ),
          ),
        ),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 150));

    // Waehrend der Ankunft blendet die Karte wirklich ein. Ohne diese
    // Zusicherung bliebe der Test gruen, wenn der Wrapper zu
    // `return widget.child` wird: Dann gaebe es null Elemente, und die
    // Schleife darunter liefe leer durch.
    final Finder fading = find.descendant(
      of: find.byType(DiagnosticStrip),
      matching: find.byType(FadeTransition),
    );
    await tester.pump(const Duration(milliseconds: 60));
    expect(fading, findsWidgets);
    for (final FadeTransition fade in tester.widgetList<FadeTransition>(
      fading,
    )) {
      expect(fade.opacity.value, lessThan(1.0));
    }

    // Danach verlaesst der Wrapper den Baum (`docs/UX.md` 7).
    // Rot, sobald er stehen bleibt.
    await tester.pumpAndSettle();
    expect(fading, findsNothing);

    // Herunterscrollen und wieder herauf: keine zweite Einblendung.
    await tester.drag(find.byType(DiagnosticStrip), const Offset(0, -200));
    await tester.pump();
    expect(fading, findsNothing);
    await tester.drag(find.byType(DiagnosticStrip), const Offset(0, 200));
    await tester.pump();
    expect(fading, findsNothing);
  });

  testWidgets('the_fade_keeps_its_time_when_animations_are_off', (
    WidgetTester tester,
  ) async {
    // Der Linux-Embedder meldet `disableAnimations`; die Vorgabe skalierte die
    // 180 ms auf neun. `docs/UX.md` 2.10: Unter reduzierter Bewegung faellt
    // der Weg weg, nicht die Rueckmeldung — das Ein- und Ausblenden behaelt
    // seine volle Dauer. Rot, sobald `AnimationBehavior.preserve` faellt: dann
    // ist das Einblenden nach 60 ms schon vorbei und der Wrapper weg.
    //
    // Der Schalter ist die Barrierefreiheit der Plattform, nicht die
    // `MediaQuery`: `AnimationController` liest
    // `SemanticsBinding.instance.disableAnimations` (wie
    // `packages/ui/test/widgets_test.dart` fuer `HPill`).
    tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(
      tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(
          code: 'TLS_003',
          severity: Severity.info,
          why: 'a handshake arrived without SNI',
        ),
      ),
    );
    await pumpIntercept(tester, client: client, disableAnimations: true);
    await playScript(tester, const Duration(milliseconds: 150));

    final Finder fading = find.descendant(
      of: find.byType(DiagnosticStrip),
      matching: find.byType(FadeTransition),
    );
    await tester.pump(const Duration(milliseconds: 60));
    expect(fading, findsOneWidget);
    expect(tester.widget<FadeTransition>(fading).opacity.value, lessThan(1.0));
    await tester.pumpAndSettle();
  });
}
