// Der Probelauf: was das Panel sagt, solange es nichts weiß, und in welcher
// Farbe der Beleg dafür steht, dass eine Regel eine Anfrage getroffen hätte.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/rules/providers/editor.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

void main() {
  late AppLocalizations l10n;

  setUpAll(() async {
    l10n = await AppLocalizations.delegate.load(const Locale('en'));
  });

  /// Ein Client mit zwei aufgezeichneten Anfragen, von denen eine zur Regel
  /// des Tests passt.
  RulesTestClient recorded() {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    for (final Flow flow in <Flow>[
      testFlow(n: 1),
      testFlow(n: 2, host: 'api.github.com', path: '/graphql'),
    ]) {
      client.state.flows[flow.id] = flow;
    }
    return client;
  }

  Future<void> openNewWithHost(WidgetTester tester, String host) async {
    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pump();
    await tester.enterText(find.byKey(const Key('rule-host')), host);
    await tester.pump();
  }

  testWidgets('a dry run without an answer counts nothing, and says so', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = recorded();
    await pumpRules(tester, client: client);
    await openNewWithHost(tester, 'registry.npmjs.org');

    // Solange der Daemon nicht geantwortet hat, steht hier keine Zahl: „0 von
    // 0" wäre eine gezählte Null, hinter der man Grün vermuten könnte, und
    // genau dieses Panel soll eine zu weite Regel aufhalten (CONVENTIONS
    // 4.13).
    expect(find.text(l10n.rulesDryRunResult(0, 0)), findsNothing);
    expect(find.text(l10n.rulesDryRunCounting), findsOneWidget);

    // Über der Schwelle steht das Skelett da, wo die Zeilen stehen werden.
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byType(HSkeleton), findsOneWidget);
    expect(find.text(l10n.rulesDryRunResult(0, 0)), findsNothing);

    // Und dann die Antwort, mit ihrem Bezug daneben.
    await tester.pump(ruleDryRunDebounce);
    await tester.pump();
    expect(find.text(l10n.rulesDryRunCounting), findsNothing);
    expect(find.text(l10n.rulesDryRunResult(1, 2)), findsOneWidget);
    expect(find.text(l10n.rulesDryRunOnlyThisRule), findsOneWidget);
  });

  testWidgets('under the threshold nothing appears at all', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = recorded();
    await pumpRules(tester, client: client);
    await openNewWithHost(tester, 'registry.npmjs.org');

    // Eine Anzeige, die kürzer sichtbar ist als eine Reaktionszeit, wird als
    // Flackern gelesen (docs/UX.md 2.11): unter HMotion.waitVisible steht der
    // Titel und sonst nichts Neues.
    await tester.pump(const Duration(milliseconds: 100));
    expect(find.text(l10n.rulesDryRunTitle), findsOneWidget);
    expect(find.byType(HSkeleton), findsNothing);
    expect(find.text(l10n.rulesDryRunResult(0, 0)), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('a dry run that failed says so with the daemon words', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = recorded()
      ..dryRunFailure = const Diagnostic(
        code: DiagnosticCodes.hostPatternInvalid,
        severity: Severity.error,
        title: 'Host pattern invalid',
        why: 'match.host: the recorder could not be read',
      );
    await pumpRules(tester, client: client);
    await openNewWithHost(tester, 'registry.npmjs.org');
    await tester.pump(ruleDryRunDebounce * 2);
    await tester.pump();

    // Kein Zählsatz, der ewig zählt: an der Stelle der Antwort steht der
    // Befund, mit Code und den Worten des Daemons (docs/UX.md 4.4).
    expect(find.text(l10n.rulesDryRunCounting), findsNothing);
    expect(find.text(l10n.rulesDryRunFailedTitle), findsOneWidget);
    expect(find.text(DiagnosticCodes.hostPatternInvalid), findsOneWidget);
    expect(
      find.textContaining('the recorder could not be read'),
      findsOneWidget,
    );
  });

  testWidgets('the time of a match is evidence, so it is readable', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = recorded();
    await pumpRules(tester, client: client);
    await openNewWithHost(tester, 'registry.npmjs.org');
    await tester.pump(ruleDryRunDebounce * 2);
    await tester.pump();

    final HTokens tokens = HThemeMode.dark.resolve(Brightness.dark);
    final Text time = tester.widget<Text>(
      find.text(l10n.rulesDryRunTime(testFlow(n: 1).receivedAt.toLocal())),
    );
    // `fg2` ist wirklich deaktivierten Controls vorbehalten und misst unter
    // 4,5:1 (`docs/UX.md` 6); der Zeitpunkt wird gelesen.
    expect(time.style?.color, tokens.colors.fg1);
  });
}
