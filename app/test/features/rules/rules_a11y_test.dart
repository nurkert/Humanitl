// Was Abschnitt 6 und 7 von `docs/UX.md` als Zahlen verlangen: Textskalierung
// bis 2.0 ohne Overflow, Ziele ab 28 px, Semantik mit Platz und Frist, und
// reduzierte Bewegung, die den Weg streicht und die Rückmeldung behält.
//
// Die beiden Warteschwellen des Probelaufs stehen in `dry_run_panel_test.dart`:
// dort steht auch, was unter der Schwelle nicht stehen darf, und ein Test, der
// nur den Titel prüft, hielte das nicht.

import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/rules/widgets/draw_in.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';

import 'fixtures.dart';

void main() {
  RulesTestClient seeded() {
    final RulesTestClient client = RulesTestClient();
    client.savedRules.addAll(<Rule>[
      testRule(n: 1, host: 'registry.npmjs.org', note: 'npm packages'),
      testRule(n: 2, action: RuleAction.block, host: '**.tracking.example'),
    ]);
    client.sessionRules.add(
      testRule(
        n: 3,
        action: RuleAction.ask,
        host: 'api.github.com',
        expires: const RuleExpiry.session(),
      ),
    );
    return client;
  }

  testWidgets('the list survives double text scale', (
    WidgetTester tester,
  ) async {
    await pumpRules(
      tester,
      client: seeded(),
      textScaler: const TextScaler.linear(2),
    );

    expect(tester.takeException(), isNull);
    expect(find.byType(RuleRow), findsNWidgets(3));
    // Die Zeilendichte ist eine Mindesthöhe, keine feste: bei doppelter
    // Skalierung wächst die Zeile, statt den Text abzuschneiden.
    expect(
      tester.getSize(find.byType(RuleRow).first).height,
      greaterThan(HSize.row),
    );
  });

  testWidgets('the editor survives double text scale', (
    WidgetTester tester,
  ) async {
    await pumpRules(
      tester,
      client: seeded(),
      textScaler: const TextScaler.linear(2),
    );

    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 600));

    expect(tester.takeException(), isNull);
    expect(find.byKey(const Key('rule-save')), findsOneWidget);
  });

  testWidgets('every hit target clears the minimum', (
    WidgetTester tester,
  ) async {
    await pumpRules(tester, client: seeded());
    await hoverOver(tester, find.byType(RuleRow).first);

    for (final Finder target in <Finder>[
      find.byKey(const ValueKey<String>('rule-grip-0')),
      find.byKey(const ValueKey<String>('rule-delete-0')),
    ]) {
      final Size size = tester.getSize(target);
      expect(size.width, greaterThanOrEqualTo(HSize.hitMin));
      expect(size.height, greaterThanOrEqualTo(HSize.hitMin));
    }

    // Und die beiden Controls des Editors.
    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pump();
    final Size save = tester.getSize(find.byKey(const Key('rule-save')));
    expect(save.height, greaterThanOrEqualTo(HSize.hitMin));
  });

  testWidgets('a row says its place in the chain and when it ends', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle semantics = tester.ensureSemantics();
    final RulesTestClient client = RulesTestClient();
    client.savedRules.add(
      testRule(
        n: 1,
        host: 'registry.npmjs.org',
        expires: RuleExpiry.at(
          at: DateTime.now().add(const Duration(hours: 3)),
        ),
      ),
    );
    await pumpRules(tester, client: client);

    final SemanticsNode node = tester.getSemantics(find.byType(RuleRow).first);
    // One of one: a position counts inside its group, and the bundled rule is
    // a group of its own (CONVENTIONS 4.5).
    expect(node.label, contains('Rule 1 of 1'));
    expect(node.label, contains('registry.npmjs.org'));
    // Die Frist steht im Value, nicht im Label: ein Label, das jede Minute
    // wechselt, wird von jeder Bildschirmleserin vollständig wiederholt
    // (docs/UX.md 6).
    expect(node.value, isNotEmpty);
    expect(node.label, isNot(contains(node.value)));
    semantics.dispose();
  });

  testWidgets('reduced motion drops the travel and keeps the fade', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client, reducedMotion: true);

    // Eine neue Regel entsteht, während der Screen steht -- über denselben
    // Weg wie im Betrieb, den Notifier.
    await addRuleThroughApp(
      tester,
      testRule(n: 4, host: 'crates.io', action: RuleAction.allow),
    );
    await tester.pump();

    expect(find.byType(DrawIn), findsWidgets);
    // Kein Wachsen der Höhe, nur ein Einblenden: der Weg fällt weg, die
    // Rückmeldung bleibt (docs/UX.md 2.10).
    expect(find.byType(SizeTransition), findsNothing);
    expect(find.byType(FadeTransition), findsWidgets);
  });

  testWidgets('a new rule draws itself in and then leaves the tree', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await addRuleThroughApp(
      tester,
      testRule(n: 4, host: 'crates.io', action: RuleAction.allow),
    );
    await tester.pump();
    expect(find.byType(SizeTransition), findsOneWidget);

    await tester.pumpAndSettle();
    // Der Wrapper verschwindet, sobald er nichts mehr zu tun hat
    // (docs/UX.md 7).
    expect(find.byType(SizeTransition), findsNothing);
    expect(find.byType(RuleRow), findsNWidgets(4));
  });

  testWidgets('every bound key moves the rule it is on', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Die zweite Regel nach oben, dann dieselbe Regel wieder nach unten.
    for (final (int row, LogicalKeyboardKey key) in <(int, LogicalKeyboardKey)>[
      (1, LogicalKeyboardKey.arrowUp),
      (0, LogicalKeyboardKey.arrowDown),
    ]) {
      focusRow(tester, row);
      await tester.pump();
      await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
      await tester.sendKeyEvent(key);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
      await tester.pump();
      await tester.pump();
    }

    // Jede gebundene Taste tut etwas (docs/UX.md 5.3), und zusammen ergeben
    // sie wieder die Reihenfolge von vorher.
    expect(client.reorders, hasLength(2));
    expect(client.savedRules.map((Rule rule) => rule.id).toList(), <RuleId>[
      testRuleId(1),
      testRuleId(2),
    ]);
  });
}
