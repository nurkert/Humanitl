// Goldens der Isolation (HUM-041): der Ring im Header in seinen drei
// Zuständen und der Reiter darunter. Erneuern mit
// `flutter test --update-goldens test/goldens`.
//
// Der Ring steht allein in seinem eigenen Bild, weil er 20 px misst und in
// einem Fenster von 1280 px kein Golden trägt: eine Farbe, die dort umkippt,
// wäre in der Shell nicht zu sehen.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../features/sandbox/harness.dart';

/// Wie groß der Ring im Golden gezeichnet wird.
///
/// Vier Mal die 20 px des Headers: das Golden prüft die Farbe je Bogen und
/// die Lücken dazwischen, und bei 20 px entscheidet ein einzelnes Pixel
/// darüber, ob ein Unterschied gesehen wird.
const double ringGoldenSize = 80;

/// Ein Fake, dessen Sandbox mit [checks] läuft.
FakeDaemonClient ringClient(List<IsolationCheckResult> checks) {
  final FakeDaemonClient client = FakeDaemonClient(
    script: const <ScriptedEvent>[],
  );
  client
    ..isolationChecks = checks
    ..sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      agentRunning: true,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
  return client;
}

/// Der Ring allein, über einem Fake mit genau diesen Ergebnissen.
Widget ringUnderTest({
  required FakeDaemonClient client,
  required HThemeMode mode,
}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  return ProviderScope(
    overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
    child: WidgetsApp(
      color: tokens.colors.bg0,
      debugShowCheckedModeBanner: false,
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      builder: (BuildContext context, Widget? _) => HTheme(
        tokens: tokens,
        child: ColoredBox(
          color: tokens.colors.bg1,
          // Wie `app.dart`: ein Overlay für alles, was schwebt; die
          // Hover-Beschriftung des Rings braucht eines.
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(
                builder: (BuildContext context) =>
                    const Center(child: IsolationRing(size: ringGoldenSize)),
              ),
            ],
          ),
        ),
      ),
    ),
  );
}

void main() {
  const BoxConstraints ringWindow = BoxConstraints.tightFor(
    width: 160,
    height: 160,
  );
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
    // Nichts gemessen: drei graue Bögen. Kein Grün, kein Anteil, keine Zahl.
    goldenTest(
      'isolation_ring_unknown_$name',
      fileName: 'isolation_ring_unknown_$name',
      constraints: ringWindow,
      pumpBeforeTest: _settle,
      builder: () =>
          ringUnderTest(client: FakeDaemonClient.empty(), mode: mode),
    );

    // Drei gemessene Garantien: der Ring schließt.
    goldenTest(
      'isolation_ring_passed_$name',
      fileName: 'isolation_ring_passed_$name',
      constraints: ringWindow,
      pumpBeforeTest: _settle,
      builder: () =>
          ringUnderTest(client: ringClient(isolationGreenChecks), mode: mode),
    );

    // Eine rote in der Mitte: der wichtigste Fall, und er muss auffallen.
    goldenTest(
      'isolation_ring_failed_$name',
      fileName: 'isolation_ring_failed_$name',
      constraints: ringWindow,
      pumpBeforeTest: _settle,
      builder: () =>
          ringUnderTest(client: ringClient(isolationOneRedCheck), mode: mode),
    );

    goldenTest(
      'isolation_panel_passed_$name',
      fileName: 'isolation_panel_passed_$name',
      constraints: window,
      pumpBeforeTest: _openIsolation,
      builder: () => sandboxUnderTest(
        client: checkedClient(isolationGreenChecks),
        mode: mode,
      ),
    );

    goldenTest(
      'isolation_panel_failed_$name',
      fileName: 'isolation_panel_failed_$name',
      constraints: window,
      pumpBeforeTest: _openIsolation,
      builder: () => sandboxUnderTest(
        client: checkedClient(isolationOneRedCheck),
        mode: mode,
      ),
    );

    // Der Bericht kam nicht an. Das ist die Form, die der Daemon dafür
    // wirklich schickt: drei rote Ergebnisse mit `SANDBOX_013`, nicht drei
    // graue und nicht gar keines.
    goldenTest(
      'isolation_panel_no_report_$name',
      fileName: 'isolation_panel_no_report_$name',
      constraints: window,
      pumpBeforeTest: _openIsolation,
      builder: () => sandboxUnderTest(
        client: checkedClient(isolationNoReportChecks),
        mode: mode,
      ),
    );
  }
}

/// Ein Frame für die Anfrage, einer für die Antwort, dann die Wartefrist von
/// `HWait` und der Versatz der drei Punkte.
Future<void> _settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 900));
}

/// Dasselbe, danach der Reiter mit den drei Garantien.
Future<void> _openIsolation(WidgetTester tester) async {
  await _settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-tab-isolation')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 900));
}
