// Goldens der Gruppierung (HUM-029) und der Notiz (HUM-072): eine Gruppe
// eingeklappt und aufgeklappt, die Pille der wartenden Ankünfte und die
// Aktionsleiste mit offenem Notizfeld, je dunkel und hell.
//
// Erneuern mit `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/providers/note.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/providers/queue_freeze.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/theme.dart';

import '../features/intercept/fixtures.dart';

/// Die Uhr aller Goldens: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Ein Schwall an eine Domain plus zwei einzelne Anfragen.
List<FlowDetail> burstDetails() => <FlowDetail>[
  for (int i = 1; i <= 12; i++)
    detailFor(
      heldFlow(
        n: i,
        deadline: testStart.add(Duration(minutes: 5, seconds: i)),
        method: i.isEven ? Method.get : Method.post,
        host: 'registry.npmjs.org',
        path: '/react/-/react-19.$i.tgz',
        requestSize: 128 * i,
      ),
    ),
  detailFor(
    heldFlow(
      n: 20,
      deadline: testStart.add(const Duration(minutes: 9)),
      method: Method.post,
      host: 'api.github.com',
      path: '/graphql?first=20',
      requestSize: 428,
    ).copyWith(findingCount: 1),
    apex: 'github.com',
    findings: <Finding>[testFinding()],
  ),
];

/// Ein Container mit fester Uhr und fester Queue.
List<Override> overridesFor(
  List<FlowDetail> details, {
  List<Override> extra = const <Override>[],
}) {
  final TestDaemonClient client = TestDaemonClient();
  for (final FlowDetail detail in details) {
    client.details[detail.summary.id] = detail;
  }
  return <Override>[
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    nowProvider.overrideWith(() => FixedNow(goldenNow)),
    flowsProvider.overrideWith(
      () => FixedFlows(<FlowId, Flow>{
        for (final FlowDetail detail in details)
          detail.summary.id: detail.summary,
      }),
    ),
    ...extra,
  ];
}

/// Die ganze App mit fester Queue.
Widget screen({
  required HThemeMode mode,
  required List<FlowDetail> details,
  List<Override> extra = const <Override>[],
  TextScaler textScaler = TextScaler.noScaling,
}) => ProviderScope(
  overrides: <Override>[
    ...overridesFor(details, extra: extra),
    themeModeProvider.overrideWith(() => FixedTheme(mode)),
  ],
  child: MediaQuery(
    data: MediaQueryData(textScaler: textScaler),
    child: const HumanitlApp(),
  ),
);

/// Ein Theme, das nicht umschaltet.
class FixedTheme extends ThemeModeSetting {
  /// Bleibt bei [mode].
  FixedTheme(this.mode);

  /// Das feste Theme.
  final HThemeMode mode;

  @override
  HThemeMode build() => mode;
}

/// Eine Gruppe, die offen steht.
class OpenGroups extends ExpandedGroups {
  @override
  Map<String, bool> build() => const <String, bool>{'npmjs.org': true};
}

/// Drei Ankünfte, die auf das lesende Auge warten.
class WaitingArrivals extends PendingArrivals {
  @override
  Set<FlowId> build() => <FlowId>{
    testFlowId(101),
    testFlowId(102),
    testFlowId(103),
  };
}

/// Ein Notizfeld, das offen steht und schon Text trägt.
class OpenNote extends BlockNote {
  @override
  NoteDraft build() =>
      const NoteDraft(open: true, text: 'use PyPI, not GitHub');
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
      'queue_grouped_collapsed_$name',
      fileName: 'queue_grouped_collapsed_$name',
      constraints: window,
      builder: () => screen(mode: mode, details: burstDetails()),
    );

    goldenTest(
      'queue_grouped_expanded_$name',
      fileName: 'queue_grouped_expanded_$name',
      constraints: window,
      builder: () => screen(
        mode: mode,
        details: burstDetails(),
        extra: <Override>[expandedGroupsProvider.overrideWith(OpenGroups.new)],
      ),
    );

    goldenTest(
      'queue_new_pill_$name',
      fileName: 'queue_new_pill_$name',
      constraints: window,
      builder: () => screen(
        mode: mode,
        details: burstDetails(),
        extra: <Override>[
          pendingArrivalsProvider.overrideWith(WaitingArrivals.new),
        ],
      ),
    );

    goldenTest(
      'queue_grouped_scale2_$name',
      fileName: 'queue_grouped_scale2_$name',
      constraints: window,
      builder: () => screen(
        mode: mode,
        details: burstDetails(),
        extra: <Override>[expandedGroupsProvider.overrideWith(OpenGroups.new)],
        // Zeile und Gruppenkopf bei doppelter Textskalierung, ohne Overflow
        // und ohne abgeschnittenen Satz (`docs/UX.md` 6).
        textScaler: const TextScaler.linear(2),
      ),
    );

    goldenTest(
      'action_bar_note_$name',
      fileName: 'action_bar_note_$name',
      constraints: window,
      builder: () => screen(
        mode: mode,
        details: burstDetails(),
        extra: <Override>[blockNoteProvider.overrideWith(OpenNote.new)],
      ),
    );
  }
}
