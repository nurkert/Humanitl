// Der Katalog auf dem Bildschirm (HUM-094): Der Kopf einer Gruppe nennt den
// Dienst, sobald jede gehaltene Anfrage dieselbe Kennung vom Daemon trägt, und
// weiter den Host, sobald sie es nicht tun; die Schnellregeln legen eine Regel
// an und entscheiden nie; das Modal bleibt für eine Gruppe ohne Daemon-Apex
// stehen.
//
// Jede Zusicherung nennt in ihrem Kommentar oder ihrem `reason`, welche
// Änderung am Produkt sie rot macht; die Proben sind von Hand gefahren worden.

import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/widgets/batch_modal.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';
import 'harness.dart';

/// Ein gehaltener Fluss an `registry.npmjs.org`, mit der Kennung des Daemons.
Flow npmFlow(
  int n, {
  String catalogId = 'npm',
  String host = 'registry.npmjs.org',
  String apex = 'npmjs.org',
}) => heldFlow(
  n: n,
  deadline: testStart.add(Duration(minutes: 5, seconds: n)),
  host: host,
  apex: apex,
).copyWith(catalogId: catalogId);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  // `rootBundle` merkt sich das geladene Asset als Future, und ein Future, das
  // in der abgelaufenen Zeitzone eines vorigen Widget-Tests fertig geworden
  // ist, hält den nächsten Test auf: Die Karte blieb dann bei der
  // Unbekannt-Karte stehen, obwohl der Katalog den Dienst kennt (gemessen
  // 2026-09-12). Vor jedem Test ein frischer Puffer.
  setUp(rootBundle.clear);

  group('group_title', () {
    late AppLocalizations l10n;

    // Der Eintrag steht hier ausgeschrieben, statt aus dem Asset gelesen zu
    // werden: Dass das Asset genau diese Werte trägt, prüft
    // `catalog_test.dart`; hier geht es allein um den Kopf, und ein Lesen des
    // Bündels in einem Test ohne Widget lässt die Widget-Tests derselben Datei
    // ihre Ladezeit nicht mehr abwarten (gemessen 2026-09-12).
    const CatalogEntry npm = CatalogEntry(
      id: 'npm',
      name: 'npm registry',
      category: 'registry',
      description: <String, String>{'en': 'Package registry for Node.js.'},
      typical: <String>['npm install'],
    );

    setUp(() async {
      l10n = await AppLocalizations.delegate.load(const Locale('en'));
    });

    test('group_title_uses_catalog_name', () {
      final HeldGroup group = groupFlows(<Flow>[npmFlow(1), npmFlow(2)])
          .groups
          .single;

      expect(group.catalogId, 'npm');
      // Rot, sobald `groupTitle` wieder auf `group.display` zurückfällt.
      expect(groupTitle(group, npm, l10n), 'npm registry');
      // Rot, sobald die Katalogzeile aus der Zusammenfassung fällt. Genau die
      // zwei Behauptungen prüft der M2-Lauf in Schritt 10.
      expect(
        groupSummary(group, npm, l10n),
        allOf(contains('npm registry'), contains('Looks like: npm install')),
      );
    });

    test('group_title_keeps_host_when_catalog_ids_differ', () {
      // Zwei Dienste unter einer Domäne: Der Kopf nennt dann keinen von beiden.
      final HeldGroup group = groupFlows(<Flow>[
        npmFlow(1),
        npmFlow(2, host: 'other.npmjs.org', catalogId: 'github'),
      ]).groups.single;

      expect(group.catalogId, '', reason: 'keine Kennung gilt für die Gruppe');
      expect(
        groupTitle(group, null, l10n),
        'registry.npmjs.org and 1 more host',
      );
      expect(groupSummary(group, null, l10n), isNot(contains('npm registry')));
      expect(groupSummary(group, null, l10n), isNot(contains('Looks like')));
    });

    test('group_title_keeps_host_when_one_flow_carries_no_catalog_id', () {
      final HeldGroup group = groupFlows(<Flow>[
        npmFlow(1),
        npmFlow(2, host: 'other.npmjs.org', catalogId: ''),
      ]).groups.single;

      expect(group.catalogId, '');
    });

    test('a_group_without_held_flows_carries_no_catalog_id', () {
      // Nur eine ruhende, schon entschiedene Zeile: Der Kopf sagt, was eine
      // Entscheidung erreichen würde, und das ist nichts.
      final HeldGroup group = groupFlows(<Flow>[
        npmFlow(1).copyWith(
          state: FlowState.decided,
          decision: DecisionKind.allow,
          deadline: null,
        ),
      ]).groups.single;

      expect(group.catalogId, '');
    });

    test('a_flow_without_an_apex_stands_under_its_own_host', () {
      // Ein leerer Apex steht für sich: Der Fluss landet unter seinem Host,
      // nicht unter der Domäne des anderen, und beide Köpfe tragen dann die
      // Kennung ihres einen Flusses (HUM-091, HUM-094).
      final HeldGroups groups = groupFlows(<Flow>[
        npmFlow(1, apex: ''),
        npmFlow(2),
      ]);

      expect(groups.groups, hasLength(2));
      expect(
        groups.groups.map((HeldGroup group) => group.apex),
        containsAll(<String>['registry.npmjs.org', 'npmjs.org']),
      );
    });

    test('group_summary_without_a_typical_carries_no_catalog_line', () {
      // Ein Eintrag ohne `typical` ergibt keine Katalogzeile, und dann steht
      // die Zusammenfassung ohne die Hälfte da, statt mit einem „Looks like: ",
      // hinter dem nichts kommt.
      const CatalogEntry nameless = CatalogEntry(
        id: 'npm',
        name: 'npm registry',
        category: 'registry',
      );
      final HeldGroup group = groupFlows(<Flow>[npmFlow(1), npmFlow(2)])
          .groups
          .single;

      expect(groupLooksLike(nameless, l10n), '');
      final String summary = groupSummary(group, nameless, l10n);
      expect(summary, contains('npm registry'));
      expect(summary, isNot(contains('Looks like')));
      expect(
        summary,
        isNot(contains(' ·  · ')),
        reason: 'keine Trennzeichen um eine Zeile, die es nicht gibt',
      );
    });
  });

  testWidgets('the header of a group names the service, not a host', (
    WidgetTester tester,
  ) async {
    final List<FlowDetail> details = <FlowDetail>[
      for (int i = 1; i <= 3; i++) detailFor(npmFlow(i), apex: 'npmjs.org'),
    ];
    await pumpIntercept(tester, client: fakeDaemon(holdScript(details)));
    await playScript(tester);
    await tester.pumpAndSettle();

    // Rot, sobald `groupTitle` wieder auf `group.display` zurückfällt: Dann
    // stünde der Host im Kopf, den die Zeilen darunter ohnehin tragen.
    final Finder header = find.byKey(const Key('queue-group-npmjs.org'));
    expect(header, findsOneWidget);
    expect(
      find.descendant(of: header, matching: find.text('npm registry')),
      findsOneWidget,
    );
    expect(
      find.descendant(of: header, matching: find.text('registry.npmjs.org')),
      findsNothing,
    );
  });

  testWidgets('quick_rule_calls_rules_add', (WidgetTester tester) async {
    final FlowDetail detail = detailFor(npmFlow(1), apex: 'npmjs.org');
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[detail]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester);

    final ProviderContainer container = containerOf(tester);
    container.read(selectedFlowIdProvider.notifier).select(detail.summary.id);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('intercept-quick-rule-allow-apex')));
    await tester.pumpAndSettle();

    // Rot, sobald die Schnellregel über `Decide` liefe: Sie darf die Anfrage
    // auf dem Tisch nicht entscheiden.
    expect(client.decisions, isEmpty);
    final List<Rule> created = (await client.listRules()).rules
        .where((Rule rule) => rule.createdFrom == detail.summary.id)
        .toList();
    expect(created, hasLength(1), reason: 'genau ein `Rules(add)`');
    final Rule rule = created.single;
    expect(rule.action, RuleAction.allow);
    expect(rule.matcher.host, '**.npmjs.org');
    expect(rule.expires, const RuleExpiry.session());
  });

  testWidgets('a quick rule for the domain is absent without an apex', (
    WidgetTester tester,
  ) async {
    final FlowDetail detail = detailFor(npmFlow(1, apex: ''));
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[detail])),
    );
    await playScript(tester);
    containerOf(tester)
        .read(selectedFlowIdProvider.notifier)
        .select(detail.summary.id);
    await tester.pumpAndSettle();

    // Ohne Domäne des Daemons entstünde keine Regel; ein Knopf, der nichts
    // anlegt, verspräche etwas.
    expect(
      find.byKey(const Key('intercept-quick-rule-allow-apex')),
      findsNothing,
    );
    expect(
      find.byKey(const Key('intercept-quick-rule-allow-host')),
      findsOneWidget,
    );
  });

  testWidgets('the known card names the service and what is typical', (
    WidgetTester tester,
  ) async {
    final FlowDetail detail = detailFor(npmFlow(1), apex: 'npmjs.org');
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[detail])),
    );
    await playScript(tester);
    containerOf(tester)
        .read(selectedFlowIdProvider.notifier)
        .select(detail.summary.id);
    await tester.pumpAndSettle();

    // Rot, sobald die Karte eine andere Sprache liest, den Katalog nicht
    // nachschlägt oder die Unbekannt-Karte für ein bekanntes Ziel zeichnet.
    expect(find.byKey(const Key('intercept-domain-name')), findsOneWidget);
    expect(find.text('Package registry'), findsOneWidget);
    expect(find.text('Typical for: npm install'), findsOneWidget);
    expect(find.byKey(const Key('intercept-domain-unknown')), findsNothing);
  });

  testWidgets('the unknown card stands for a host the catalog misses', (
    WidgetTester tester,
  ) async {
    final FlowDetail detail = detailFor(
      heldFlow(
        n: 9,
        deadline: testStart.add(const Duration(minutes: 5)),
        host: 'evil.example',
      ),
    );
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[detail])),
    );
    await playScript(tester);
    containerOf(tester)
        .read(selectedFlowIdProvider.notifier)
        .select(detail.summary.id);
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('intercept-domain-unknown')), findsOneWidget);
    expect(find.byKey(const Key('intercept-domain-name')), findsNothing);
    final HButton preview = tester.widget<HButton>(
      find.byKey(const Key('intercept-domain-preview')),
    );
    expect(preview.onPressed, isNull, reason: 'nichts wird von selbst geholt');
  });

  testWidgets('group_modal_still_asks_when_one_name_covers_two_hosts', (
    WidgetTester tester,
  ) async {
    // Der Fall, den dieses Issue neu schafft: Zwei Hosts tragen dieselbe
    // Kennung, der Kopf nennt deshalb einen Dienst, und die Gruppe sieht aus
    // wie ein Ding. Sie ist keines. Zwei Anfragen liegen unter
    // `modalAboveReach`, das Modal erscheint also allein wegen der zwei Hosts
    // — und genau das muss so bleiben: Wer über mehrere Hosts entscheidet,
    // soll sie lesen (`backlog/CONVENTIONS.md` 4.13, 4.15).
    //
    // Rot, sobald `_reasonToAsk` eine Gruppe mit gemeinsamer Kennung durchwinkt.
    final List<FlowDetail> details = <FlowDetail>[
      detailFor(npmFlow(1), apex: 'npmjs.org'),
      detailFor(npmFlow(2, host: 'registry.npmjs.com'), apex: 'npmjs.org'),
    ];
    await pumpIntercept(tester, client: fakeDaemon(holdScript(details)));
    await playScript(tester);

    final ProviderContainer container = containerOf(tester);
    final List<Flow> held = container.read(heldFlowsProvider);
    expect(held, hasLength(2));
    expect(
      held.length,
      lessThanOrEqualTo(modalAboveReach),
      reason: 'die Zahl allein löst das Modal hier nicht aus',
    );
    expect(held.map((Flow flow) => flow.catalogId).toSet(), <String>{
      'npm',
    }, reason: 'beide tragen denselben Dienst, der Kopf nennt ihn');
    expect(container.read(heldGroupsProvider).groups.single.catalogId, 'npm');

    await container
        .read(interceptDecisionProvider.notifier)
        .allowMany(held, remember: false);
    await tester.pump();

    expect(
      find.byType(BatchModal),
      findsOneWidget,
      reason:
          'zwei Hosts unter einem Dienstnamen sind zwei Hosts; das Modal '
          'listet sie auf, bevor etwas hinausgeht',
    );
    final BatchModal modal = tester.widget<BatchModal>(find.byType(BatchModal));
    expect(modal.request.hostCount, 2, reason: 'das Modal zählt beide Hosts');
  });
}
