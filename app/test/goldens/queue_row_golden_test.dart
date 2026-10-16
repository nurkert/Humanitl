// Goldens der Queue-Zeile (HUM-020, HUM-029, HUM-054): gehalten ohne und mit
// Funden, ausgewählt, unter dem Zeiger, und der Kopf einer Gruppe. Je in den
// vier Varianten aus `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../features/intercept/fixtures.dart';
import '../harness/ui_state.dart';
import 'support/golden_variants.dart';

/// Die Uhr aller Bilder dieser Datei: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Eine GET-Anfrage ohne Fund, 42 Sekunden vor ihrer Frist.
Flow heldGet() => heldFlow(
  n: 1,
  deadline: goldenNow.add(const Duration(seconds: 42)),
  host: 'api.github.com',
  apex: 'github.com',
  path: '/repos/acme/api/issues?state=open',
);

/// Eine POST-Anfrage mit zwei Funden.
Flow heldPostWithFindings() => heldFlow(
  n: 2,
  deadline: goldenNow.add(const Duration(minutes: 4, seconds: 20)),
  method: Method.post,
  host: 'api.github.com',
  apex: 'github.com',
  path: '/graphql?first=20',
  requestSize: 428,
).copyWith(findingCount: 2);

/// Vier gehaltene Anfragen an github.com, über zwei Hosts.
List<Flow> githubBurst() => <Flow>[
  for (int i = 0; i < 4; i++)
    heldFlow(
      n: 10 + i,
      deadline: goldenNow.add(Duration(minutes: 3, seconds: i)),
      method: i.isEven ? Method.get : Method.post,
      host: i < 3 ? 'api.github.com' : 'uploads.github.com',
      apex: 'github.com',
      path: '/repos/acme/api/contents/$i',
      requestSize: 96 * (i + 1),
    ).copyWith(catalogId: 'github', findingCount: i == 1 ? 1 : 0),
];

/// Feste Uhr, feste Queue und ein Client, der die Details beantwortet.
List<Override> overridesFor(List<Flow> flows) {
  final TestDaemonClient client = TestDaemonClient();
  for (final Flow flow in flows) {
    client.details[flow.id] = detailFor(flow, apex: flow.apex);
  }
  return <Override>[
    uiStateOverride(),
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    nowProvider.overrideWith(() => FixedNow(goldenNow)),
    flowsProvider.overrideWith(
      () => FixedFlows(<FlowId, Flow>{
        for (final Flow flow in flows) flow.id: flow,
      }),
    ),
  ];
}

/// Eine Zeile in natürlicher Höhe, oben im Bild.
Widget row(GoldenVariant variant, Flow flow, {bool selected = false}) =>
    goldenFrame(
      variant: variant,
      overrides: overridesFor(<Flow>[flow]),
      padding: const EdgeInsets.all(HSpace.x3),
      child: Align(
        alignment: Alignment.topCenter,
        child: QueueRow(flow: flow, selected: selected, onSelect: () {}),
      ),
    );

/// Fährt mit der Maus auf die Zeile und lässt die Überblendung auslaufen.
Future<void> hoverRow(WidgetTester tester) async {
  await tester.pumpAndSettle();
  final TestGesture gesture = await tester.createGesture(
    kind: PointerDeviceKind.mouse,
  );
  await gesture.addPointer(location: Offset.zero);
  addTearDown(gesture.removePointer);
  await gesture.moveTo(tester.getCenter(find.byType(QueueRow)));
  await tester.pumpAndSettle();
}

void main() {
  const BoxConstraints rowBox = BoxConstraints.tightFor(width: 420, height: 92);

  goldenVariants(
    'queue_row_held',
    constraints: rowBox,
    builder: (GoldenVariant variant) => row(variant, heldGet()),
  );

  goldenVariants(
    'queue_row_held_findings',
    constraints: rowBox,
    builder: (GoldenVariant variant) => row(variant, heldPostWithFindings()),
  );

  goldenVariants(
    'queue_row_selected',
    constraints: rowBox,
    builder: (GoldenVariant variant) =>
        row(variant, heldPostWithFindings(), selected: true),
  );

  goldenVariants(
    'queue_row_hover',
    constraints: rowBox,
    pumpBeforeTest: hoverRow,
    builder: (GoldenVariant variant) => row(variant, heldGet()),
  );

  goldenVariants(
    'queue_group_header',
    constraints: rowBox,
    builder: (GoldenVariant variant) {
      final List<Flow> flows = githubBurst();
      return goldenFrame(
        variant: variant,
        overrides: overridesFor(flows),
        padding: const EdgeInsets.all(HSpace.x3),
        child: Align(
          alignment: Alignment.topCenter,
          child: GroupHeaderRow(
            group: groupFlows(flows).groups.single,
            open: false,
            selected: false,
            onToggle: () {},
            onSelect: () {},
          ),
        ),
      );
    },
  );
}
