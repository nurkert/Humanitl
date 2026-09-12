// Die Gruppierung der Queue (HUM-029): nach registrierbarer Domain, sortiert
// nach der frühesten Frist, mit Methodenmix und Findings-Summe.
//
// Reine Funktion und reiner Provider: keine Uhr, kein Widget. Ein Provider,
// der gruppiert, sieht nie eine Uhr (docs/UX.md 7).

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';

import 'fixtures.dart';

/// Ein angehaltener Flow für [host], dessen Frist in [seconds] abläuft.
///
/// [apex] ist die registrierbare Domain, die der Daemon zu [host] nennt
/// (`FlowSummary.apex`, HUM-091); leer heißt „er weiß sie nicht", und die
/// Gruppe steht dann unter dem Host. Hier steht sie geschrieben, weil die
/// Public Suffix List im Daemon liegt und eine zweite Tabelle im Test genau
/// der Rat wäre, den dieses Issue abgeschafft hat.
Flow flowOf(
  int n, {
  required String host,
  String apex = '',
  int seconds = 300,
  Method method = Method.get,
  int findings = 0,
}) => heldFlow(
  n: n,
  deadline: testStart.add(Duration(seconds: seconds)),
  method: method,
  host: host,
  apex: apex,
  path: '/thing/$n',
).copyWith(findingCount: findings);

/// Ein Container über [flows].
ProviderContainer containerOver(List<Flow> flows) {
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      flowsProvider.overrideWith(
        () => FixedFlows(<FlowId, Flow>{
          for (final Flow flow in flows) flow.id: flow,
        }),
      ),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('groups_sorted_by_deadline', () {
    final ProviderContainer container = containerOver(<Flow>[
      flowOf(1, host: 'api.github.com', apex: 'github.com', seconds: 600),
      flowOf(2, host: 'registry.npmjs.org', apex: 'npmjs.org', seconds: 120),
      flowOf(3, host: 'registry.npmjs.org', apex: 'npmjs.org', seconds: 300),
      flowOf(4, host: 'codeload.github.com', apex: 'github.com', seconds: 900),
    ]);

    final List<HeldGroup> groups = container.read(heldGroupsProvider).groups;

    // Die früheste Frist steht oben, und die beiden GitHub-Hosts stehen unter
    // ihrer registrierbaren Domain.
    expect(groups.map((HeldGroup group) => group.apex).toList(), <String>[
      'npmjs.org',
      'github.com',
    ]);
    expect(groups.first.display, 'registry.npmjs.org');
    // Zwei Hosts: die Gruppe nennt keine Domain, obwohl der Daemon sie kennt.
    // Wer über mehrere Hosts entscheidet, soll sie lesen; der Kopf schreibt
    // deshalb den ersten Host und zählt den Rest (CONVENTIONS 4.13).
    expect(groups.last.display, isEmpty);
    expect(groups.last.hosts, <String>[
      'api.github.com',
      'codeload.github.com',
    ]);
    expect(
      groups.first.earliestDeadline,
      testStart.add(const Duration(seconds: 120)),
    );
  });

  test('a group counts findings and the method mix', () {
    final HeldGroups groups = groupFlows(<Flow>[
      flowOf(1, host: 'registry.npmjs.org', apex: 'npmjs.org', findings: 2),
      flowOf(
        2,
        host: 'registry.npmjs.org',
        apex: 'npmjs.org',
        method: Method.post,
      ),
      flowOf(3, host: 'registry.npmjs.org', apex: 'npmjs.org', findings: 1),
    ]);

    final HeldGroup npm = groups.groups.single;
    expect(npm.findingsTotal, 3);
    // Die häufigste Methode zuerst.
    expect(npm.methods, <String, int>{'GET': 2, 'POST': 1});
    expect(npm.length, 3);
  });

  test('one request stays a plain row, two become a group', () {
    final HeldGroups one = groupFlows(<Flow>[
      flowOf(1, host: 'pypi.org', apex: 'pypi.org'),
    ]);
    expect(one.groups.single.isBurst, isFalse);

    final HeldGroups two = groupFlows(<Flow>[
      flowOf(1, host: 'pypi.org', apex: 'pypi.org'),
      flowOf(2, host: 'files.pypi.org', apex: 'pypi.org'),
    ]);
    expect(two.groups.single.isBurst, isTrue);
    // Zwei passen unter ihren Kopf, drei sind der Schwall, für den es die
    // Gruppe gibt (HUM-029).
    expect(two.groups.single.openByDefault, isTrue);

    final HeldGroups three = groupFlows(<Flow>[
      flowOf(1, host: 'pypi.org', apex: 'pypi.org'),
      flowOf(2, host: 'pypi.org', apex: 'pypi.org'),
      flowOf(3, host: 'pypi.org', apex: 'pypi.org'),
    ]);
    expect(three.groups.single.openByDefault, isFalse);
  });

  test('an address is its own group; it has no apex', () {
    // Ob ein Host eine Adresse ist, weiß der Daemon und sagt es im
    // `Authority`; die Oberfläche rät es nicht (CONVENTIONS 4.13).
    final Flow address = flowOf(1, host: '127.0.0.1').copyWith(
      authority: const Authority(
        host: '127.0.0.1',
        port: 443,
        isIpLiteral: true,
      ),
    );
    final HeldGroups groups = groupFlows(<Flow>[
      address,
      flowOf(2, host: 'api.github.com', apex: 'github.com'),
    ]);

    expect(
      groups.groups.map((HeldGroup group) => group.apex).toList(),
      <String>['127.0.0.1', 'github.com'],
    );
  });

  test('the expansion keeps only what deviates from the default', () {
    final ProviderContainer container = containerOver(<Flow>[
      flowOf(1, host: 'registry.npmjs.org', apex: 'npmjs.org'),
      flowOf(2, host: 'registry.npmjs.org', apex: 'npmjs.org'),
      flowOf(3, host: 'registry.npmjs.org', apex: 'npmjs.org'),
    ]);
    final HeldGroup npm = container.read(heldGroupsProvider).groups.single;
    final ExpandedGroups expanded = container.read(
      expandedGroupsProvider.notifier,
    );

    expect(expanded.isOpen(npm), isFalse);
    expanded.toggle(npm);
    expect(expanded.isOpen(npm), isTrue);
    expanded.setOpen(npm, false);
    expect(expanded.isOpen(npm), isFalse);
  });

  test('the group of a flow is found by its id', () {
    final ProviderContainer container = containerOver(<Flow>[
      flowOf(1, host: 'registry.npmjs.org', apex: 'npmjs.org'),
      flowOf(2, host: 'api.github.com', apex: 'github.com'),
    ]);

    expect(
      container.read(heldGroupsProvider).groupOf(testFlowId(2))?.apex,
      'github.com',
    );
  });

  test('two registrants under one public suffix stay two groups', () {
    // Der Fall, der die geratene Tabelle erledigt hat: `com.pl` ist ein
    // Public Suffix, und `a.foo.com.pl` und `b.evil.com.pl` gehören zwei
    // Fremden. Der Daemon liest die Public Suffix List und nennt zwei
    // Domains; eine Entscheidung deckt deshalb nie beide (HUM-091).
    final HeldGroups groups = groupFlows(<Flow>[
      flowOf(1, host: 'a.foo.com.pl', apex: 'foo.com.pl'),
      flowOf(2, host: 'b.evil.com.pl', apex: 'evil.com.pl'),
    ]);

    expect(
      groups.groups.map((HeldGroup group) => group.apex).toList(),
      <String>['foo.com.pl', 'evil.com.pl'],
    );
  });

  test('a private suffix groups under the registrable domain', () {
    // `github.io` steht im privaten Abschnitt der Liste: `a.b.github.io` und
    // `c.b.github.io` gehören demselben Betreiber, `github.io` nicht.
    final HeldGroups groups = groupFlows(<Flow>[
      flowOf(1, host: 'a.b.github.io', apex: 'b.github.io'),
      flowOf(2, host: 'c.b.github.io', apex: 'b.github.io'),
    ]);

    final HeldGroup group = groups.groups.single;
    expect(group.apex, 'b.github.io');
    expect(group.hosts, <String>['a.b.github.io', 'c.b.github.io']);
    // Zwei Hosts: der Kopf nennt sie, nicht die Domain.
    expect(group.display, isEmpty);
  });

  test('four held requests to four hosts become three groups', () {
    // Das Akzeptanzkriterium von HUM-091, Zeile für Zeile: `com.pl` ist ein
    // Public Suffix, also sind `a.foo.com.pl` und `b.evil.com.pl` zwei
    // Registranten und zwei Gruppen; `github.io` steht im privaten Abschnitt
    // der Liste, also gehören `a.b.github.io` und `c.b.github.io` zu
    // `b.github.io` und in eine.
    final HeldGroups groups = groupFlows(<Flow>[
      flowOf(1, host: 'a.foo.com.pl', apex: 'foo.com.pl'),
      flowOf(2, host: 'b.evil.com.pl', apex: 'evil.com.pl'),
      flowOf(3, host: 'a.b.github.io', apex: 'b.github.io'),
      flowOf(4, host: 'c.b.github.io', apex: 'b.github.io'),
    ]);

    expect(groups.groups, hasLength(3));
    expect(
      groups.groups.map((HeldGroup group) => group.apex).toList(),
      <String>['foo.com.pl', 'evil.com.pl', 'b.github.io'],
    );
    expect(groups.groups.last.hosts, <String>[
      'a.b.github.io',
      'c.b.github.io',
    ]);
  });

  test('an empty apex stands for itself, one group per host', () {
    // Leer heißt „der Daemon weiß es nicht". Dann steht der Host da, und
    // nichts wird ergänzt: Zwei unbekannte Domains sind nicht dieselbe.
    final HeldGroups groups = groupFlows(<Flow>[
      flowOf(1, host: 'first.example'),
      flowOf(2, host: 'second.example'),
    ]);

    expect(
      groups.groups.map((HeldGroup group) => group.apex).toList(),
      <String>['first.example', 'second.example'],
    );
  });
}
