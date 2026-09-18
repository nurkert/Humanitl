// Das eine Prädikat der Pause mit offenen Funden (HUM-049).
//
// `findingsPauseVisibleProvider` liest die Aktionsleiste, wenn sie die Pause
// zeichnet, und der Bildschirm, wenn er `S`, `P` und `Esc` zulässt. Jede
// Bedingung, die hier fehlt, hieße: Die Pause ist unsichtbar, aber eine Taste
// wirkt noch auf sie.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/findings_pause.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/selection.dart';

import 'fixtures.dart';

Flow held(int n) =>
    heldFlow(n: n, deadline: testStart.add(const Duration(minutes: 5)));

/// Ein Container, dessen Auswahl der Test festlegt.
ProviderContainer containerFor({
  required Flow? selected,
  List<Flow>? selection,
  FindingSet findings = const FindingSet(count: 1),
}) {
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[
      selectedFlowProvider.overrideWithValue(selected),
      selectedFlowsProvider.overrideWithValue(
        QueueSnapshot(selection ?? <Flow>[?selected]),
      ),
      selectedFindingsProvider.overrideWithValue(findings),
    ],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('open over the one held request with an open finding', () {
    final Flow flow = held(1);
    final ProviderContainer container = containerFor(selected: flow);
    expect(container.read(findingsPauseVisibleProvider), isFalse);

    container.read(openFindingsPauseProvider.notifier).open(flow.id);

    expect(container.read(findingsPauseVisibleProvider), isTrue);
  });

  test('gone once no finding is open any more', () {
    final Flow flow = held(1);
    final ProviderContainer container = containerFor(
      selected: flow,
      findings: FindingSet.none,
    );
    container.read(openFindingsPauseProvider.notifier).open(flow.id);

    expect(container.read(findingsPauseVisibleProvider), isFalse);
  });

  test('gone once the request is no longer held', () {
    final Flow flow = held(1).copyWith(state: FlowState.decided);
    final ProviderContainer container = containerFor(selected: flow);
    container.read(openFindingsPauseProvider.notifier).open(flow.id);

    expect(container.read(findingsPauseVisibleProvider), isFalse);
  });

  test('gone over another request and over a selection of two', () {
    final Flow first = held(1);
    final Flow second = held(2);
    final ProviderContainer other = containerFor(selected: second);
    other.read(openFindingsPauseProvider.notifier).open(first.id);
    expect(other.read(findingsPauseVisibleProvider), isFalse);

    final ProviderContainer both = containerFor(
      selected: first,
      selection: <Flow>[first, second],
    );
    both.read(openFindingsPauseProvider.notifier).open(first.id);
    expect(both.read(findingsPauseVisibleProvider), isFalse);
  });
}
