// Goldens der Isolation (HUM-041): der Ring im Header in seinen drei
// Zuständen und der Reiter darunter. Seit HUM-054 stehen die drei Ringe
// nebeneinander in einem Bild und die Panels „bestanden" und „gescheitert" in
// den vier Varianten aus `support/golden_variants.dart`; der fehlende Bericht
// bleibt dunkel und hell.
//
// Erneuern mit `make goldens-update`.
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

import '../features/sandbox/harness.dart';
import 'support/golden_variants.dart';

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

/// Ein Ring über einem eigenen Container, damit drei Ringe mit drei
/// verschiedenen Messungen in einem Bild stehen können.
///
/// Ein verschachtelter `ProviderScope` reichte nicht: Die Provider, die vom
/// Client abhängen, nennen keine `dependencies` und würden im inneren Scope
/// nicht neu gebaut. Der Container gehört dem Widget und geht mit ihm; ein
/// `addTearDown` käme zu spät, nach der Prüfung auf offene Zeitgeber.
class IsolatedRing extends StatefulWidget {
  /// Ein Ring über [client].
  const IsolatedRing({required this.client, super.key});

  /// Der Fake mit den Ergebnissen dieses Rings.
  final FakeDaemonClient client;

  @override
  State<IsolatedRing> createState() => _IsolatedRingState();
}

class _IsolatedRingState extends State<IsolatedRing> {
  late final ProviderContainer _container = ProviderContainer(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(widget.client),
    ],
  );

  @override
  void dispose() {
    _container.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => UncontrolledProviderScope(
    container: _container,
    child: const IsolationRing(size: ringGoldenSize),
  );
}

/// Ein laufender Client, dessen Bericht den LLM-Endpunkt als Ausnahme nennt.
SandboxTestClient passedWithLlm() {
  final SandboxTestClient client = checkedClient(isolationGreenChecks);
  client.sandbox = client.sandbox.copyWith(llmEndpoint: '192.168.1.20:11434');
  return client;
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );
  // Höher als das Standardfenster: Der Reiter steht im unteren Teil des
  // Bildschirms, und erst so passen die drei Garantien, die Zeile mit der
  // Ausnahme und der Abschnitt der Verweigerungen ganz hinein.
  const BoxConstraints tall = BoxConstraints.tightFor(
    width: 1280,
    height: 1100,
  );

  // 3/3, 2/3 und 0/3 nebeneinander: drei gemessene Garantien, eine rote in
  // der Mitte, und der Bericht, der nicht ankam (drei rote mit
  // `SANDBOX_013`). Rechts daneben der Ring vor jeder Messung: drei graue
  // Bögen, kein Grün, kein Anteil.
  goldenVariants(
    'isolation_ring_states',
    constraints: const BoxConstraints.tightFor(width: 520, height: 160),
    pumpBeforeTest: _settle,
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
        children: <Widget>[
          IsolatedRing(client: ringClient(isolationGreenChecks)),
          IsolatedRing(client: ringClient(isolationOneRedCheck)),
          IsolatedRing(client: ringClient(isolationNoReportChecks)),
          IsolatedRing(client: FakeDaemonClient.empty()),
        ],
      ),
    ),
  );

  // Drei grüne Garantien und die Zeile, die den LLM-Endpunkt als die eine
  // Ausnahme nennt.
  goldenVariants(
    'isolation_panel_ok',
    constraints: tall,
    pumpBeforeTest: _openIsolation,
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: passedWithLlm(),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );

  goldenVariants(
    'isolation_panel_failed',
    constraints: tall,
    pumpBeforeTest: _openIsolation,
    builder: (GoldenVariant variant) => sandboxUnderTest(
      client: checkedClient(isolationOneRedCheck),
      mode: variant.mode,
      locale: variant.locale,
    ),
  );

  for (final (String name, HThemeMode mode) in <(String, HThemeMode)>[
    ('dark', HThemeMode.dark),
    ('light', HThemeMode.light),
  ]) {
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
