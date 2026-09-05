// Goldens des Intercept-Screens (HUM-020): Queue-Zeile in drei Zuständen,
// Request-Karte, leere Queue und drei angehaltene Anfragen, je dunkel und
// hell. Erneuern mit `flutter test --update-goldens test/goldens`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/diagnostics.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/intercept/widgets/request_card.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../features/intercept/fixtures.dart';

/// Die Uhr aller Goldens: 40 Sekunden nach dem Anhalten.
final DateTime goldenNow = testStart.add(const Duration(seconds: 40));

/// Drei angehaltene Anfragen mit verschiedenen Methoden und Fristen.
List<FlowDetail> goldenDetails() => <FlowDetail>[
  detailFor(
    heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(minutes: 5)),
      method: Method.post,
      host: 'api.github.com',
      path: '/graphql?first=20',
      requestSize: 428,
    ),
    headers: <Header>[
      header('authorization', 'Bearer ghp_R8kQexample'),
      header('content-type', 'application/json'),
      header('user-agent', 'opencode/0.4.2'),
    ],
    bodyPreview: '{"query": "mutation { createIssue(title: \\"bug\\") }"}',
    contentType: 'application/json',
  ),
  detailFor(
    heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(minutes: 8)),
      host: 'registry.npmjs.org',
      path: '/react/-/react-19.2.0.tgz',
    ),
  ),
  detailFor(
    heldFlow(
      n: 3,
      deadline: testStart.add(const Duration(minutes: 12)),
      method: Method.delete,
      host: 'storage.googleapis.com',
      path: '/humanitl-cache/very/long/path/to/object-42.json',
      requestSize: 96,
    ),
  ),
];

/// Ein Container mit fester Uhr, fester Queue und einem Client, der die
/// Details der Karte beantwortet.
List<Override> overridesFor(List<FlowDetail> details) {
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
  ];
}

/// Ein Widget im Theme [mode], mit Lokalisierung und Providern.
Widget piece({
  required HThemeMode mode,
  required List<FlowDetail> details,
  required Widget child,
}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  return ProviderScope(
    overrides: overridesFor(details),
    child: WidgetsApp(
      color: tokens.colors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      // Wie `app.dart`: kein Navigator, ein Overlay für alles, was
      // schwebt.
      builder: (BuildContext context, Widget? _) => HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: tokens.colors.bg1,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(
                builder: (BuildContext context) => Padding(
                  padding: const EdgeInsets.all(HSpace.x3),
                  child: child,
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

/// Die ganze App mit fester Queue.
///
/// [found] steht über der Warteschlange; leer heißt: kein Streifen.
Widget screen({
  required HThemeMode mode,
  required List<FlowDetail> details,
  List<SessionDiagnostic> found = const <SessionDiagnostic>[],
}) => ProviderScope(
  overrides: <Override>[
    ...overridesFor(details),
    themeModeProvider.overrideWith(() => FixedTheme(mode)),
    if (found.isNotEmpty)
      diagnosticsProvider.overrideWith(() => FixedDiagnostics(found)),
  ],
  child: const HumanitlApp(),
);

/// Ein Befund, der nicht aus dem Strom kommt, sondern feststeht.
class FixedDiagnostics extends Diagnostics {
  /// Hält [found].
  FixedDiagnostics(this.found);

  /// Die Befunde des Goldens.
  final List<SessionDiagnostic> found;

  @override
  List<SessionDiagnostic> build() => found;
}

/// Das `TLS_001` des Standard-Szenarios, an einem Fluss.
SessionDiagnostic goldenDiagnostic() => SessionDiagnostic(
  id: 0,
  at: goldenNow,
  flowId: const FlowId('018f0001-0000-7000-8000-000000060000'),
  diagnostic: const Diagnostic(
    code: 'TLS_001',
    severity: Severity.warning,
    why: 'curl in the sandbox does not trust the Humanitl CA yet',
    fix: FixAction.setEnv(key: 'CURL_CA_BUNDLE', value: '/etc/humanitl/ca.crt'),
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

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );
  const BoxConstraints rowBox = BoxConstraints.tightFor(width: 420, height: 92);
  const BoxConstraints cardBox = BoxConstraints.tightFor(
    width: 680,
    height: 460,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    goldenTest(
      'queue_row_idle_$name',
      fileName: 'queue_row_idle_$name',
      constraints: rowBox,
      builder: () {
        final List<FlowDetail> details = goldenDetails();
        return piece(
          mode: mode,
          details: details,
          child: QueueRow(
            flow: details.first.summary,
            selected: false,
            onSelect: () {},
          ),
        );
      },
    );

    goldenTest(
      'queue_row_selected_$name',
      fileName: 'queue_row_selected_$name',
      constraints: rowBox,
      builder: () {
        final List<FlowDetail> details = goldenDetails();
        return piece(
          mode: mode,
          details: details,
          child: QueueRow(
            flow: details.first.summary,
            selected: true,
            onSelect: () {},
          ),
        );
      },
    );

    goldenTest(
      'queue_row_hover_$name',
      fileName: 'queue_row_hover_$name',
      constraints: rowBox,
      pumpBeforeTest: (WidgetTester tester) async {
        await tester.pumpAndSettle();
        final TestGesture gesture = await tester.createGesture(
          kind: PointerDeviceKind.mouse,
        );
        await gesture.addPointer(location: Offset.zero);
        addTearDown(gesture.removePointer);
        await gesture.moveTo(tester.getCenter(find.byType(QueueRow)));
        await tester.pumpAndSettle();
      },
      builder: () {
        final List<FlowDetail> details = goldenDetails();
        return piece(
          mode: mode,
          details: details,
          child: QueueRow(
            flow: details.first.summary,
            selected: false,
            onSelect: () {},
          ),
        );
      },
    );

    goldenTest(
      'request_card_basic_$name',
      fileName: 'request_card_basic_$name',
      constraints: cardBox,
      builder: () {
        final List<FlowDetail> details = goldenDetails();
        return piece(
          mode: mode,
          details: details,
          child: RequestCard(flow: details.first.summary),
        );
      },
    );

    goldenTest(
      'intercept_empty_$name',
      fileName: 'intercept_empty_$name',
      constraints: window,
      builder: () => screen(mode: mode, details: const <FlowDetail>[]),
    );

    goldenTest(
      'intercept_three_held_$name',
      fileName: 'intercept_three_held_$name',
      constraints: window,
      builder: () => screen(mode: mode, details: goldenDetails()),
    );

    // Der Befund des Daemons über der Warteschlange: Code, Titel der
    // Anwendung, der Satz des Daemons, das Abzeichen und die Kopierzeile.
    goldenTest(
      'intercept_diagnostic_tls_$name',
      fileName: 'intercept_diagnostic_tls_$name',
      constraints: window,
      builder: () => screen(
        mode: mode,
        details: goldenDetails(),
        found: <SessionDiagnostic>[goldenDiagnostic()],
      ),
    );
  }
}
