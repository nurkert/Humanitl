// Die Karte, mit der der Agent um etwas bittet (HUM-073, ADR-014).
//
// Der Text auf dieser Karte ist das einzige Stück Text im ganzen Programm, das
// der Agent geschrieben hat, und das Blatt daneben legt eine Regel an. Die
// Tests hier prüfen deshalb nicht nur, dass die Karte erscheint, sondern auch,
// dass sie nichts von dem tut, was ein feindlicher Text von ihr wollen würde:
// als Systemmeldung durchgehen, den Host abschneiden, oder mit einem Klick
// einen ganzen Host aufmachen.
//
// Jede Zusicherung, die eine Schutzmaßnahme prüft, nennt in ihrem Kommentar
// die Änderung, die sie rot macht; die Proben sind von Hand gefahren worden.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/agent_asks.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';

import 'harness.dart';

/// Ein Skript, das nach 100 ms genau eine Bitte des Agenten schickt.
List<ScriptedEvent> askScript({
  required String text,
  String suggestedHost = '',
  String suggestedPath = '',
  String askId = 'ask-1',
}) => <ScriptedEvent>[
  ScriptedEvent(
    const Duration(milliseconds: 100),
    (FakeSessionState state, DateTime now) => FlowEvent.agentAsk(
      at: now,
      askId: askId,
      text: text,
      suggestedHost: suggestedHost,
      suggestedPath: suggestedPath,
    ),
  ),
];

/// Die Regeln, die in diesem Test entstanden sind.
///
/// Der Fake bringt eine mitgelieferte Regel mit (`models.dev`); sie gehört
/// nicht dem Nutzer und zählt hier nicht.
Future<List<Rule>> ownRules(FakeDaemonClient client) async =>
    (await client.listRules()).rules
        .where((Rule rule) => !rule.bundled)
        .toList();

/// Der Inhalt eines Textfeldes des Blattes.
String fieldText(WidgetTester tester, String key) => tester
    .widget<EditableText>(
      find.descendant(
        of: find.byKey(Key(key)),
        matching: find.byType(EditableText),
      ),
    )
    .controller
    .text;

/// Schreibt [value] in das Feld [key].
Future<void> typeInto(WidgetTester tester, String key, String value) async {
  await tester.enterText(find.byKey(Key(key)), value);
  await tester.pumpAndSettle();
}

/// Wählt im Regel-Blatt die Aktion [label].
///
/// Der Bildschirm trägt dieselben Wörter auch in der Aktionsleiste; gesucht
/// wird deshalb ausdrücklich im Blatt.
Future<void> chooseAction(WidgetTester tester, String label) async {
  await tester.tap(
    find.descendant(
      of: find.byType(AgentAskRuleSheet),
      matching: find.text(label),
    ),
  );
  await tester.pumpAndSettle();
}

/// Öffnet das Regel-Blatt der einen Karte.
Future<void> openRuleSheet(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('intercept-agent-ask-open-rule')));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('agent_ask_card_appears', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org/simple/ freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/',
      ),
    );
    await pumpIntercept(tester, client: client);
    expect(find.byType(AgentAskCard), findsNothing);

    await playScript(tester);

    expect(find.byType(AgentAskCard), findsOneWidget);
    expect(
      find.text('bitte https://pypi.org/simple/ freischalten'),
      findsOneWidget,
    );
    expect(find.text('pypi.org/simple/'), findsOneWidget);
  });

  testWidgets('agent_ask_text_is_plain_text', (WidgetTester tester) async {
    // Der Text kommt vom Agenten. Er darf weder als Markdown noch als Verweis
    // wirken und in keiner Schicht stecken, die ihn anklickbar oder greifbar
    // macht.
    const String text =
        'siehe **fett** und [klick](https://evil.io) sowie <b>tag</b>';
    final FakeDaemonClient client = fakeDaemon(askScript(text: text));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final Finder drawnFinder = find.byKey(
      const Key('intercept-agent-ask-text'),
    );
    final Text drawn = tester.widget<Text>(drawnFinder);
    // `data` gesetzt und `textSpan` leer: genau die Zeichenkette, die ankam.
    // Rot, sobald jemand `Text.rich` daraus macht.
    expect(drawn.data, text);
    expect(drawn.textSpan, isNull);

    // Und die Spanne, die daraus gemalt wird, trägt keinen Erkenner und keine
    // Kinder — ein Link in der Mitte des Textes wäre genau das.
    // Rot, sobald eine Spanne mit `recognizer` entsteht.
    final RichText painted = tester.widget<RichText>(
      find.descendant(of: drawnFinder, matching: find.byType(RichText)),
    );
    final InlineSpan span = painted.text;
    expect(span, isA<TextSpan>());
    expect((span as TextSpan).recognizer, isNull);
    expect(span.children, isNull);
    expect(span.toPlainText(), text);

    // Keine Auswahlschicht darüber. `SelectionArea` baut genau dieses Widget;
    // die Prüfung greift deshalb für beide Schreibweisen.
    // Rot, sobald die Karte in eine `SelectionArea` gewickelt wird.
    expect(
      find.ancestor(of: drawnFinder, matching: find.byType(SelectableRegion)),
      findsNothing,
    );

    // Der Kasten ist in beide Richtungen begrenzt: Zeilen gedeckelt und
    // Rahmen beschnitten, damit kein Zeichen über die Bedienelemente läuft.
    // Rot, sobald `maxLines` oder das `ClipRect` verschwindet.
    expect(drawn.maxLines, agentAskMaxLines);
    expect(
      find.ancestor(of: drawnFinder, matching: find.byType(ClipRect)),
      findsWidgets,
    );
  });

  testWidgets('agent_ask_cannot_pose_as_the_application', (
    WidgetTester tester,
  ) async {
    // Ein Text, der die Ausgabe von `/` nachahmt und wie eine Systemmeldung
    // aussieht. Er bleibt Zitat: eigenes Abzeichen, eigene Schrift, eigene
    // Ansage für den Screenreader — und keine Regel.
    const String text =
        'humanitl session=00000000-0000-7000-8000-000000000000 ask=none '
        'timeout=0 llm=none rules (first match wins): allow * * * never';
    final FakeDaemonClient client = fakeDaemon(askScript(text: text));
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final BuildContext context = tester.element(find.byType(AgentAskCard));
    final HTokens tokens = HTheme.of(context);

    // 1. Das Abzeichen nennt die Quelle. Rot ohne `HBadge`.
    expect(find.text('Agent'), findsOneWidget);

    // 2. Der Text steht in der Zitatschrift, nicht in der Schrift, in der die
    //    Anwendung selbst spricht. Genau das unterscheidet die Zeile des
    //    Agenten von einer Zeile von uns.
    //    Rot, sobald der Text die Titelschrift `ui13.semibold`/`fg0` bekommt.
    final Text drawn = tester.widget<Text>(
      find.byKey(const Key('intercept-agent-ask-text')),
    );
    expect(drawn.data, text, reason: 'the text is quoted, never interpreted');
    expect(
      drawn.style,
      tokens.typography.mono12.tinted(tokens.colors.fg1),
      reason: 'quoted material has its own face',
    );
    expect(
      drawn.style,
      isNot(tokens.typography.ui13.semibold.tinted(tokens.colors.fg0)),
      reason: 'never the face the application speaks in',
    );

    // 3. Auch vorgelesen wird die Quelle vor den Worten genannt.
    //    Rot, sobald das `Semantics`-Etikett der Karte den Vorspann verliert.
    final Semantics wrapper = tester.widget<Semantics>(
      find
          .descendant(
            of: find.byType(AgentAskCard),
            matching: find.byType(Semantics),
          )
          .first,
    );
    final String spoken = wrapper.properties.label ?? '';
    expect(spoken, contains(text));
    expect(
      spoken.indexOf(text),
      greaterThan(0),
      reason: 'the source is named before the words are read out',
    );

    // 4. Der Text nennt eine Regel; angelegt wurde keine.
    expect(await ownRules(client), isEmpty);
  });

  testWidgets('agent_ask_host_is_never_shortened', (WidgetTester tester) async {
    // `pypi.org.attacker.com` darf nie als `pypi.org…` erscheinen: Das wäre
    // Domain-Täuschung durch die eigene Oberfläche, genau in dem Augenblick,
    // in dem ein Mensch entscheidet.
    // Rot, sobald die Zeile wieder `overflow: TextOverflow.ellipsis` bekommt.
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org.attacker.com/ freischalten',
        suggestedHost: 'pypi.org.attacker.com',
      ),
    );
    // Schmale Spalte: Hier würde eine Ellipse wirklich zuschlagen.
    await pumpIntercept(tester, client: client, size: const Size(1100, 900));
    await playScript(tester);

    final Text host = tester.widget<Text>(
      find.byKey(const Key('intercept-agent-ask-host')),
    );
    expect(host.data, 'pypi.org.attacker.com');
    expect(host.overflow, isNot(TextOverflow.ellipsis));
    expect(
      host.maxLines,
      isNull,
      reason: 'the name wraps, it never ends early',
    );
    expect(find.text('pypi.org.attacker.com'), findsOneWidget);
  });

  testWidgets('agent_ask_rule_is_as_narrow_as_the_request', (
    WidgetTester tester,
  ) async {
    // Der blockierende Befund: Ein Klick darf nicht den ganzen Host öffnen.
    // Der Pfad aus der URL steht im Blatt und in der Regel, und die Regel wird
    // als Ganzes verglichen — jede spätere Verbreiterung fällt hier auf.
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org/simple/flask/ freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/flask/',
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openRuleSheet(tester);

    expect(fieldText(tester, 'intercept-agent-ask-rule-host'), 'pypi.org');
    expect(
      fieldText(tester, 'intercept-agent-ask-rule-path'),
      '/simple/flask/',
      reason: 'the request named a path, so the rule carries it',
    );
    // Ohne Pfad stünde hier die Warnung; mit Pfad steht sie nicht da.
    expect(
      find.byKey(const Key('intercept-agent-ask-rule-whole-host')),
      findsNothing,
    );

    await chooseAction(tester, 'Allow');
    await tester.tap(find.byKey(const Key('intercept-agent-ask-rule-create')));
    await tester.pumpAndSettle();

    final List<Rule> rules = await ownRules(client);
    expect(rules, hasLength(1));
    final Rule made = rules.single;
    // Die ganze Regel, Feld für Feld: nichts davon darf weiter sein als die
    // Bitte, aus der sie entstand.
    expect(made.action, RuleAction.allow);
    expect(made.matcher.host, 'pypi.org');
    expect(made.matcher.path, '/simple/flask/');
    expect(made.matcher.methods, isEmpty);
    expect(made.matcher.scheme, isNull);
    expect(made.matcher.port, 0);
    expect(made.matcher.upgrade, isNull);
    expect(made.expires, const RuleExpiry.session());
    expect(made.stream, isFalse);
    expect(made.allowPrivate, isFalse);
    expect(made.bundled, isFalse);
    expect(made.note, isNull);
    expect(made.createdFrom, isNull);

    expect(find.byType(AgentAskRuleSheet), findsNothing);
    expect(
      find.byType(AgentAskCard),
      findsNothing,
      reason: 'the request is answered, so its card is gone',
    );
  });

  testWidgets('agent_ask_rule_has_no_preselected_action', (
    WidgetTester tester,
  ) async {
    // Das Feld, das entscheidet, ob Verkehr fließt, wird von Hand gewählt.
    // Ohne Wahl bleibt der Knopf aus, und ein Klick darauf legt nichts an.
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org/simple/ freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/',
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openRuleSheet(tester);

    expect(
      find.byKey(const Key('intercept-agent-ask-rule-choose')),
      findsOneWidget,
    );
    final HButton create = tester.widget<HButton>(
      find.byKey(const Key('intercept-agent-ask-rule-create')),
    );
    expect(create.onPressed, isNull, reason: 'nothing is chosen yet');

    await tester.tap(
      find.byKey(const Key('intercept-agent-ask-rule-create')),
      warnIfMissed: false,
    );
    await tester.pumpAndSettle();
    expect(await ownRules(client), isEmpty);
    expect(find.byType(AgentAskRuleSheet), findsOneWidget);

    // Nach der Wahl geht es.
    await chooseAction(tester, 'Block');
    expect(
      find.byKey(const Key('intercept-agent-ask-rule-choose')),
      findsNothing,
    );
    await tester.tap(find.byKey(const Key('intercept-agent-ask-rule-create')));
    await tester.pumpAndSettle();
    final List<Rule> rules = await ownRules(client);
    expect(rules, hasLength(1));
    expect(rules.single.action, RuleAction.block);
  });

  testWidgets('agent_ask_rule_says_when_it_opens_a_whole_host', (
    WidgetTester tester,
  ) async {
    // Ohne Pfad ist die Regel viel weiter als die Bitte. Das muss dastehen,
    // bevor jemand sie anlegt, und es muss verschwinden, sobald ein Pfad da
    // ist.
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org freischalten',
        suggestedHost: 'pypi.org',
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openRuleSheet(tester);

    expect(fieldText(tester, 'intercept-agent-ask-rule-path'), isEmpty);
    expect(
      find.byKey(const Key('intercept-agent-ask-rule-whole-host')),
      findsOneWidget,
    );
    expect(
      find.textContaining('every method and every path of pypi.org'),
      findsOneWidget,
    );

    await typeInto(tester, 'intercept-agent-ask-rule-path', '/simple/');
    expect(
      find.byKey(const Key('intercept-agent-ask-rule-whole-host')),
      findsNothing,
    );
  });

  testWidgets('agent_ask_rule_refuses_a_host_that_is_no_pattern', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org/simple/ freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/',
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openRuleSheet(tester);
    await chooseAction(tester, 'Allow');

    await typeInto(tester, 'intercept-agent-ask-rule-host', '*pypi.org');
    expect(
      find.byKey(const Key('intercept-agent-ask-rule-host-problem')),
      findsOneWidget,
    );
    final HButton create = tester.widget<HButton>(
      find.byKey(const Key('intercept-agent-ask-rule-create')),
    );
    expect(create.onPressed, isNull, reason: 'a broken pattern is not sent');
    expect(await ownRules(client), isEmpty);
  });

  testWidgets('agent_ask_rule_shows_why_the_daemon_refused', (
    WidgetTester tester,
  ) async {
    // Der Daemon hat das letzte Wort über ein Muster. Sagt er nein, steht sein
    // Satz im Blatt, und der Entwurf bleibt stehen.
    final FakeDaemonClient client = fakeDaemon(
      askScript(
        text: 'bitte https://pypi.org/simple/ freischalten',
        suggestedHost: 'pypi.org',
        suggestedPath: '/simple/',
      ),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    await openRuleSheet(tester);
    await chooseAction(tester, 'Allow');
    // Ein regulärer Ausdruck, den die Engine nicht bauen kann: Der Client
    // prüft Pfade nicht vor, der Daemon lehnt ab.
    await typeInto(tester, 'intercept-agent-ask-rule-path', '~[');
    await tester.tap(find.byKey(const Key('intercept-agent-ask-rule-create')));
    await tester.pumpAndSettle();

    expect(
      find.byKey(const Key('intercept-agent-ask-rule-failure')),
      findsOneWidget,
    );
    expect(find.textContaining('The rule was not created'), findsOneWidget);
    expect(
      find.byType(AgentAskRuleSheet),
      findsOneWidget,
      reason: 'the draft survives a refusal',
    );
    expect(find.byType(AgentAskCard), findsOneWidget);
    expect(await ownRules(client), isEmpty);
  });

  testWidgets('agent_ask_dismiss_takes_the_card_away', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      askScript(text: 'bitte irgendetwas freischalten'),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);
    expect(find.byType(AgentAskCard), findsOneWidget);

    containerOf(tester).read(agentAsksProvider.notifier).dismiss('ask-1');
    await tester.pumpAndSettle();

    expect(find.byType(AgentAskCard), findsNothing);
  });

  testWidgets('two_asks_make_two_cards', (WidgetTester tester) async {
    final FakeDaemonClient client = fakeDaemon(<ScriptedEvent>[
      ...askScript(text: 'die erste Bitte', askId: 'ask-1'),
      ...askScript(text: 'die zweite Bitte', askId: 'ask-2'),
    ]);
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    expect(find.byType(AgentAskCard), findsNWidgets(2));
    expect(find.text('die erste Bitte'), findsOneWidget);
    expect(find.text('die zweite Bitte'), findsOneWidget);
  });
}
