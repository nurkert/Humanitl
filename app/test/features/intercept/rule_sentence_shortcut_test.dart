// Ein Tastenkürzel speichert keine Regel, deren Satz nicht dastand (HUM-249).
//
// Die Leiste zeigte früher den Satz der Aktion, nach der der Zeiger oder der
// Fokus gerade griff. Ein Kürzel fragt beide nicht: Mit der Maus über
// „Blockieren" speicherte `A` eine Allow-Regel, die niemand gelesen hatte, und
// mit dem Fokus auf dem Ventil speicherte `Ctrl+L` eine Block-Regel ebenso.
// (`B` allein gibt ein fokussiertes Control an sich selbst weiter,
// `docs/UX.md` 5.2; der Akkord wirkt überall.) Beide
// Tests halten fest, was unmittelbar vor dem Druck auf dem Schirm stand, und
// prüfen danach, dass der Satz der gespeicherten Regel darunter war.
//
// Mutationsbeweis: `_FirstLine` in `action_bar.dart` zeigt wieder nur den Satz
// der hervorgehobenen Aktion. Dann wird der erste Test rot (über „Blockieren"
// steht nur „block …", gespeichert wird „allow …"), und der zweite ebenso (am
// Ventil steht nur „allow …", gespeichert wird „block …"). Ebenso rot wird
// es, wenn die zweite Zeile die Aktion der ersten wiederholt.
//
// Dazu zwei Fälle aus dem Review: Ein Satz zählt nur, wenn er ganz dasteht,
// auch bei 480 px Breite, doppelter Schrift und deutschem Text; und das
// Halten des Ventils bei geschlossenem Raster, das eine Sitzungsregel
// anlegt, zeigt deren Satz, solange der Finger unten ist.

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/text/rule_sentence.dart' as sentence;
import 'package:humanitl/core/ui/ui.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/ui_state.dart';
import 'fixtures.dart';
import 'harness.dart';

final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

/// Jeder Text, der gerade ganz auf dem Schirm steht.
///
/// Gelesen wird, was gezeichnet ist: jeder `RenderParagraph`, gleich ob er
/// aus `Text`, `Text.rich` oder `RichText` kommt. Ein Absatz, der an seiner
/// Zeilengrenze gekürzt wurde, zählt nicht; sein Ende hat niemand gesehen.
Set<String> onScreen(WidgetTester tester) => <String>{
  for (final RenderParagraph paragraph in tester.renderObjectList(
    find.byType(RichText),
  ))
    if (!paragraph.didExceedMaxLines) paragraph.text.toPlainText(),
};

/// Der Satz, mit dem der Regel-Bildschirm [rule] vorliest; derselbe
/// Generator, aus dem die Leiste ihre Sätze baut.
String sentenceOf(Rule rule, [AppLocalizations? l10n]) =>
    sentence.ruleSentence(rule, l10n ?? en, now: testStart);

/// Ein offenes Raster: eine Sitzung, über die registrierbare Domain.
class ApexSessionDraft extends RememberDraft {
  @override
  RememberState build() =>
      const RememberState(open: true, target: RememberTarget.apex);
}

/// Öffnet das Raster mit der Gültigkeit Session: `2` (`docs/UX.md` 5.1).
Future<void> openSessionGrid(WidgetTester tester) async {
  await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
  await tester.pump();
}

/// Die eine Entscheidung, die der Fake empfing, und die Regel daran.
Rule savedRule(FakeDaemonClient client, RuleAction action) {
  expect(client.decisions, hasLength(1));
  final RecordedDecision decided = client.decisions.single;
  final Rule? rule = decided.remember;
  expect(rule, isNotNull, reason: 'the grid remembers, so a rule is saved');
  expect(rule!.action, action);
  return rule;
}

/// Gibt dem Ventil den Tastaturfokus, wie es Tab täte.
void focusValve(WidgetTester tester) {
  final Finder focus = find
      .descendant(
        of: find.byKey(const Key('intercept-allow')),
        matching: find.byType(Focus),
      )
      .first;
  Element? below;
  tester.element(focus).visitChildElements((Element child) => below = child);
  Focus.of(below!, createDependency: false).requestFocus();
}

void main() {
  testWidgets(
    'hover on Block, then A: the allow sentence stood before saving',
    (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openSessionGrid(tester);

      await hoverOver(tester, find.byKey(const Key('intercept-block')));
      // Der Zeiger steht über „Blockieren": Die Notiz daneben deckt sich auf.
      expect(
        tester
            .widget<Visibility>(
              find
                  .ancestor(
                    of: find.byKey(const Key('intercept-note-open')),
                    matching: find.byType(Visibility),
                  )
                  .first,
            )
            .visible,
        isTrue,
      );
      final Set<String> before = onScreen(tester);
      // Die Hervorhebung stand: Der Satz für Blockieren ist zu lesen.
      expect(before, contains('block · ∗ · registry.npmjs.org · this session'));

      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pump();
      await tester.pump();

      final Rule rule = savedRule(client, RuleAction.allow);
      expect(
        before,
        contains(sentenceOf(rule)),
        reason: 'the rule A saved was not on screen before the key',
      );
    },
  );

  testWidgets(
    'focus on the valve, then Ctrl+L: the block sentence stood first',
    (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);
      await openSessionGrid(tester);

      focusValve(tester);
      await tester.pump();
      expect(
        FocusManager.instance.primaryFocus?.context
            ?.findAncestorWidgetOfExactType<ReleaseValve>(),
        isNotNull,
        reason: 'the valve holds the keyboard focus',
      );
      final Set<String> before = onScreen(tester);
      // Am Ventil steht der Satz für Senden.
      expect(before, contains('allow · ∗ · registry.npmjs.org · this session'));

      // Ein fokussiertes Control behält die einzelnen Buchstaben für sich
      // (`docs/UX.md` 5.2); blockiert wird von dort mit dem Akkord `Ctrl+L`.
      await pressControl(tester, LogicalKeyboardKey.keyL);
      await tester.pump();

      final Rule rule = savedRule(client, RuleAction.block);
      expect(
        before,
        contains(sentenceOf(rule)),
        reason: 'the rule Ctrl+L saved was not on screen before the key',
      );
    },
  );

  // Auf Ebene der Leiste und nicht am ganzen Bildschirm: Bei 480 px und
  // doppelter Schrift laufen auf Deutsch schon heute Teile daneben über, die
  // mit diesem Issue nichts zu tun haben („Bearbeiten + Senden", der Kopf der
  // Karte). Geprüft wird hier, was die Regel betrifft: Jeder Satz steht ganz.
  testWidgets('480 px, twice the text, German: the sentence wraps whole', (
    WidgetTester tester,
  ) async {
    final FlowDetail detail = held(
      1,
      host: 'telemetry-ingest.eu-central-1.example-analytics.com',
      apex: 'example-analytics.com',
    );
    final TestDaemonClient client = TestDaemonClient();
    client.details[detail.summary.id] = detail;
    await tester.binding.setSurfaceSize(const Size(480, 900));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          uiStateOverride(),
          daemonClientProvider.overrideWithValue(client),
          connectionHeartbeatProvider.overrideWithValue(null),
          nowProvider.overrideWith(() => FixedNow(testStart)),
          flowsProvider.overrideWith(
            () => FixedFlows(<FlowId, Flow>{detail.summary.id: detail.summary}),
          ),
          rememberDraftProvider.overrideWith(ApexSessionDraft.new),
        ],
        child: WidgetsApp(
          color: const Color(0xFF000000),
          locale: const Locale('de'),
          localizationsDelegates: AppLocalizations.localizationsDelegates,
          supportedLocales: AppLocalizations.supportedLocales,
          builder: (BuildContext context, Widget? _) => MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: HTheme.dark(
              child: Overlay(
                initialEntries: <OverlayEntry>[
                  OverlayEntry(
                    builder: (BuildContext context) => Align(
                      alignment: Alignment.bottomCenter,
                      child: ActionBar(flow: detail.summary),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pump();
    await tester.pump();

    final List<RenderParagraph> sentences = tester
        .renderObjectList<RenderParagraph>(
          find.descendant(
            of: find.byKey(const Key('intercept-rule-sentence')),
            matching: find.byType(RichText),
          ),
        )
        .toList();
    expect(sentences, hasLength(2));
    for (final (int i, RuleAction action) in <RuleAction>[
      RuleAction.allow,
      RuleAction.block,
    ].indexed) {
      final RenderParagraph paragraph = sentences[i];
      final Rule rule = buildRule(
        RuleDraft(
          duration: RememberDuration.session,
          target: RememberTarget.apex,
          flow: detail.summary,
          action: action,
        ),
        now: testStart,
        apexOf: (String host) => 'example-analytics.com',
      )!;
      // Der ganze Satz der Regel, die die Taste speicherte ...
      expect(paragraph.text.toPlainText(), sentenceOf(rule, de));
      // ... und er ist nicht gekürzt, sondern umgebrochen.
      expect(
        paragraph.didExceedMaxLines,
        isFalse,
        reason: '"${paragraph.text.toPlainText()}" is cut short',
      );
      expect(paragraph.size.height, greaterThan(actionBarLineHeight));
    }
    // Und beide stehen ganz im Bild, ohne Scrollen: Die Leiste lässt sie zu.
    final ScrollableState region = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const Key('intercept-rule-sentence-scroll')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    expect(region.position.maxScrollExtent, 0);
    expect(tester.takeException(), isNull);
  });

  testWidgets('holding the valve with the grid closed shows its rule first', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[held(1)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    // Ohne Raster steht kein Regelsatz, nur der Grund des Anhaltens.
    expect(find.byKey(const Key('intercept-rule-sentence')), findsNothing);

    final TestGesture finger = await tester.startGesture(
      tester.getCenter(find.byKey(const Key('intercept-valve-hold'))),
    );
    await tester.pump(const Duration(milliseconds: 100));
    // Mitten im Halten, bevor etwas gespeichert ist.
    expect(client.decisions, isEmpty);
    final Set<String> during = onScreen(tester);
    expect(during, contains('allow · ∗ · registry.npmjs.org · this session'));

    await tester.pump(HMotion.holdToConfirm + const Duration(milliseconds: 50));
    await tester.pump();
    await finger.up();
    await tester.pump();

    final Rule rule = savedRule(client, RuleAction.allow);
    expect(
      during,
      contains(sentenceOf(rule)),
      reason: 'the rule the hold saved was not on screen before it fired',
    );
  });

  testWidgets(
    'a draft that changes in the last frame of a hold saves nothing',
    (WidgetTester tester) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );
      await pumpIntercept(tester, client: client);
      await playScript(tester);

      final TestGesture finger = await tester.startGesture(
        tester.getCenter(find.byKey(const Key('intercept-valve-hold'))),
      );
      await tester.pump(const Duration(milliseconds: 100));
      expect(
        onScreen(tester),
        contains('allow · ∗ · registry.npmjs.org · this session'),
      );

      // `3` öffnet das Raster mit einer Stunde, ohne dass ein Frame dazwischen
      // den neuen Satz malt: Das Halten schließt im Animations-Callback ab,
      // vor dem Build.
      await tester.sendKeyEvent(LogicalKeyboardKey.digit3);
      await tester.pump(
        HMotion.holdToConfirm + const Duration(milliseconds: 50),
      );
      await finger.up();
      await tester.pump();

      expect(
        client.decisions,
        isEmpty,
        reason: 'the hold showed a session rule and must not save another one',
      );
      expect(
        find.text(
          'The rule changed while you held · nothing was sent, hold again',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('a short window: the modal confirms only after the whole rule', (
    WidgetTester tester,
  ) async {
    final String path =
        '/${List<String>.generate(160, (int i) => 'segment$i').join('/')}';
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[held(1, path: path)]),
    );
    await pumpIntercept(tester, client: client, size: const Size(1400, 600));
    await playScript(tester);

    // Für immer, über genau diese URL: Das fragt im Modal (`docs/UX.md` 5.4).
    await tester.sendKeyEvent(LogicalKeyboardKey.digit4);
    await tester.pump();
    await pressShiftKey(tester, LogicalKeyboardKey.digit1);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 300));

    expect(find.byKey(const Key('intercept-batch-modal')), findsOneWidget);
    expect(tester.takeException(), isNull);
    final Rect window = Offset.zero & const Size(1400, 600);
    final Rect sentence = tester.getRect(
      find.byKey(const Key('intercept-batch-rule')),
    );
    // Der Satz ist länger als das Fenster; ein Teil steht außerhalb.
    expect(window.contains(sentence.bottomLeft), isFalse);
    expect(
      find.byKey(const Key('intercept-batch-scroll-hint')),
      findsOneWidget,
    );

    // Bestätigen tut nichts, solange der Satz nicht ganz zu sehen war.
    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pump();
    await tester.pump();
    expect(client.decisions, isEmpty);
    expect(find.byKey(const Key('intercept-batch-modal')), findsOneWidget);

    // Bis zum Ende gescrollt, dann geht es.
    final ScrollableState body = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const Key('intercept-batch-modal')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    body.position.jumpTo(body.position.maxScrollExtent);
    await tester.pump();
    await tester.pump();
    final String shown = tester
        .renderObject<RenderParagraph>(
          find.descendant(
            of: find.byKey(const Key('intercept-batch-rule')),
            matching: find.byType(RichText),
          ),
        )
        .text
        .toPlainText();
    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pump();
    await tester.pump();

    final Rule rule = savedRule(client, RuleAction.allow);
    expect(shown, startsWith(sentenceOf(rule)));
  });

  testWidgets('a rule longer than the bar saves only after its end was seen', (
    WidgetTester tester,
  ) async {
    final String path =
        '/${List<String>.generate(160, (int i) => 'segment$i').join('/')}';
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[held(1, path: path)]),
    );
    await pumpIntercept(tester, client: client, size: const Size(1400, 600));
    await playScript(tester);

    // Sitzung, über genau diese URL: Die Taste spräche ohne Modal.
    await openSessionGrid(tester);
    await pressShiftKey(tester, LogicalKeyboardKey.digit1);
    await tester.pump();
    expect(tester.takeException(), isNull);
    final ScrollableState region = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const Key('intercept-rule-sentence-scroll')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    // Die Sätze sind länger als ihr Platz; ihr Ende steht außer Sicht.
    expect(region.position.maxScrollExtent, greaterThan(0));

    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, isEmpty);
    expect(
      find.text(
        'Scroll the rule under the controls to its end first · nothing was '
        'sent',
      ),
      findsOneWidget,
    );

    region.position.jumpTo(region.position.maxScrollExtent);
    await tester.pump();
    await tester.pump();
    final String shown = tester
        .renderObject<RenderParagraph>(
          find.descendant(
            of: find.byKey(const Key('intercept-rule-sentence-allow')),
            matching: find.byType(RichText),
          ),
        )
        .text
        .toPlainText();
    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.pump();

    final Rule rule = savedRule(client, RuleAction.allow);
    expect(shown, sentenceOf(rule));
  });

  testWidgets('a pane that shrinks after reading hides the end again', (
    WidgetTester tester,
  ) async {
    final String path =
        '/${List<String>.generate(20, (int i) => 'segment$i').join('/')}';
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[held(1, path: path)]),
    );
    await pumpIntercept(tester, client: client, size: const Size(1400, 900));
    await playScript(tester);
    await openSessionGrid(tester);
    await pressShiftKey(tester, LogicalKeyboardKey.digit1);
    await tester.pump();

    ScrollPosition region() => tester
        .state<ScrollableState>(
          find
              .descendant(
                of: find.byKey(const Key('intercept-rule-sentence-scroll')),
                matching: find.byType(Scrollable),
              )
              .first,
        )
        .position;
    // Im hohen Fenster passen die Sätze ganz.
    expect(region().maxScrollExtent, 0);

    // Das Fenster wird niedriger; das Ende der Sätze rutscht aus dem Bild.
    await tester.binding.setSurfaceSize(const Size(1400, 600));
    await tester.pump();
    await tester.pump();
    expect(region().maxScrollExtent, greaterThan(0));

    await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
    await tester.pump();
    await tester.pump();
    expect(client.decisions, isEmpty);
  });

  // Die M2-Demo klickt „Bestätigen" in dem Frame, in dem das Modal zum ersten
  // Mal steht. Passt der Rumpf, gilt er sofort als gesehen; ein Knopf, der
  // erst einen Frame später angeht, verschluckte diesen Klick.
  testWidgets('a modal whose body fits confirms in its very first frame', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[held(1)]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.digit4);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(find.byKey(const Key('intercept-batch-modal')), findsOneWidget);

    await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
    await tester.pump();
    await tester.pump();

    savedRule(client, RuleAction.allow);
  });
}
