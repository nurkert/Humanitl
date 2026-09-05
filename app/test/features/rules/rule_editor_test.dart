// Der Editor: Vorprüfung während des Tippens, ein Probelauf statt dreier,
// der Befund des Daemons unter dem Formular und der Weg um eine
// mitgelieferte Regel herum.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/rules/providers/editor.dart';
import 'package:humanitl/features/rules/widgets/rule_editor.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

void main() {
  late AppLocalizations l10n;

  setUpAll(() async {
    l10n = await AppLocalizations.delegate.load(const Locale('en'));
  });

  /// Öffnet den leeren Editor.
  Future<void> openNew(WidgetTester tester) async {
    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pump();
  }

  /// Holt [target] ins Bild: das Formular ist die eine Liste, die scrollt,
  /// und der angeheftete Fuß deckt sonst die untere Hälfte ab.
  Future<void> reveal(WidgetTester tester, Finder target) async {
    await tester.scrollUntilVisible(
      target,
      120,
      scrollable: find
          .descendant(
            of: find.byType(ListView),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.ensureVisible(target);
    await tester.pump();
  }

  testWidgets('editor_local_validation_star_in_label', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), '*npmjs.org');
    await tester.pump();

    // Sofort, ohne Rundreise, mit dem Satz, der die Regel nennt.
    expect(find.text(l10n.rulesHostWildcard), findsOneWidget);
    // Und der Probelauf fragt nicht nach einem Muster, das ohnehin abgelehnt
    // würde.
    await tester.pump(ruleDryRunDebounce * 2);
    expect(client.dryRuns, isEmpty);

    // Ein ganzes Label ist in Ordnung, und dann läuft auch der Probelauf.
    await tester.enterText(find.byKey(const Key('rule-host')), '**.npmjs.org');
    await tester.pump();
    expect(find.text(l10n.rulesHostWildcard), findsNothing);
    await tester.pump(ruleDryRunDebounce * 2);
    expect(client.dryRuns, hasLength(1));
  });

  testWidgets('dry_run_debounced', (WidgetTester tester) async {
    final RulesTestClient client = RulesTestClient();
    client.state.flows[testFlow(n: 1).id] = testFlow(n: 1);
    client.state.flows[testFlow(n: 2, host: 'api.github.com').id] = testFlow(
      n: 2,
      host: 'api.github.com',
    );
    await pumpRules(tester, client: client);
    await openNew(tester);

    // Drei schnelle Änderungen: jede verwirft die Anfrage der vorigen.
    await tester.enterText(find.byKey(const Key('rule-host')), 'r');
    await tester.pump(const Duration(milliseconds: 50));
    await tester.enterText(find.byKey(const Key('rule-host')), 'registry');
    await tester.pump(const Duration(milliseconds: 50));
    await tester.enterText(
      find.byKey(const Key('rule-host')),
      'registry.npmjs.org',
    );
    // Ein Frame für die Eingabe, wie im Betrieb auch; erst danach steht fest,
    // welcher Entwurf gefragt wird.
    await tester.pump(const Duration(milliseconds: 50));
    await tester.pump(ruleDryRunDebounce * 2);
    // Und ein Frame für die Antwort.
    await tester.pump();

    expect(client.dryRuns, hasLength(1));
    expect(client.dryRuns.single.matcher.host, 'registry.npmjs.org');
    // Und die Antwort zählt, was sie geprüft hat, ohne sie eine Entscheidung
    // zu nennen.
    expect(find.text(l10n.rulesDryRunResult(1, 2)), findsOneWidget);
    expect(find.text(l10n.rulesDryRunOnlyThisRule), findsOneWidget);
  });

  testWidgets('a saved rule appears in the list it belongs to', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    await pumpRules(tester, client: client);
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    await tester.pump();
    await tester.tap(find.text(l10n.rulesActionAllow).last);
    await tester.pump();
    // Die Vorgabe ist die Sitzung; das steht so im Entwurf und wird geprüft.
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(client.added, hasLength(1));
    expect(client.added.single.action, RuleAction.allow);
    expect(client.sessionRules, hasLength(1));
    expect(find.byType(RuleRow), findsNothing);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pumpAndSettle();
    expect(find.byType(RuleRow), findsOneWidget);
  });

  testWidgets('a refused rule keeps the form open and shows the daemon words', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()
      ..addFailure = const Diagnostic(
        code: DiagnosticCodes.hostPatternInvalid,
        severity: Severity.error,
        title: 'Host pattern invalid',
        why:
            'match.host (line 4): host pattern "xn--mnchen-3ya.de" contains a '
            'punycode label',
      );
    await pumpRules(tester, client: client);
    await openNew(tester);

    await tester.enterText(
      find.byKey(const Key('rule-host')),
      'xn--mnchen-3ya.de',
    );
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const Key('rule-save')), findsOneWidget);
    expect(find.text(l10n.rulesSaveFailedTitle), findsOneWidget);
    expect(find.text(DiagnosticCodes.hostPatternInvalid), findsOneWidget);
    expect(find.textContaining('match.host (line 4)'), findsOneWidget);
  });

  testWidgets('a_switched_off_rule_says_so_in_the_editor_too', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);

    Finder inEditor(Finder what) =>
        find.descendant(of: find.byType(RuleEditor), matching: what);
    Finder glyph(HGlyph wanted) => find.byWidgetPredicate(
      (Widget widget) => widget is HGlyphIcon && widget.glyph == wanted,
    );

    // Eingeschaltet: Schloss, kein Kreuz, kein Wort über einen Zustand.
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    expect(inEditor(glyph(HGlyph.lock)), findsWidgets);
    expect(inEditor(glyph(HGlyph.close)), findsNothing);
    expect(inEditor(find.text(l10n.rulesOriginBundledOff)), findsNothing);

    // Ausgeschaltet: dieselben drei Kanäle wie in der Zeile. Der Editor ist
    // die größere Hälfte des Bildschirms; stünde die Regel hier in voller
    // Stärke, läse sie sich als wirksam.
    client.bundledRules[0] = client.bundledRules[0].copyWith(disabled: true);
    await tester.tap(find.byKey(const Key('rules-reload')));
    await tester.pump();
    await tester.pump();
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();

    expect(inEditor(find.text(l10n.rulesOriginBundledOff)), findsOneWidget);
    expect(inEditor(glyph(HGlyph.close)), findsWidgets);
    expect(inEditor(glyph(HGlyph.lock)), findsNothing);
  });

  testWidgets('override_bundled_creates_an_ask_rule_in_front', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);

    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-override')));
    await tester.pump();

    expect(find.byKey(const Key('rule-save')), findsOneWidget);
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(client.added, hasLength(1));
    expect(client.added.single.action, RuleAction.ask);
    expect(client.added.single.matcher.host, 'models.dev');
    // Vorne in der eigenen Gruppe: nur davor gewinnt sie gegen die
    // mitgelieferte Regel.
    expect(client.added.single.position, 1);
    expect(client.savedRules.first.matcher.host, 'models.dev');
  });

  testWidgets('the preview says what will be saved', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    await tester.pump();

    // Derselbe Wortlaut, den die Aktionsleiste vor dem Anlegen zeigt: ein
    // Generator, ein Satz ARB-Schlüssel (CONVENTIONS 4.13).
    expect(find.text('ask · ∗ · crates.io · this session'), findsOneWidget);
  });

  testWidgets('a pristine field is not a mistake, a pressed Save says why', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);
    await openNew(tester);

    // Nothing is wrong yet: nobody has answered the field.
    expect(find.text(l10n.rulesHostEmpty), findsNothing);

    // Pressing Save says why instead of doing nothing, and sends nothing.
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    expect(find.text(l10n.rulesHostEmpty), findsOneWidget);
    expect(client.added, isEmpty);
  });

  testWidgets('a path the engine cannot build blocks the save too', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    await tester.enterText(find.byKey(const Key('rule-path')), '~(unclosed');
    await tester.pump();
    expect(find.text(l10n.rulesPathRegex), findsOneWidget);

    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, isEmpty);

    // With a path it can build, the same press saves.
    await tester.enterText(find.byKey(const Key('rule-path')), '/**');
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, hasLength(1));
  });

  testWidgets('a time nobody can read blocks the save', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    // Hoch genug, dass das Formular ohne Scrollen dasteht: die Lebensdauer
    // steht sonst unter dem Falz, und ein Test, der dorthin tippt, prüft die
    // Geometrie des Fensters statt die Regel.
    await pumpRules(tester, client: client, size: const Size(1280, 1200));
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    // Ein Frame dazwischen, wie im Betrieb: die Rückrufe des Formulars lesen
    // den Entwurf des letzten Aufbaus, und ohne diesen Frame trüge er den
    // Host noch nicht.
    await tester.pump();
    await tester.tap(find.text(l10n.rulesExpiresAt));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('rule-ends-at')), 'tomorrow');
    await tester.pump();

    expect(find.text(l10n.rulesEndsAtInvalid), findsOneWidget);
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, isEmpty);
  });

  testWidgets('the stream flag has no control until a tier guards it', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.add(
      testRule(n: 1, host: 'llm.lan', action: RuleAction.allow, stream: true),
    );
    // Hoch genug für das ganze Formular: die Liste baut nur, was ins Bild
    // passt, und „nicht gebaut" wäre kein Beweis für „nicht da".
    await pumpRules(tester, client: client, size: const Size(1280, 2000));
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();

    // Das Formular steht vollständig da; die Notiz ist sein letztes Feld.
    expect(find.byKey(const Key('rule-note')), findsOneWidget);
    // Das eine Control, das einen Rumpf über der Kappe ungelesen hinauslässt,
    // steht nicht da, solange kein Tier es schützt (CONVENTIONS 4.16).
    expect(find.byKey(const Key('rule-stream')), findsNothing);
    expect(find.text(l10n.rulesStream), findsNothing);
    expect(find.text(l10n.rulesFieldStream), findsNothing);

    // Und das Feld der Regel bleibt, was der Daemon geliefert hat: das
    // Formular schreibt es weder an noch ab.
    await tester.tap(find.text(l10n.rulesActionBlock));
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(client.updated, hasLength(1));
    expect(client.updated.single.action, RuleAction.block);
    expect(client.updated.single.stream, isTrue);
  });

  testWidgets('an end time that has passed blocks the save', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client, size: const Size(1280, 1200));
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    // Ein Frame dazwischen, wie im Betrieb: die Rückrufe des Formulars lesen
    // den Entwurf des letzten Aufbaus, und ohne diesen Frame trüge er den
    // Host noch nicht.
    await tester.pump();
    await tester.tap(find.text(l10n.rulesExpiresAt));
    await tester.pump();
    await tester.enterText(
      find.byKey(const Key('rule-ends-at')),
      '1999-01-01T00:00',
    );
    await tester.pump();

    // Lesbar und trotzdem keine Frist: die Engine überspringt eine Regel,
    // deren Ende vorbei ist, also gälte sie ab dem Speichern nichts. Bei
    // einer `block`-Regel ist das die weitende Richtung (CONVENTIONS 4.13).
    expect(find.text(l10n.rulesEndsAtPast), findsOneWidget);
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, isEmpty);

    // Mit einem Zeitpunkt in der Zukunft speichert derselbe Druck.
    final DateTime later = DateTime.now().add(const Duration(hours: 2));
    await tester.enterText(
      find.byKey(const Key('rule-ends-at')),
      later.toIso8601String().split('.').first.substring(0, 16),
    );
    await tester.pump();
    expect(find.text(l10n.rulesEndsAtPast), findsNothing);
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, hasLength(1));
  });

  testWidgets('the port field is as wide as it says it is', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client);
    await openNew(tester);

    final Finder port = find.byKey(const Key('rule-port'));
    await reveal(tester, port);
    // Die dokumentierte Breite und die gezeichnete sind dieselbe Zahl; eine
    // Literale daneben liefe beim ersten Ändern der Konstante auseinander.
    expect(tester.getSize(port).width, rulePortFieldWidth);
  });

  testWidgets('switching the lifetime away and back drops the old reason', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient();
    await pumpRules(tester, client: client, size: const Size(1280, 1200));
    await openNew(tester);

    await tester.enterText(find.byKey(const Key('rule-host')), 'crates.io');
    await tester.pump();
    await tester.tap(find.text(l10n.rulesExpiresAt));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('rule-ends-at')), 'tomorrow');
    await tester.pump();
    expect(find.text(l10n.rulesEndsAtInvalid), findsOneWidget);

    // Weg vom Zeitpunkt und wieder hin: das Feld trägt jetzt eine frische
    // Stunde, und die Begründung von vorhin beschreibt nichts mehr.
    await tester.tap(find.text(l10n.rulesExpiresSession));
    await tester.pump();
    await tester.tap(find.text(l10n.rulesExpiresAt));
    await tester.pump();
    expect(find.text(l10n.rulesEndsAtInvalid), findsNothing);
    expect(find.text(l10n.rulesEndsAtPast), findsNothing);

    // Und `Save` verweigert nicht mehr mit einem Grund, den es nicht gibt.
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();
    expect(client.added, hasLength(1));
  });

  testWidgets('a rule whose end has passed is not saved again', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.add(
      testRule(
        n: 1,
        host: 'registry.npmjs.org',
        expires: RuleExpiry.at(
          at: DateTime.now().subtract(const Duration(minutes: 5)),
        ),
      ),
    );
    await pumpRules(tester, client: client, size: const Size(1280, 1200));

    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    // Niemand hat hier getippt: die Prüfung hängt an der Uhr und nicht am
    // Rückruf des Feldes. Ein Zeitpunkt, der beim Tippen in der Zukunft lag,
    // ist beim Speichern Vergangenheit, und `RulesStore::validated` lehnt ihn
    // nicht ab.
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(client.updated, isEmpty);
    expect(find.text(l10n.rulesEndsAtPast), findsOneWidget);
  });
}
