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
Flow flowOf(
  int n, {
  required String host,
  int seconds = 300,
  Method method = Method.get,
  int findings = 0,
}) => heldFlow(
  n: n,
  deadline: testStart.add(Duration(seconds: seconds)),
  method: method,
  host: host,
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
      flowOf(1, host: 'api.github.com', seconds: 600),
      flowOf(2, host: 'registry.npmjs.org', seconds: 120),
      flowOf(3, host: 'registry.npmjs.org', seconds: 300),
      flowOf(4, host: 'codeload.github.com', seconds: 900),
    ]);

    final List<HeldGroup> groups = container.read(heldGroupsProvider).groups;

    // Die früheste Frist steht oben, und die beiden GitHub-Hosts stehen unter
    // ihrer registrierbaren Domain.
    expect(groups.map((HeldGroup group) => group.apex).toList(), <String>[
      'npmjs.org',
      'github.com',
    ]);
    expect(groups.first.display, 'registry.npmjs.org');
    // Zwei Hosts: die Gruppe nennt keine Domain, weil die Tabelle in
    // `psl.dart` sie nur rät (CONVENTIONS 4.13). Der Kopf schreibt statt
    // dessen den ersten Host und zählt den Rest.
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
      flowOf(1, host: 'registry.npmjs.org', findings: 2),
      flowOf(2, host: 'registry.npmjs.org', method: Method.post),
      flowOf(3, host: 'registry.npmjs.org', findings: 1),
    ]);

    final HeldGroup npm = groups.groups.single;
    expect(npm.findingsTotal, 3);
    // Die häufigste Methode zuerst.
    expect(npm.methods, <String, int>{'GET': 2, 'POST': 1});
    expect(npm.length, 3);
  });

  test('one request stays a plain row, two become a group', () {
    final HeldGroups one = groupFlows(<Flow>[flowOf(1, host: 'pypi.org')]);
    expect(one.groups.single.isBurst, isFalse);

    final HeldGroups two = groupFlows(<Flow>[
      flowOf(1, host: 'pypi.org'),
      flowOf(2, host: 'files.pypi.org'),
    ]);
    expect(two.groups.single.isBurst, isTrue);
    // Zwei passen unter ihren Kopf, drei sind der Schwall, für den es die
    // Gruppe gibt (HUM-029).
    expect(two.groups.single.openByDefault, isTrue);

    final HeldGroups three = groupFlows(<Flow>[
      flowOf(1, host: 'pypi.org'),
      flowOf(2, host: 'pypi.org'),
      flowOf(3, host: 'pypi.org'),
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
      flowOf(2, host: 'api.github.com'),
    ]);

    expect(
      groups.groups.map((HeldGroup group) => group.apex).toList(),
      <String>['127.0.0.1', 'github.com'],
    );
  });

  test('the expansion keeps only what deviates from the default', () {
    final ProviderContainer container = containerOver(<Flow>[
      flowOf(1, host: 'registry.npmjs.org'),
      flowOf(2, host: 'registry.npmjs.org'),
      flowOf(3, host: 'registry.npmjs.org'),
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
      flowOf(1, host: 'registry.npmjs.org'),
      flowOf(2, host: 'api.github.com'),
    ]);

    expect(
      container.read(heldGroupsProvider).groupOf(testFlowId(2))?.apex,
      'github.com',
    );
  });
}
