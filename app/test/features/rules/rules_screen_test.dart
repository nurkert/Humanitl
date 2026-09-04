// Der Rules-Screen: Tabs, Kette, Reihenfolge, Löschen mit Rückgängig,
// Dauerhaft-Machen und der Befund über der Liste.

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/flow_handoff.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/rules/widgets/rules_list.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

void main() {
  late AppLocalizations l10n;

  setUpAll(() async {
    l10n = await AppLocalizations.delegate.load(const Locale('en'));
  });

  /// Ein Client mit zwei gespeicherten und einer Sitzungsregel, dazu die
  /// mitgelieferte Regel des Fakes.
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

  testWidgets('tabs_split_saved_temporary', (WidgetTester tester) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Gespeichert: zwei eigene Regeln plus die mitgelieferte.
    expect(find.byType(RuleRow), findsNWidgets(3));
    expect(find.text(l10n.rulesBundledTitle), findsOneWidget);
    // Die Kette läuft über den Tab hinaus, und der Screen sagt das.
    expect(find.text(l10n.rulesChainSessionFirst(1)), findsOneWidget);
    expect(find.text(l10n.rulesChainDefault), findsOneWidget);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();

    expect(find.byType(RuleRow), findsOneWidget);
    expect(find.text(l10n.rulesBundledTitle), findsNothing);
    expect(find.text(l10n.rulesChainThenSaved(3)), findsOneWidget);
  });

  testWidgets('reorder_calls_rpc_with_position', (WidgetTester tester) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Die Tastenentsprechung des Ziehens: Alt und ein Pfeil bewegen die
    // fokussierte Regel (docs/UX.md 5.1).
    focusRow(tester, 0);
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pump();
    await tester.pump();

    expect(client.reorders, hasLength(1));
    // Die ganze Liste geht hin, ohne die mitgelieferte Regel, und die beiden
    // gespeicherten stehen getauscht darin.
    expect(client.reorders.single, <RuleId>[
      testRuleId(3),
      testRuleId(2),
      testRuleId(1),
    ]);
    expect(client.savedRules.first.id, testRuleId(2));
  });

  testWidgets('drag_moves_the_rule_the_pointer_moved', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    final Finder handle = find.byKey(const ValueKey<String>('rule-grip-0'));
    expect(handle, findsOneWidget);
    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(handle),
    );
    await tester.pump(kPressTimeout);
    await gesture.moveBy(const Offset(0, 20));
    await tester.pump();
    await gesture.moveBy(const Offset(0, 40));
    await tester.pump();
    await gesture.up();
    await tester.pumpAndSettle();

    expect(client.reorders, hasLength(1));
    expect(client.savedRules.first.id, testRuleId(2));
  });

  testWidgets('bundled_not_draggable_not_deletable', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Zwei Griffe für zwei eigene Regeln; die mitgelieferte hat keinen.
    expect(find.byKey(const ValueKey<String>('rule-grip-0')), findsOneWidget);
    expect(find.byKey(const ValueKey<String>('rule-grip-1')), findsOneWidget);
    expect(find.byKey(const ValueKey<String>('rule-grip-2')), findsNothing);

    // Und keinen Papierkorb: der Screen bietet nicht an, was der Daemon
    // ablehnen würde (RULES_010).
    final Finder bundledRow = find.byType(RuleRow).last;
    expect(find.byKey(const ValueKey<String>('rule-delete-2')), findsNothing);
    expect(client.removed, isEmpty);

    // Der Editor zeigt sie, ändert sie nicht und nennt den einen Weg.
    await tester.tap(bundledRow);
    await tester.pump();
    expect(find.text(l10n.rulesEditorBundledTitle), findsOneWidget);
    expect(find.byKey(const Key('rule-override')), findsOneWidget);
    expect(find.byKey(const Key('rule-save')), findsNothing);
  });

  testWidgets('delete_undo_restores_position', (WidgetTester tester) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Die zweite gespeicherte Regel löschen. Der Papierkorb erscheint bei
    // Hover und bei Fokus; hier fährt der Zeiger hin, wie er es täte.
    final Finder second = find.byType(RuleRow).at(1);
    await hoverOver(tester, second);
    // Mit der Maus geklickt, nicht getippt: ein Tippen schaltet den
    // Hervorhebungsmodus auf „touch" zurück, und dann wäre der Papierkorb
    // wieder verborgen -- genau wie im Betrieb.
    await tester.tap(
      find.byKey(const ValueKey<String>('rule-delete-1')),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    await tester.pump();

    expect(client.removed, <RuleId>[testRuleId(2)]);
    expect(find.text(l10n.rulesUndoRemoved), findsOneWidget);

    await tester.tap(find.byKey(const Key('rules-undo')));
    await tester.pump();
    await tester.pump();

    expect(client.added, hasLength(1));
    // Der Platz ist ein Wunsch, und er wird geäußert: Platz 2 der eigenen
    // Gruppe, eins-basiert.
    expect(client.added.single.position, 2);
    expect(client.savedRules.map((Rule rule) => rule.id).toList(), <RuleId>[
      testRuleId(1),
      testRuleId(2),
    ]);
    expect(find.text(l10n.rulesUndoRemoved), findsNothing);
  });

  testWidgets('make_permanent_moves_tab', (WidgetTester tester) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();

    await tester.tap(find.byKey(const Key('rule-make-permanent')));
    await tester.pump();
    await tester.pump();

    expect(client.permanent, <RuleId>[testRuleId(3)]);
    expect(client.sessionRules, isEmpty);
    expect(
      client.savedRules.map((Rule rule) => rule.id),
      contains(testRuleId(3)),
    );
    // Der Tab, in dem sie stand, ist leer; die Kette geht im anderen weiter.
    expect(find.text(l10n.rulesEmptyTemporaryTitle), findsOneWidget);
    expect(find.text(l10n.rulesUndoPermanent), findsOneWidget);
  });

  testWidgets('a refused file stands over the list with its own words', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    client.reloadDiagnostics = <Diagnostic>[FakeDaemonClient.brokenFileReport];
    await pumpRules(tester, client: client);

    await tester.tap(find.byKey(const Key('rules-reload')));
    await tester.pump();
    await tester.pump();
    await tester.pumpAndSettle();

    expect(client.reloadCalls, 1);
    expect(find.text(DiagnosticCodes.rulesFileInvalid), findsOneWidget);
    // Der Satz des Daemons, mit Feld und Zeile, nicht ein umgeschriebener.
    expect(
      find.textContaining('rules[2].match.host (line 12)'),
      findsOneWidget,
    );
    // Und die Liste zeigt weiter den letzten gültigen Stand.
    expect(find.byType(RuleRow), findsNWidgets(3));
  });

  testWidgets('an empty tab names the next event, not the absence', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    await pumpRules(tester, client: client);

    expect(find.text(l10n.rulesEmptySavedTitle), findsOneWidget);
    expect(find.byType(RuleRow), findsNothing);
    for (final String forbidden in <String>[
      'Nothing',
      'nothing',
      ' no ',
      'yet',
    ]) {
      expect(
        find.textContaining(forbidden),
        findsNothing,
        reason: 'an empty state never says "$forbidden" (docs/UX.md 4.1)',
      );
    }
  });

  testWidgets('a filter that matches nothing counts and offers the way back', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.enterText(find.byKey(const Key('rules-filter')), 'zzz');
    await tester.pump();

    expect(find.text(l10n.rulesFilterEmpty('zzz', 3)), findsOneWidget);
    await tester.tap(find.byKey(const Key('rules-filter-reset')));
    await tester.pump();
    expect(find.byType(RuleRow), findsNWidgets(3));
  });

  testWidgets('a filtered list cannot be dragged', (WidgetTester tester) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.enterText(find.byKey(const Key('rules-filter')), 'npm');
    await tester.pump();

    expect(find.byType(RuleRow), findsOneWidget);
    expect(find.byKey(const ValueKey<String>('rule-grip-0')), findsNothing);
  });

  testWidgets('an answer that changes nothing rebuilds no row', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    final Element row = tester.element(find.byType(RuleRow).first);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(RulesList)),
    );
    final int before = client.listCalls;

    await container.read(rulesProvider.notifier).refresh();

    // Der Regelsatz kam noch einmal, und er war derselbe: `RuleSet` und
    // `RuleChain` haben Wertgleichheit, also merkt riverpod, dass sich nichts
    // geändert hat, und keine Zeile wird schmutzig (docs/UX.md 7).
    expect(client.listCalls, before + 1);
    expect(row.dirty, isFalse);
    await tester.pump();
    expect(tester.element(find.byType(RuleRow).first).dirty, isFalse);
  });

  testWidgets('the number is the place in the group, not in the view', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    // Two own rules, one bundled: 1, 2 -- and the bundled one starts its own
    // group at 1, because that is where the daemon counts it (CONVENTIONS
    // 4.5) and because an override has to go in front of *that* one.
    final List<int> shown = <int>[
      for (int i = 0; i < 3; i++)
        int.parse(
          tester
              .widgetList<Text>(
                find.descendant(
                  of: find.byType(RuleRow).at(i),
                  matching: find.byType(Text),
                ),
              )
              .first
              .data!,
        ),
    ];
    expect(shown, <int>[1, 2, 1]);

    // A filter hides rules; it does not renumber the ones it keeps.
    await tester.enterText(find.byKey(const Key('rules-filter')), 'tracking');
    await tester.pump();
    expect(find.byType(RuleRow), findsOneWidget);
    expect(
      tester
          .widgetList<Text>(
            find.descendant(
              of: find.byType(RuleRow),
              matching: find.byType(Text),
            ),
          )
          .first
          .data,
      '2',
    );
  });

  testWidgets('a rule that has run out claims nothing any more', (
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
    client.sessionRules.add(
      testRule(
        n: 2,
        host: 'api.github.com',
        expires: RuleExpiry.at(
          at: DateTime.now().subtract(const Duration(minutes: 5)),
        ),
      ),
    );
    await pumpRules(tester, client: client);

    // The rail and the glyph of a rule the engine skips are the ones of a
    // hold that ran out, never the saturated colour of an action it no longer
    // takes (CONVENTIONS 4.13).
    expect(
      tester.widget<HRow>(find.byType(HRow).first).state,
      HFlowState.timedOut,
    );
    expect(find.text(l10n.rulesExpired), findsWidgets);
  });

  testWidgets('the chain note counts what is still in force', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.add(testRule(n: 1));
    client.sessionRules.addAll(<Rule>[
      testRule(n: 2, expires: const RuleExpiry.session()),
      testRule(
        n: 3,
        expires: RuleExpiry.at(
          at: DateTime.now().subtract(const Duration(minutes: 5)),
        ),
      ),
    ]);
    await pumpRules(tester, client: client);

    // Two temporary rules stand in the other tab, but only one of them is
    // evaluated before these: the other one is over.
    expect(find.text(l10n.rulesChainSessionFirst(1)), findsOneWidget);
  });

  testWidgets('under 900 px the editor is a sheet, and Escape closes it', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    // Der Aufbau der übrigen Tests ist 1280 breit; der Blatt-Weg lief deshalb
    // in keinem von ihnen.
    await pumpRules(tester, client: client, size: const Size(800, 800));

    await tester.tap(find.byType(RuleRow).first);
    await tester.pumpAndSettle();
    expect(find.byType(HSheet), findsOneWidget);
    expect(find.byKey(const Key('rule-save')), findsOneWidget);

    // Ein Blatt fängt den Fokus und schließt auf `Escape`; sonst ist es für
    // die Tastatur eine Sackgasse (`docs/UX.md` 5.1 und 5.4).
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(HSheet), findsNothing);
    expect(find.byKey(const Key('rule-save')), findsNothing);
    // Und die Kette dahinter steht noch.
    expect(find.byType(RuleRow), findsNWidgets(3));
  });

  testWidgets('the origin of a rule is a control that reaches the request', (
    WidgetTester tester,
  ) async {
    const FlowId from = FlowId('018f0001-0000-7000-8000-000000010000');
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.add(testRule(n: 1, createdFrom: from));
    // Das Abzeichen verlässt die Zeile unter 420 px Spaltenbreite; der
    // Standardaufbau mit 1280 px ist genau darunter.
    await pumpRules(tester, client: client, size: const Size(1440, 800));

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(RuleRow).first),
    );
    expect(container.read(flowHandoffProvider), isNull);

    final Finder origin = find.byKey(
      ValueKey<String>('rule-origin-${from.value}'),
    );
    expect(origin, findsOneWidget);
    await tester.tap(origin);
    await tester.pump();

    // Die Bitte steht im Provider; ausgeführt wird sie von der Shell, damit
    // kein Feature in ein anderes greift (ARCHITECTURE 5).
    expect(container.read(flowHandoffProvider), from);

    // Und dasselbe über die Tastatur, weil jede Zeigergeste eine
    // Tastenentsprechung hat (docs/UX.md 5.1).
    container.read(flowHandoffProvider.notifier).clear();
    await tester.pump();
    Focus.of(
      tester.element(
        find.descendant(of: origin, matching: find.byType(Text)).first,
      ),
    ).requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(container.read(flowHandoffProvider), from);
  });

  testWidgets('a truncated note keeps its second half at the pointer', (
    WidgetTester tester,
  ) async {
    const String note =
        'a lock for further agents, not a claim about what OpenCode does '
        'today';
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.add(testRule(n: 1, note: note));
    // Die Notiz verlässt die Zeile unter 520 px Spaltenbreite.
    await pumpRules(tester, client: client, size: const Size(1800, 800));

    final Finder shown = find.text(note);
    expect(shown, findsOneWidget);
    // Einzeilig gekürzt: die Einschränkung steht im zweiten Halbsatz und
    // fiele weg.
    expect(tester.widget<Text>(shown).maxLines, 1);
    expect(
      tester
          .widget<HoverLabel>(
            find.ancestor(of: shown, matching: find.byType(HoverLabel)),
          )
          .label,
      note,
    );

    await hoverOver(tester, shown);
    await tester.pump(HMotion.hoverLabel);
    await tester.pump();
    expect(find.text(note), findsNWidgets(2));
  });

  testWidgets('Alt and an arrow key in a filtered list say why they did not', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.enterText(find.byKey(const Key('rules-filter')), 'npm');
    await tester.pump();
    expect(find.byType(RuleRow), findsOneWidget);

    focusRow(tester, 0);
    await tester.pump();
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
    await tester.pump();

    // Nichts geht an den Daemon, und die Taste schweigt nicht: der Grund
    // steht über der Liste (docs/UX.md 5.3).
    expect(client.reorders, isEmpty);
    expect(find.text(l10n.rulesMoveRefusedFiltered), findsOneWidget);

    // Ohne Filter ist die Lage eine andere, und der Grund geht mit ihr.
    await tester.enterText(find.byKey(const Key('rules-filter')), '');
    await tester.pump();
    expect(find.text(l10n.rulesMoveRefusedFiltered), findsNothing);
  });

  testWidgets('make permanent works on the saved rule, never on the form', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();

    // Im Formular auf `allow` gestellt und nicht gespeichert.
    await tester.tap(find.text(l10n.rulesActionAllow).last);
    await tester.pump();

    expect(find.text(l10n.rulesMakePermanentDirty), findsOneWidget);
    await tester.tap(find.byKey(const Key('rule-make-permanent')));
    await tester.pump();
    await tester.pump();

    // Nichts ist gegangen. Sonst nähme der Daemon nur die Id, die Regel
    // bliebe `ask`, und der Streifen böte ein „Undo" an, das aus `ask` ein
    // `allow` machte (docs/UX.md 4.5).
    expect(client.permanent, isEmpty);
    expect(client.updated, isEmpty);
    expect(find.text(l10n.rulesUndoPermanent), findsNothing);
    expect(client.sessionRules.single.action, RuleAction.ask);
  });

  testWidgets('a rule that is already permanent offers no such button', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    expect(find.byKey(const Key('rule-make-permanent')), findsNothing);

    // Auch nicht, wenn das Formular auf die Sitzung gestellt wird: der
    // Daemon bekäme nur die Id und antwortete IPC_005.
    // Das Formular ist die eine Liste, die scrollt; die Lebensdauer steht
    // unterhalb des ersten Bildes.
    final Finder session = find.text(l10n.rulesExpiresSession);
    await tester.scrollUntilVisible(
      session,
      120,
      scrollable: find
          .descendant(
            of: find.byType(ListView),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    await tester.ensureVisible(session);
    await tester.pump();
    await tester.tap(session);
    await tester.pump();
    expect(find.byKey(const Key('rule-make-permanent')), findsNothing);
  });

  testWidgets('a later change to the same rule takes the undo offer back', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-make-permanent')));
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('rules-undo')), findsOneWidget);

    // Dieselbe Regel, jetzt im Saved-Tab, wird enger gestellt und gespeichert.
    // Der Streifen fährt ein und schiebt die Liste; erst danach steht eine
    // Zeile dort, wo der Test sie antippt.
    await tester.tap(find.text(l10n.rulesTabSaved(4)));
    await tester.pumpAndSettle();
    final Finder moved = find.byWidgetPredicate(
      (Widget widget) => widget is RuleRow && widget.rule.id == testRuleId(3),
    );
    expect(moved, findsOneWidget);
    await tester.tap(moved);
    await tester.pumpAndSettle();
    await tester.tap(find.text(l10n.rulesActionBlock).last);
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-save')));
    await tester.pump();
    await tester.pump();

    expect(client.updated.single.action, RuleAction.block);
    // Der Streifen spricht über einen Zustand, den es nicht mehr gibt. Ein
    // Druck auf „Undo" machte aus der gespeicherten `block`-Regel wieder eine
    // `allow`-Regel für die Sitzung -- ein Rückgängig, das weitet
    // (docs/UX.md 4.5).
    expect(find.byKey(const Key('rules-undo')), findsNothing);
  });

  testWidgets('undo of make permanent puts back the lifetime, nothing else', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();
    await tester.tap(find.byType(RuleRow).first);
    await tester.pump();
    await tester.tap(find.byKey(const Key('rule-make-permanent')));
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('rules-undo')), findsOneWidget);

    // Die Regel ändert sich außerhalb dieses Fensters, so wie eine von Hand
    // editierte `rules.yaml` es täte, und der Screen fragt neu.
    final int at = client.savedRules.indexWhere(
      (Rule rule) => rule.id == testRuleId(3),
    );
    client.savedRules[at] = client.savedRules[at].copyWith(
      note: 'changed outside',
    );
    // Der Tab, in dem die Regel stand, ist leer; der Streifen steht.
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byKey(const Key('rules-undo'))),
    );
    await container.read(rulesProvider.notifier).refresh();
    await tester.pump();

    await tester.tap(find.byKey(const Key('rules-undo')));
    await tester.pump();
    await tester.pump();

    // Zurückgenommen wird die Frist, nicht der ganze Schnappschuss von
    // vorhin: was inzwischen an der Regel steht, bleibt stehen.
    expect(client.sessionRules, hasLength(1));
    expect(client.sessionRules.single.id, testRuleId(3));
    expect(client.sessionRules.single.note, 'changed outside');
    expect(client.sessionRules.single.expires, const RuleExpiry.session());
  });

  testWidgets('both counts of the other tab are the same count', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = RulesTestClient()..bundledRules.clear();
    client.savedRules.addAll(<Rule>[
      testRule(n: 1, host: 'registry.npmjs.org'),
      testRule(
        n: 2,
        host: 'api.github.com',
        expires: RuleExpiry.at(
          at: DateTime.now().subtract(const Duration(minutes: 5)),
        ),
      ),
    ]);
    client.sessionRules.add(
      testRule(n: 3, host: 'crates.io', expires: const RuleExpiry.session()),
    );
    await pumpRules(tester, client: client);

    await tester.tap(find.text(l10n.rulesTabTemporary(1)));
    await tester.pump();

    // Im selben Bild stehen zwei Zahlen über dieselbe Menge; sie zählen
    // dasselbe: was der andere Tab hält.
    expect(find.text(l10n.rulesTabSaved(2)), findsOneWidget);
    expect(find.text(l10n.rulesChainThenSaved(2)), findsOneWidget);
  });

  testWidgets('a finding of any answer stands over the list', (
    WidgetTester tester,
  ) async {
    final RulesTestClient client = seeded();
    await pumpRules(tester, client: client);
    expect(find.text(DiagnosticCodes.rulesFileInvalid), findsNothing);

    // Nicht nur der Reload trägt Befunde: `RulesResponse.diagnostics` steht an
    // jeder Antwort, und eine Antwort, deren Befunde niemand liest, wäre eine
    // stille Warnung (docs/UX.md 4.4).
    client.answerDiagnostics = <Diagnostic>[FakeDaemonClient.brokenFileReport];
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(RuleRow).first),
    );
    await container.read(rulesProvider.notifier).refresh();
    await tester.pump();
    await tester.pumpAndSettle();

    expect(find.text(DiagnosticCodes.rulesFileInvalid), findsOneWidget);
  });
}
