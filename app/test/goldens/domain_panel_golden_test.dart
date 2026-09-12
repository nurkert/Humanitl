// Goldens des rechten Panes (HUM-031, HUM-094): die Katalog-Karte für ein
// Ziel, das der Daemon einem Eintrag zugeordnet hat, die Unbekannt-Karte für
// eines, das er keinem zuordnen konnte, und die Zusammenfassung der Sitzung
// ohne Auswahl.
//
// Erneuern mit `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/services.dart' show rootBundle;
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/widgets/domain_panel.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../features/intercept/fixtures.dart';
import '../harness/ui_state.dart';
import 'intercept_group_golden_test.dart' show FixedTheme, goldenNow;

/// Eine gehaltene Anfrage an `registry.npmjs.org`, wie der Daemon sie schickt:
/// mit registrierbarer Domäne und mit der Kennung des Katalogeintrags.
Flow npmFlow() => heldFlow(
  n: 1,
  deadline: goldenNow.add(const Duration(minutes: 5)),
  host: 'registry.npmjs.org',
  apex: 'npmjs.org',
  path: '/react/-/react-19.1.tgz',
  requestSize: 412,
).copyWith(catalogId: 'npm');

/// Eine Anfrage an ein Ziel, zu dem der Katalog nichts sagt.
Flow unknownFlow() => heldFlow(
  n: 2,
  deadline: goldenNow.add(const Duration(minutes: 5)),
  host: 'evil.example',
  path: '/collect',
  requestSize: 96,
);

/// Die Hosts, aus denen die Zusammenfassung ihre Liste baut.
List<Flow> sessionFlows() => <Flow>[
  for (int i = 0; i < 7; i++)
    heldFlow(
      n: 100 + i,
      deadline: goldenNow.add(Duration(minutes: 5, seconds: i)),
      host: 'registry.npmjs.org',
      apex: 'npmjs.org',
    ).copyWith(catalogId: 'npm'),
  for (int i = 0; i < 4; i++)
    heldFlow(
      n: 200 + i,
      deadline: goldenNow.add(Duration(minutes: 5, seconds: i)),
      host: 'api.github.com',
      apex: 'github.com',
    ).copyWith(catalogId: 'github'),
  for (int i = 0; i < 2; i++)
    heldFlow(
      n: 300 + i,
      deadline: goldenNow.add(Duration(minutes: 5, seconds: i)),
      host: 'pypi.org',
      apex: 'pypi.org',
    ).copyWith(catalogId: 'pypi'),
  heldFlow(
    n: 400,
    deadline: goldenNow.add(const Duration(minutes: 5)),
    host: 'crates.io',
    apex: 'crates.io',
  ).copyWith(catalogId: 'crates-io'),
  heldFlow(
    n: 401,
    deadline: goldenNow.add(const Duration(minutes: 5)),
    host: 'evil.example',
  ),
];

/// Das Pane allein, über einer festen Warteschlange.
Widget pane({
  required HThemeMode mode,
  required Flow? selected,
  List<Flow> queue = const <Flow>[],
}) {
  final TestDaemonClient client = TestDaemonClient();
  final List<Flow> flows = <Flow>[...queue, ?selected];
  for (final Flow flow in flows) {
    client.details[flow.id] = detailFor(flow, apex: flow.apex);
  }
  return ProviderScope(
    overrides: <Override>[
      uiStateOverride(),
      daemonClientProvider.overrideWithValue(client),
      flowsProvider.overrideWith(
        () => FixedFlows(<FlowId, Flow>{
          for (final Flow flow in flows) flow.id: flow,
        }),
      ),
      themeModeProvider.overrideWith(() => FixedTheme(mode)),
    ],
    child: HTheme(
      tokens: mode == HThemeMode.dark ? HTokens.dark : HTokens.light,
      child: Localizations(
        locale: const Locale('en'),
        delegates: AppLocalizations.localizationsDelegates,
        child: Directionality(
          textDirection: TextDirection.ltr,
          child: DomainPanel(flow: selected),
        ),
      ),
    ),
  );
}

void main() {
  // Das Pane liest den gebündelten Katalog; ein Future aus der Zeitzone eines
  // vorigen Tests hielte den nächsten auf (siehe `domain_panel_test.dart`).
  setUp(rootBundle.clear);

  const BoxConstraints window = BoxConstraints.tightFor(
    width: 320,
    height: 640,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'domain_panel_known_$name',
      fileName: 'domain_panel_known_$name',
      constraints: window,
      builder: () => pane(mode: mode, selected: npmFlow()),
    );

    goldenTest(
      'domain_panel_unknown_$name',
      fileName: 'domain_panel_unknown_$name',
      constraints: window,
      builder: () => pane(mode: mode, selected: unknownFlow()),
    );

    goldenTest(
      'domain_panel_session_summary_$name',
      fileName: 'domain_panel_session_summary_$name',
      constraints: window,
      builder: () => pane(mode: mode, selected: null, queue: sessionFlows()),
    );
  }
}
