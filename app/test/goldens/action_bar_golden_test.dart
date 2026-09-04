// Goldens der Aktionsleiste (HUM-028): Ruhe, offenes Raster, schmaler Pane,
// haltende Release Valve und doppelte Textskalierung, je dunkel und hell.
// Erneuern mit `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../features/intercept/fixtures.dart';

/// Die Uhr aller Goldens: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Die Anfrage, über die entschieden wird.
FlowDetail goldenDetail() => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    path: '/graphql?first=20',
    requestSize: 428,
  ),
  bodyPreview: '{"query": "mutation { createIssue }"}',
  contentType: 'application/json',
);

/// Ein Entwurf, dessen Raster offen ist.
class OpenDraft extends RememberDraft {
  @override
  RememberState build() => const RememberState(open: true);
}

/// Ein Theme, das nicht umschaltet.
class FixedTheme extends ThemeModeSetting {
  /// Bleibt bei [mode].
  FixedTheme(this.mode);

  /// Das feste Theme.
  final HThemeMode mode;

  @override
  HThemeMode build() => mode;
}

List<Override> overridesFor(FlowDetail detail, {bool gridOpen = false}) {
  final TestDaemonClient client = TestDaemonClient();
  client.details[detail.summary.id] = detail;
  return <Override>[
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    nowProvider.overrideWith(() => FixedNow(goldenNow)),
    flowsProvider.overrideWith(
      () => FixedFlows(<FlowId, Flow>{detail.summary.id: detail.summary}),
    ),
    if (gridOpen) rememberDraftProvider.overrideWith(OpenDraft.new),
  ];
}

/// Ein Widget im Theme [mode], mit Lokalisierung und Providern.
Widget piece({
  required HThemeMode mode,
  required Widget child,
  bool gridOpen = false,
  TextScaler textScaler = TextScaler.noScaling,
}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  final FlowDetail detail = goldenDetail();
  return ProviderScope(
    overrides: overridesFor(detail, gridOpen: gridOpen),
    child: WidgetsApp(
      color: tokens.colors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      builder: (BuildContext context, Widget? _) => MediaQuery(
        data: MediaQueryData(textScaler: textScaler),
        child: HTheme(
          tokens: tokens,
          child: ColoredBox(
            color: tokens.colors.bg1,
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) =>
                      Align(alignment: Alignment.bottomCenter, child: child),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

void main() {
  const BoxConstraints wide = BoxConstraints.tightFor(width: 900, height: 160);
  const BoxConstraints narrow = BoxConstraints.tightFor(
    width: 480,
    height: 200,
  );
  // Das offene Raster bekommt immer eine eigene Zeile: acht Segmente messen
  // rund 800 px, und die hat kein Pane neben den beiden Entscheidungen frei.
  const BoxConstraints withGrid = BoxConstraints.tightFor(
    width: 900,
    height: 220,
  );
  const BoxConstraints valveBox = BoxConstraints.tightFor(
    width: 260,
    height: 60,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'action_bar_default_$name',
      fileName: 'action_bar_default_$name',
      constraints: wide,
      builder: () => piece(
        mode: mode,
        child: ActionBar(flow: goldenDetail().summary),
      ),
    );

    goldenTest(
      'action_bar_grid_open_$name',
      fileName: 'action_bar_grid_open_$name',
      constraints: withGrid,
      builder: () => piece(
        mode: mode,
        gridOpen: true,
        child: ActionBar(flow: goldenDetail().summary),
      ),
    );

    goldenTest(
      'action_bar_narrow_$name',
      fileName: 'action_bar_narrow_$name',
      constraints: narrow,
      builder: () => piece(
        mode: mode,
        child: ActionBar(flow: goldenDetail().summary),
      ),
    );

    goldenTest(
      'release_valve_holding_$name',
      fileName: 'release_valve_holding_$name',
      constraints: valveBox,
      builder: () => piece(
        mode: mode,
        child: Center(
          child: ReleaseValve(
            label: 'Allow',
            holdLabel: 'Allow for session',
            shortcutHint: 'Enter',
            semanticsValue: '04:20 left',
            optionsLabel: 'Duration and scope of the rule',
            onAllow: () {},
            onAllowRemembered: () {},
            onToggleOptions: () {},
            previewHold: 0.6,
          ),
        ),
      ),
    );
  }

  goldenTest(
    'action_bar_text_scale_2',
    fileName: 'action_bar_text_scale_2',
    constraints: const BoxConstraints.tightFor(width: 900, height: 320),
    builder: () => piece(
      mode: HThemeMode.dark,
      gridOpen: true,
      textScaler: const TextScaler.linear(2),
      child: ActionBar(flow: goldenDetail().summary),
    ),
  );
}
