// Die Release Valve mit offenem Fund sieht nicht aus wie eine Routineanfrage
// (`docs/UX.md` 4.7). Seit HUM-260 ist die Warnfarbe das Gelb der Marke und
// gleich dem Akzent; die Farbe allein unterscheidet die beiden nicht mehr.
// Das tut das Warnglyph vor der Beschriftung.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';

import 'fixtures.dart';
import 'harness.dart';

const Key warningKey = Key('intercept-valve-warning');

void main() {
  testWidgets('the valve carries the warning glyph only with a finding', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = fakeDaemon(
      holdScript(<FlowDetail>[
        held(1, path: '/clean'),
        detailFor(
          heldFlow(
            n: 2,
            deadline: testStart.add(const Duration(minutes: 5)),
            host: 'leaky.example',
            apex: 'leaky.example',
            path: '/leaky',
          ).copyWith(findingCount: 1),
          apex: 'leaky.example',
          findings: <Finding>[testFinding()],
        ),
      ]),
    );
    await pumpIntercept(tester, client: client);
    await playScript(tester, const Duration(seconds: 1));
    final container = containerOf(tester);

    container.read(selectedFlowIdProvider.notifier).select(testFlowId(1));
    await playScript(tester);
    expect(find.byKey(const Key('intercept-allow')), findsOneWidget);
    expect(find.byKey(warningKey), findsNothing);

    container.read(selectedFlowIdProvider.notifier).select(testFlowId(2));
    await playScript(tester);
    expect(find.byKey(warningKey), findsOneWidget);
    // Das Glyph steht im Ventil selbst, vor der Beschriftung.
    expect(
      find.descendant(
        of: find.byKey(const Key('intercept-allow')),
        matching: find.byKey(warningKey),
      ),
      findsOneWidget,
    );
    expect(
      tester.getTopLeft(find.byKey(warningKey)).dx,
      lessThan(
        tester.getTopLeft(find.byKey(const Key('intercept-valve-label'))).dx,
      ),
    );
  });
}
