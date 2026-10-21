// Goldens der Aktionsleiste (HUM-028): schmaler Pane, haltende Release Valve
// und doppelte Textskalierung, je dunkel und hell. Ruhe, offene Funde, harte
// Sperre und das offene Raster stehen seit HUM-054 in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

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
import '../harness/ui_state.dart';
import 'support/golden_variants.dart';

/// Die Uhr aller Goldens: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Die Anfrage, über die entschieden wird.
FlowDetail goldenDetail() => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    // Seit HUM-091 trägt die Zeile die registrierbare Domain selbst; das
    // Ziel „Domäne" steht damit ohne einen zweiten Aufruf zur Verfügung.
    apex: 'github.com',
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

/// Dieselbe Anfrage mit zwei Funden im Rumpf: einem Schlüssel und einer
/// Adresse.
FlowDetail findingsDetail() {
  final FlowDetail detail = goldenDetail();
  return detail.copyWith(
    summary: detail.summary.copyWith(findingCount: 2),
    findings: <Finding>[
      testFinding(),
      testFinding(kind: 'email'),
    ],
  );
}

/// Dieselbe Anfrage unter der harten Sperre (`HOLD_004`, HUM-159): eine
/// bestätigte Prüfsumme im Rumpf, gesendet wird nur bearbeitet.
FlowDetail hardBlockDetail() {
  final FlowDetail detail = findingsDetail();
  return detail.copyWith(
    summary: detail.summary.copyWith(
      findingCount: 1,
      sendRefusal: const Diagnostic(
        code: 'HOLD_004',
        severity: Severity.blocking,
        why:
            'the request carries a checksum-confirmed secret and '
            'hold.hard_block_checksum_secrets is on; nothing was sent',
      ),
    ),
    findings: <Finding>[testFinding()],
  );
}

List<Override> overridesFor(FlowDetail detail, {bool gridOpen = false}) {
  final TestDaemonClient client = TestDaemonClient();
  client.details[detail.summary.id] = detail;
  return <Override>[
    uiStateOverride(),
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

/// Die Leiste über [detail] in der Variante [variant], unten im Bild.
Widget variantBar(
  GoldenVariant variant,
  FlowDetail detail, {
  bool gridOpen = false,
}) => goldenFrame(
  variant: variant,
  overrides: overridesFor(detail, gridOpen: gridOpen),
  child: Align(
    alignment: Alignment.bottomCenter,
    child: ActionBar(flow: detail.summary),
  ),
);

void main() {
  const BoxConstraints wide = BoxConstraints.tightFor(width: 900, height: 160);
  const BoxConstraints narrow = BoxConstraints.tightFor(
    width: 480,
    height: 200,
  );
  // Das offene Raster bekommt immer eine eigene Zeile: acht Segmente messen
  // rund 800 px, und die hat kein Pane neben den beiden Entscheidungen frei.
  // Dazu kommt eine Zeile: Solange das Raster merkt, stehen beide Regelsätze
  // da, der für Senden und der für Blockieren (HUM-249).
  const BoxConstraints withGrid = BoxConstraints.tightFor(
    width: 900,
    height: 244,
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
            onActivate: () {},
            onAllowRemembered: () {},
            onToggleOptions: () {},
            previewHold: 0.6,
          ),
        ),
      ),
    );
  }

  goldenVariants(
    'action_bar_default',
    constraints: wide,
    builder: (GoldenVariant variant) => variantBar(variant, goldenDetail()),
  );

  // Offene Funde: die Freigabe in der Warnfarbe mit Warnglyph, mit eigener Beschriftung und der
  // Zeile, die sagt, was wohin ginge (`docs/UX.md` 4.7).
  goldenVariants(
    'action_bar_findings',
    constraints: wide,
    builder: (GoldenVariant variant) => variantBar(variant, findingsDetail()),
  );

  // Die harte Sperre: „Senden nicht möglich", abgeschaltet (HUM-159).
  goldenVariants(
    'action_bar_hard_block',
    constraints: wide,
    builder: (GoldenVariant variant) => variantBar(variant, hardBlockDetail()),
  );

  goldenVariants(
    'action_bar_remember_open',
    constraints: withGrid,
    builder: (GoldenVariant variant) =>
        variantBar(variant, goldenDetail(), gridOpen: true),
  );

  goldenTest(
    'action_bar_text_scale_2',
    fileName: 'action_bar_text_scale_2',
    // Bei doppelter Schrift brechen beide Regelsätze um (HUM-249).
    constraints: const BoxConstraints.tightFor(width: 900, height: 400),
    builder: () => piece(
      mode: HThemeMode.dark,
      gridOpen: true,
      textScaler: const TextScaler.linear(2),
      child: ActionBar(flow: goldenDetail().summary),
    ),
  );
}
