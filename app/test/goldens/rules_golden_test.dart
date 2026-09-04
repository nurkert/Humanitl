// Goldens des Rules-Screens (HUM-033): die Kette im Tab „Gespeichert", die
// Kette im Tab „Temporär" und der Editor mit seinem Probelauf, je dunkel und
// hell. Erneuern mit `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart' hide Flow;
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/rules/providers/editor.dart';
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:humanitl/core/ui/ui.dart';

import '../features/rules/fixtures.dart';

/// Der Regelsatz der Goldens: vier eigene Regeln, eine Sitzungsregel und die
/// mitgelieferte des Fakes. Keine Regel mit Endzeitpunkt, damit dasselbe Bild
/// zweimal dasselbe Bild ist.
RulesTestClient goldenClient() {
  final RulesTestClient client = RulesTestClient();
  client.savedRules.addAll(<Rule>[
    testRule(
      n: 1,
      host: '**.npmjs.org',
      methods: <Method>[Method.get, Method.head],
      path: '/**',
      note: 'npm packages',
    ),
    testRule(
      n: 2,
      action: RuleAction.block,
      host: '**.tracking.example',
      note: 'analytics endpoints',
    ),
    testRule(
      n: 3,
      action: RuleAction.redact,
      host: 'api.github.com',
      methods: <Method>[Method.post],
      path: '/graphql',
      createdFrom: const FlowId('018f0001-0000-7000-8000-000000010000'),
    ),
    testRule(n: 4, host: 'crates.io', methods: <Method>[Method.get]),
  ]);
  client.sessionRules.add(
    testRule(
      n: 5,
      action: RuleAction.ask,
      host: 'storage.googleapis.com',
      expires: const RuleExpiry.session(),
      note: 'until I know what it uploads',
    ),
  );
  for (int i = 1; i <= 4; i++) {
    final Flow flow = testFlow(
      n: i,
      host: i.isEven ? 'registry.npmjs.org' : 'api.github.com',
      path: i.isEven ? '/react/-/react-19.2.0.tgz' : '/graphql',
    );
    client.state.flows[flow.id] = flow;
  }
  return client;
}

/// Ein Tab, der nicht wechselt.
class FixedTab extends RuleTabSelection {
  /// Bleibt bei [tab].
  FixedTab(this.tab);

  /// Der feste Tab.
  final RuleTab tab;

  @override
  RuleTab build() => tab;
}

/// Ein Editor, der beim Bauen schon offen ist.
class OpenEditor extends RuleEditorController {
  /// Zeigt [rule].
  OpenEditor(this.rule);

  /// Der Entwurf im Formular.
  final Rule rule;

  @override
  RuleEditorState build() => RuleEditorState(draft: rule);
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'rules_list_saved_$name',
      fileName: 'rules_list_saved_$name',
      constraints: window,
      pumpBeforeTest: (WidgetTester tester) async {
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 600));
      },
      builder: () => rulesUnderTest(client: goldenClient(), mode: mode),
    );

    goldenTest(
      'rules_list_temporary_$name',
      fileName: 'rules_list_temporary_$name',
      constraints: window,
      pumpBeforeTest: (WidgetTester tester) async {
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 600));
      },
      builder: () => rulesUnderTest(
        client: goldenClient(),
        mode: mode,
        overrides: <Override>[
          ruleTabSelectionProvider.overrideWith(
            () => FixedTab(RuleTab.temporary),
          ),
        ],
      ),
    );

    goldenTest(
      'rules_editor_dry_run_$name',
      fileName: 'rules_editor_dry_run_$name',
      constraints: window,
      pumpBeforeTest: (WidgetTester tester) async {
        await tester.pump();
        // Der Probelauf wartet seine Sperrfrist ab und antwortet danach.
        await tester.pump(ruleDryRunDebounce * 2);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 600));
      },
      builder: () => rulesUnderTest(
        client: goldenClient(),
        mode: mode,
        overrides: <Override>[
          ruleEditorProvider.overrideWith(
            () => OpenEditor(
              Rule(
                action: RuleAction.allow,
                matcher: const RuleMatcher(
                  host: 'api.github.com',
                  methods: <Method>[Method.post],
                  path: '/graphql',
                ),
                expires: const RuleExpiry.session(),
                note: 'the agent files issues',
              ),
            ),
          ),
        ],
      ),
    );
  }
}
