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

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_reveal.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';
import 'package:humanitl/features/intercept/widgets/diagnostic_card.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';
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

  /// `LLM_005` steht im Streifen als bernsteinfarbene Zeile (HUM-039).
  ///
  /// Der Befund entsteht, wenn eine durchgereichte Anfrage an das Sprachmodell
  /// Funde trägt: Sie ist schon gesendet, niemand hat über sie entschieden,
  /// und der Streifen ist der Ort, an dem der Mensch davon erfährt. Bernstein
  /// und nicht Rot: Rot heißt in diesem Programm „blockiert", und diese
  /// Anfrage war das Gegenteil (`docs/UX.md` Regel 6).
  testWidgets('llm_005_is_an_amber_line_in_the_strip', (
    WidgetTester tester,
  ) async {
    const String why =
        'this request to your language model at http://192.168.1.50:11434 '
        'contains 2 potential secret(s) or personal data';
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(code: 'LLM_005', severity: Severity.warning, why: why),
        flowId: const FlowId('01920000-0000-7000-8000-0000000000ff'),
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 200));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    expect(find.text('LLM_005'), findsOneWidget);
    expect(find.text(why), findsOneWidget);

    final HDiagnosticCard card = tester.widget<HDiagnosticCard>(
      find.byType(HDiagnosticCard),
    );
    final HTokens tokens = HTheme.of(
      tester.element(find.byType(HDiagnosticCard)),
    );
    expect(card.color, tokens.state.held, reason: 'amber, the held hue');
    expect(card.color, isNot(tokens.state.blocked));
  });

  /// Ein Befund an einem Flow führt zu ihm (HUM-039).
  ///
  /// Der Flow hinter `LLM_005` ist eine Durchreiche und steht in keiner
  /// Warteschlange. Die Karte hinterlässt deshalb eine Notiz für die
  /// Historie; rot, sobald der Knopf fehlt oder eine andere Id meldet.
  testWidgets('a_finding_on_a_flow_offers_to_open_it', (
    WidgetTester tester,
  ) async {
    // Ein Flow, den der Fake kennt: Ohne ihn scheiterte `GetFlow` still, das
    // Blatt ginge nie auf, und der Test prüfte nur die halbe Kette.
    final Flow recorded = heldFlow(
      n: 255,
      deadline: testStart.add(const Duration(minutes: 5)),
      host: '192.168.1.50',
      path: '/v1/chat/completions',
    );
    final FlowId passthrough = recorded.id;
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(
          code: 'LLM_005',
          severity: Severity.warning,
          why: 'this request contains 1 potential secret(s)',
        ),
        flowId: passthrough,
      ),
    );
    client.state.details[passthrough] = detailFor(recorded);
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 200));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    final Finder open = find.byKey(
      const ValueKey<String>('intercept-diagnostic-open-0'),
    );
    expect(open, findsOneWidget);
    expect(find.text('Open request'), findsOneWidget);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(DiagnosticCard)),
    );
    expect(container.read(flowRevealProvider), isNull);
    expect(container.read(navigationProvider), Section.intercept);

    await tester.tap(open);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    // Die ganze Kette, denn das Gerüst baut die ganze Anwendung: Die Shell
    // hat zur Historie gewechselt, die Historie hat genau diese Anfrage
    // ausgewählt, ihr Blatt aus `GetFlow` geöffnet und die Notiz gelöscht.
    expect(container.read(navigationProvider), Section.history);
    expect(container.read(historySelectionProvider), passthrough);
    expect(find.byType(HSheet), findsOneWidget);
    expect(container.read(flowRevealProvider), isNull);
  });

  /// Ein Befund der ganzen Sitzung hat keinen Flow und keinen Weg dorthin;
  /// rot, sobald der Knopf ohne `flowId` erscheint und ins Leere führt.
  testWidgets('a_finding_without_a_flow_has_no_way_to_one', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      diagnosticScript(
        const Diagnostic(
          code: 'TLS_003',
          severity: Severity.info,
          why: 'a client opened a tunnel without naming the host',
        ),
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(milliseconds: 200));
    await tester.pump(HMotion.arrive);
    await tester.pump();

    expect(find.text('TLS_003'), findsOneWidget);
    expect(
      find.byKey(const ValueKey<String>('intercept-diagnostic-open-0')),
      findsNothing,
    );
    expect(find.text('Open request'), findsNothing);
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

  // Der lange Satz des Daemons (HUM-150).
  //
  // Gemessen am 2026-09-13, ehe hier etwas geändert wurde: Die Schätzung von
  // `IntrinsicHeight` ist richtig — bei 280 und 560 Pixeln Breite und bei
  // Textskalierung 1 und 2 stimmt sie auf das Pixel mit der Höhe überein, die
  // die Zeile dann bekommt. Falsch war, was `IntrinsicHeight` mit der Zahl
  // tut: `BoxConstraints.tighten` klemmt sie in die Schranke des Elternteils.
  // In einer Kiste von 560 mal 200 Pixeln stand danach „A RenderFlex
  // overflowed by 420 pixels on the bottom", der Absatz war 520 Pixel hoch,
  // sichtbar blieben 116, und das `ClipRRect` der Karte schnitt den Rest ohne
  // Auslassungszeichen ab.
  //
  // **Dieser erste Test war auch mit dem alten Aufbau grün, in jeder
  // Zusicherung.** Im Streifen steht die Karte in einer Liste ohne
  // Höhenschranke, und dort hat die Klemme nie zugeschlagen; rot wird der
  // Aufbau erst in `a_card_with_too_little_room_scrolls_instead_of_cutting`.
  // Was dieser Test hält, ist der Satz selbst: Er ist vollständig, er ist
  // nicht gekürzt, und sein Ende ist zu erreichen.
  testWidgets('the_longest_tls_001_sentence_stands_whole_in_a_280_px_strip', (
    WidgetTester tester,
  ) async {
    await pumpStrip(tester, found: <SessionDiagnostic>[longestTlsFinding()]);
    expect(tester.takeException(), isNull);
    expect(tester.getSize(find.byType(DiagnosticStrip)).width, 280);

    // Der Absatz trägt den Satz ganz und ist so hoch, wie er sein muss.
    // Jede Zeile hier kann fallen: die erste, sobald jemand den Satz vor der
    // Karte kürzt; die zweite, sobald die Karte ein `maxLines` bekommt (ohne
    // `maxLines` kürzt ein `Text` nie, auch mit `TextOverflow` nicht, deshalb
    // steht hier `didExceedMaxLines` und nicht eine Aussage über das Kürzen);
    // die dritte, sobald eine Höhenschranke oder ein Schnitt den Absatz
    // kleiner macht, als sein Text braucht.
    final RenderParagraph why = tester.renderObject<RenderParagraph>(
      find.text(longestTlsWhy),
    );
    expect(why.text.toPlainText(), longestTlsWhy);
    expect(why.didExceedMaxLines, isFalse);
    expect(why.size.height, why.getMaxIntrinsicHeight(why.size.width));

    // Und der letzte Satz des Befundes steht auf dem Bildschirm, sobald der
    // Streifen dorthin gescrollt ist: ganz, innerhalb des Ausschnitts.
    final ScrollableState strip = stripScroll(tester);
    final Rect window = tester.getRect(find.byType(DiagnosticStrip));
    Rect words = boxOf(tester, longestTlsWhy, lastWords);
    strip.position.jumpTo(
      (strip.position.pixels + words.bottom - window.bottom + HSpace.x2).clamp(
        0,
        strip.position.maxScrollExtent,
      ),
    );
    await tester.pump();
    words = boxOf(tester, longestTlsWhy, lastWords);
    expect(words.top, greaterThanOrEqualTo(window.top));
    expect(words.bottom, lessThanOrEqualTo(window.bottom));
    expect(words.left, greaterThanOrEqualTo(window.left));
    expect(words.right, lessThanOrEqualTo(window.right));
  });

  testWidgets('a_card_with_too_little_room_scrolls_instead_of_cutting', (
    WidgetTester tester,
  ) async {
    // Die Karte in einer Kiste, die kleiner ist als ihr Satz: genau der Fall,
    // in dem die alte Karte den Satz stumm abschnitt. Rot mit dem alten
    // Aufbau, gleich dreifach: `A RenderFlex overflowed by ... pixels`, kein
    // `Scrollable` in der Karte, und der letzte Satz nirgends zu erreichen.
    await pumpCard(
      tester,
      const HDiagnosticCard(
        code: 'TLS_001',
        severityLabel: 'Warning',
        color: HColors.held,
        title: 'The daemon reports',
        why: longestTlsWhy,
      ),
      width: 280,
      height: interceptStripsMaxHeight,
    );
    expect(tester.takeException(), isNull);
    expect(
      tester.getSize(find.byType(HDiagnosticCard)),
      const Size(280, interceptStripsMaxHeight),
    );

    final RenderParagraph why = tester.renderObject<RenderParagraph>(
      find.text(longestTlsWhy),
    );
    expect(why.size.height, why.getMaxIntrinsicHeight(why.size.width));

    final ScrollableState inside = tester.state<ScrollableState>(
      find.descendant(
        of: find.byType(HDiagnosticCard),
        matching: find.byType(Scrollable),
      ),
    );
    expect(inside.position.maxScrollExtent, greaterThan(0));
    final Rect card = tester.getRect(find.byType(HDiagnosticCard));
    Rect words = boxOf(tester, longestTlsWhy, lastWords);
    inside.position.jumpTo(
      (inside.position.pixels + words.bottom - card.bottom + HSpace.x2).clamp(
        0,
        inside.position.maxScrollExtent,
      ),
    );
    await tester.pump();
    words = boxOf(tester, longestTlsWhy, lastWords);
    expect(words.top, greaterThanOrEqualTo(card.top));
    expect(words.bottom, lessThanOrEqualTo(card.bottom));
  });

  testWidgets('the_strip_shows_that_something_stands_below_its_edge', (
    WidgetTester tester,
  ) async {
    // Der Streifen scrollte auch vorher; nur sagte das Bild es nicht. Rot,
    // sobald der Balken fällt oder nur beim Scrollen erscheint.
    await pumpStrip(tester, found: <SessionDiagnostic>[longestTlsFinding()]);
    final Finder bar = find.descendant(
      of: find.byType(DiagnosticStrip),
      matching: find.byType(RawScrollbar),
    );
    expect(bar, findsOneWidget);
    expect(tester.widget<RawScrollbar>(bar).thumbVisibility, isTrue);
    expect(stripScroll(tester).position.maxScrollExtent, greaterThan(0));
  });

  testWidgets('on_linux_the_strip_carries_exactly_one_bar', (
    WidgetTester tester,
  ) async {
    // Die ausgelieferte Plattform, und die einzige, auf der dieser Defekt zu
    // sehen ist: `ScrollBehavior.buildScrollbar` hängt auf Linux, macOS und
    // Windows über **jedes** `Scrollable` einen eigenen `RawScrollbar`, weil
    // `app.dart` `WidgetsApp` ohne `scrollBehavior` baut. `flutter test` läuft
    // sonst als `TargetPlatform.android`, wo die Vorgabe keinen Balken baut;
    // ohne diese Variante kann kein Test dieses Verzeichnisses den Fehler je
    // sehen. Rot mit zwei Balken, sobald das `ScrollConfiguration` im Streifen
    // fällt. Nicht rot, wenn die Karte wieder in jeder Lage eine eigene
    // Ansicht mitbringt: Dasselbe `ScrollConfiguration` nimmt auch dieser den
    // Balken der Plattform ab; dagegen stehen
    // `a_card_with_room_enough_brings_no_bar_of_its_own` und
    // `and_the_page_key_still_reaches_the_strip`.
    await pumpStrip(tester, found: <SessionDiagnostic>[longestTlsFinding()]);
    final Finder bars = find.descendant(
      of: find.byType(DiagnosticStrip),
      matching: find.byType(RawScrollbar),
    );
    expect(bars, findsOneWidget);
    final RawScrollbar bar = tester.widget<RawScrollbar>(bars);
    expect(bar.thumbVisibility, isTrue);
    expect(bar.thickness, interceptStripScrollbarWidth);
    expect(bar.thumbColor, HTokens.dark.colors.fg2);
    // Der Balken ist die einzige Auskunft darüber, dass unter der Kante noch
    // etwas steht, also hält er die 3:1 für nicht-textliche Auskünfte, auf
    // jeder Fläche, auf der er liegen kann, in beiden Themes.
    for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
      for (final Color surface in <Color>[
        tokens.colors.bg1,
        tokens.colors.bg2,
      ]) {
        expect(
          HColorDerivation.contrast(tokens.colors.fg2, surface),
          greaterThanOrEqualTo(3),
        );
      }
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('and_the_page_key_still_reaches_the_strip', (
    WidgetTester tester,
  ) async {
    // `ScrollAction` nimmt zu „Bild ab" das innerste `Scrollable` vom Fokus
    // aus, verbraucht die Taste und bewegt nichts, wenn dieses nichts zu
    // scrollen hat. Eine Ansicht in jeder Karte machte die Taste damit tot
    // (`docs/UX.md` 5.3: Jede belegte Taste tut etwas). Gemessen mit einer
    // Ansicht in der Karte: Der Fokus im Kartenrahmen findet als nächstes
    // `Scrollable` eines mit `maxScrollExtent` 0.0, und der Streifen bleibt
    // nach „Bild ab" auf 1486.0 stehen; ohne sie findet er den Streifen mit
    // 1680.0 und fährt auf 1680.0. Rot, sobald die Karte ihre Ansicht wieder
    // ohne Höhenschranke baut.
    await pumpStrip(
      tester,
      found: <SessionDiagnostic>[longestTlsFinding()],
      scale: 1,
    );
    expect(
      find.descendant(
        of: find.byType(HDiagnosticCard),
        matching: find.byType(Scrollable),
      ),
      findsNothing,
    );
    final ScrollableState strip = stripScroll(tester);
    expect(strip.position.maxScrollExtent, greaterThan(0));

    // Der erste Halt der Tabulatorkette liegt auf dem Ausblenden-Knopf, der
    // neben dem Kartenrahmen steht; der zweite steht im Rahmen selbst, und
    // nur dort greift der Defekt.
    Element focused = tester.element(find.byType(DiagnosticStrip));
    for (int stop = 0; stop < 8; stop++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
      focused = primaryFocus!.context! as Element;
      if (find
          .descendant(
            of: find.byType(HDiagnosticCard),
            matching: find.byWidget(focused.widget),
          )
          .evaluate()
          .isNotEmpty) {
        break;
      }
    }
    expect(
      find.descendant(
        of: find.byType(HDiagnosticCard),
        matching: find.byWidget(focused.widget),
      ),
      findsOneWidget,
    );
    // Das nächste `Scrollable` vom Fokus aus ist der Streifen und nicht eine
    // taube Ansicht in der Karte: genau die Frage, die `ScrollAction` stellt.
    expect(
      Scrollable.maybeOf(focused)?.position.maxScrollExtent,
      strip.position.maxScrollExtent,
    );

    // `ScrollAction` fährt die Strecke in 100 ms, nicht in einem Sprung. Der
    // Vergleich geht gegen den Stand vor der Taste: Der Fokuswechsel selbst
    // hat den Streifen schon bewegt.
    final double before = strip.position.pixels;
    await tester.sendKeyEvent(LogicalKeyboardKey.pageDown);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(strip.position.pixels, greaterThan(before));
    final double down = strip.position.pixels;
    await tester.sendKeyEvent(LogicalKeyboardKey.pageUp);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(strip.position.pixels, lessThan(down));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a_card_with_room_enough_brings_no_bar_of_its_own', (
    WidgetTester tester,
  ) async {
    // Dieselbe Karte wird an achtzehn Stellen gebaut. Wo ihr Elternteil keine
    // Höhenschranke setzt — der Streifen, eine Spalte, ein Blatt —, bringt sie
    // gar keine Ansicht mit, die scrollen könnte, und damit auch keinen
    // Balken, den `ScrollBehavior.buildScrollbar` auf Linux über jedes
    // `Scrollable` hängt. Rot mit einer Ansicht in jeder Lage: dann steht hier
    // ein `Scrollable` und darüber ein zweiter Balken.
    await pumpCard(
      tester,
      const HDiagnosticCard(
        code: 'TLS_001',
        severityLabel: 'Warning',
        color: HColors.held,
        title: 'The daemon reports',
        why: scenarioWhy,
      ),
      width: 280,
      height: null,
    );
    expect(tester.takeException(), isNull);
    expect(
      find.descendant(
        of: find.byType(HDiagnosticCard),
        matching: find.byType(Scrollable),
      ),
      findsNothing,
    );
    expect(
      find.descendant(
        of: find.byType(HDiagnosticCard),
        matching: find.byType(RawScrollbar),
      ),
      findsNothing,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('and_where_everything_fits_there_is_nothing_to_scroll', (
    WidgetTester tester,
  ) async {
    // Geprüft wird die Bedingung, an der `ScrollbarPainter` den Daumen malt
    // (`_needPaint`: `maxScrollExtent - minScrollExtent > 0`), nicht das Bild:
    // Ein kurzer Befund unter dem Deckel hat nichts zu scrollen, und der
    // Streifen bleibt so leer wie vorher.
    await pumpStrip(
      tester,
      found: <SessionDiagnostic>[
        SessionDiagnostic(
          id: 1,
          at: testStart,
          diagnostic: const Diagnostic(
            code: 'TLS_003',
            severity: Severity.info,
            why: 'a handshake arrived without SNI',
          ),
        ),
      ],
      scale: 1,
    );
    expect(stripScroll(tester).position.maxScrollExtent, 0);
  });
}

/// Die letzten Worte des Befundes.
///
/// Das Ende von [lastSentence]. Der ganze Satz mit dem Zähler ist bei 280
/// Pixeln Breite und Textskalierung 2 höher als der Ausschnitt des Streifens;
/// auf dem Bildschirm gesucht wird deshalb sein Ende.
const String lastWords = 'since the previous card, this one included.';

/// Der letzte Satz des längsten Befundes, der Satz mit dem Zähler.
const String lastSentence =
    '7 failed TLS connections have been counted for this host and this tool '
    'hint since the previous card, this one included.';

/// Der längste `why`-Satz, den `TLS_001` heute schicken kann.
///
/// Zusammengesetzt wie in `daemon/crates/proxy/src/tls_observe.rs`
/// (`rejected_ca`): der längere der beiden Anfänge (der Tunnel, der nach dem
/// Handschlag leer geschlossen wurde), der Absatz zur schon gesetzten
/// Variablen, die Anmerkung zu curl (`ToolHint::note`) und der Satz mit dem
/// Wiederholungszähler (`repeat_sentence`). Wörtlich und nicht gekürzt: Der
/// Satz gehört dem Daemon, und die Karte zeichnet ihn, wie er ist
/// (`docs/UX.md` 4.4).
const String longestTlsWhy =
    'A client that calls itself curl inside the sandbox finished the TLS '
    'handshake for api.github.com and then closed the connection without '
    "sending a request. Most clients that do this do not trust Humanitl's "
    'certificate: they check it only after the handshake. Nothing left the '
    'sandbox. Humanitl already sets CURL_CA_BUNDLE=/etc/humanitl/ca.crt in '
    'the sandbox, so this is not a missing variable: the client overrode it '
    'on its command line, keeps its own certificate pool, pins a certificate, '
    'or does not read CURL_CA_BUNDLE at all. Setting CURL_CA_BUNDLE under '
    '[sandbox.env] in config.toml helps only where the sandbox profile in use '
    'does not carry the variable, and a profile that sets sandbox.env of its '
    "own replaces that table from config.toml. curl's --cacert and --capath "
    'override CURL_CA_BUNDLE, and --insecure skips the check altogether. '
    '$lastSentence';

/// Der Befund zu [longestTlsWhy], mit Vorschlag und mit Fluss: beide Knöpfe,
/// also die höchste Karte, die dieser Code baut.
SessionDiagnostic longestTlsFinding() => SessionDiagnostic(
  id: 0,
  at: testStart,
  flowId: const FlowId('018f0001-0000-7000-8000-000000060000'),
  diagnostic: const Diagnostic(
    code: 'TLS_001',
    severity: Severity.warning,
    why: longestTlsWhy,
    fix: FixAction.setEnv(key: 'CURL_CA_BUNDLE', value: sandboxCaPath),
  ),
);

/// Ein Streifen, der nicht am Ereignisstrom hängt.
class FixedDiagnostics extends Diagnostics {
  /// Hält [found].
  FixedDiagnostics(this.found);

  /// Die Befunde des Tests.
  final List<SessionDiagnostic> found;

  @override
  List<SessionDiagnostic> build() => found;
}

/// Die Liste des Streifens.
///
/// `.first` aus Vorsicht und nicht, weil es mehrere gäbe: Im Streifen hat
/// heute keine Karte eine eigene Ansicht — zwei Tests dieser Datei halten das
/// fest —, aber eine Karte unter einer Höhenschranke brächte eine mit, und
/// die äußere ist dann immer noch die Liste des Streifens.
ScrollableState stripScroll(WidgetTester tester) =>
    tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byType(DiagnosticStrip),
            matching: find.byType(Scrollable),
          )
          .first,
    );

/// Das Rechteck, in dem [part] von [text] steht, in globalen Koordinaten.
Rect boxOf(WidgetTester tester, String text, String part) {
  final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
    find.text(text),
  );
  final int start = text.indexOf(part);
  expect(start, isNonNegative);
  final List<TextBox> boxes = paragraph.getBoxesForSelection(
    TextSelection(baseOffset: start, extentOffset: start + part.length),
  );
  expect(boxes, isNotEmpty);
  final Offset origin = paragraph.localToGlobal(Offset.zero);
  Rect box = boxes.first.toRect();
  for (final TextBox next in boxes.skip(1)) {
    box = box.expandToInclude(next.toRect());
  }
  return box.shift(origin);
}

/// Ein Wirt mit Theme, Sprache und Overlay, wie ihn die Anwendung mitbringt.
Widget stripHost(Widget child, {required double scale}) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  locale: const Locale('en'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  onGenerateTitle: (BuildContext context) => 'diagnostic strip',
  builder: (BuildContext context, Widget? _) => MediaQuery(
    data: MediaQueryData(textScaler: TextScaler.linear(scale)),
    child: HTheme(
      tokens: HTokens.dark,
      child: Overlay(
        initialEntries: <OverlayEntry>[
          OverlayEntry(
            // Ein Bereich, der den Fokus annimmt: ohne ihn hat `Tab` keinen
            // Ausgangspunkt, und die Anwendung hat ihn über ihre Route.
            builder: (BuildContext context) => FocusScope(
              autofocus: true,
              child: Align(alignment: Alignment.topLeft, child: child),
            ),
          ),
        ],
      ),
    ),
  ),
);

/// Der Streifen allein, [width] Pixel breit, unter dem Deckel des
/// Warteschlangen-Panes.
///
/// Dieselbe Verschachtelung wie in `queue_pane.dart`: ein `ConstrainedBox` mit
/// [interceptStripsMaxHeight] und der Streifen als einziges `Flexible`. Der
/// Streifen steht hier allein, weil die Akzeptanz von HUM-150 eine Breite von
/// 280 Pixeln nennt; über die ganze Anwendung hinge die Breite am Splitter.
Future<void> pumpStrip(
  WidgetTester tester, {
  required List<SessionDiagnostic> found,
  double width = 280,
  double scale = 2,
}) async {
  await tester.binding.setSurfaceSize(Size(width + HSpace.x6, 900));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        diagnosticsProvider.overrideWith(() => FixedDiagnostics(found)),
      ],
      child: stripHost(
        SizedBox(
          width: width,
          child: ConstrainedBox(
            constraints: const BoxConstraints(
              maxHeight: interceptStripsMaxHeight,
            ),
            child: const Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[Flexible(child: DiagnosticStrip())],
            ),
          ),
        ),
        scale: scale,
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}

/// Eine Karte allein, in einer Kiste von [width] mal [height]; ohne [height]
/// in einer Spalte, also ohne Schranke nach unten.
Future<void> pumpCard(
  WidgetTester tester,
  Widget card, {
  required double width,
  required double? height,
  double scale = 2,
}) async {
  await tester.binding.setSurfaceSize(
    Size(width + HSpace.x6, (height ?? 600) + 200),
  );
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      child: stripHost(
        // Ohne [height] steht die Karte in einer Spalte und bekommt keine
        // Schranke nach unten, so wie an den meisten ihrer Stellen.
        height == null
            ? SizedBox(
                width: width,
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[card],
                ),
              )
            : SizedBox(width: width, height: height, child: card),
        scale: scale,
      ),
    ),
  );
  await tester.pump();
  await tester.pump();
}
