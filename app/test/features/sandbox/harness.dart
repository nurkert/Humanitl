// Gerüst der Sandbox-Tests: der Bildschirm über einem Fake-Daemon, ohne
// Shell, mit einem Verzeichnis-Dialog, den der Test steuert.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl/features/sandbox/widgets/work_dir_picker.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Der Zeitpunkt, gegen den jeder Test rechnet.
final DateTime sandboxTestNow = DateTime.utc(2026, 9, 4, 12);

/// Ein Fake ohne Skript; die Sandbox-Tests abonnieren nichts.
class SandboxTestClient extends FakeDaemonClient {
  /// Erzeugt den Client mit stehender Uhr.
  SandboxTestClient()
    : super(script: const <ScriptedEvent>[], clock: () => sandboxTestNow);

  /// Jede `Plan`-Anfrage, älteste zuerst.
  final List<(String?, WorkMode?)> plans = <(String?, WorkMode?)>[];

  /// Wie oft `Sandbox(Start)` gerufen wurde.
  int starts = 0;

  /// Wie oft `Sandbox(Stop)` gerufen wurde.
  int stops = 0;

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) {
    plans.add((workDir, workMode));
    return super.planSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
  }

  @override
  Stream<SandboxUpdate> startSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) {
    starts++;
    return super.startSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
  }

  @override
  Stream<SandboxUpdate> stopSandbox() {
    stops++;
    return super.stopSandbox();
  }
}

/// Ein Client, dessen Sandbox schon läuft.
SandboxTestClient runningClient({bool agentRunning = true}) {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.running,
    agentRunning: agentRunning,
    startedAt: sandboxTestNow,
    sandboxId: FakeDaemonClient.defaultSandbox,
  );
  return client;
}

/// Ein Client, dessen Start an einem blockierenden Befund scheitert.
SandboxTestClient blockedClient() {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(
    state: SandboxState.failed,
    diagnostics: const <Diagnostic>[blockingFinding],
  );
  return client;
}

/// Die drei Garantien, gemessen und belegt.
///
/// Die Evidenzzeilen sind die, die `humanitl sandbox check --json` am
/// 2026-09-04 auf dem Entwicklungsrechner geliefert hat; `limit=none` steht
/// hier, weil der abgebrochene Suchlauf sein eigener Fall ist.
const List<IsolationCheckResult> isolationGreenChecks = <IsolationCheckResult>[
  IsolationCheckResult(
    check: IsolationCheck.noNetworkInterface,
    passed: true,
    evidence: 'no_interfaces ok: lo',
  ),
  IsolationCheckResult(
    check: IsolationCheck.singleSocket,
    passed: true,
    evidence:
        'single_socket ok: sockets=/run/humanitl/proxy.sock;unexpected=none;'
        'entries=41;limit=none; '
        'bridge_listening ok: proxy=127.0.0.1:3128->/run/humanitl/proxy.sock',
  ),
  IsolationCheckResult(
    check: IsolationCheck.seccompActive,
    passed: true,
    evidence:
        'seccomp_applied ok: Seccomp:2;NoNewPrivs:1; '
        'families ok: socket(AF_UNIX,SOCK_STREAM)=EPERM;'
        'socket(AF_INET,SOCK_DGRAM)=EPERM;x32:socket=EPERM;'
        'io_uring_setup=EPERM;socket(AF_INET,SOCK_STREAM)=ok',
  ),
];

/// Der rote Fall: eine zweite Socket-Datei im Projektverzeichnis, der Befund
/// des Daemons daneben.
const IsolationCheckResult isolationRedSocketCheck = IsolationCheckResult(
  check: IsolationCheck.singleSocket,
  passed: false,
  evidence:
      'single_socket FAIL: sockets=/run/humanitl/proxy.sock,/work/agent.sock;'
      'unexpected=/work/agent.sock;entries=41;limit=none; '
      'bridge_listening ok: proxy=127.0.0.1:3128->/run/humanitl/proxy.sock',
  diagnostic: Diagnostic(
    code: DiagnosticCodes.isolationSingleSocket,
    severity: Severity.blocking,
    title: 'Isolation check 2: more than one door',
    // Der Wortlaut, den `check_from` in `bwrap.rs` baut: `why` ist der Name
    // der Garantie und die rohe Zeile des Shims. **Kein `fix`** -- die vier
    // Codes tragen heute keine Behebungs-Aktion, und eine Fixture, die eine
    // erfindet, prueft eine Karte, die der Daemon nie schickt.
    why:
        'single_socket: single_socket FAIL: '
        'sockets=/run/humanitl/proxy.sock,/work/agent.sock;'
        'unexpected=/work/agent.sock;entries=41;limit=none; '
        'bridge_listening ok: proxy=127.0.0.1:3128->/run/humanitl/proxy.sock',
  ),
);

/// Was der Daemon meldet, wenn der Shim keinen Bericht geliefert hat: drei
/// rote Ergebnisse mit `SANDBOX_013`.
final List<IsolationCheckResult> isolationNoReportChecks =
    fakeIsolationNoReport();

/// Dieselben drei Garantien mit einer roten in der Mitte.
const List<IsolationCheckResult> isolationOneRedCheck = <IsolationCheckResult>[
  IsolationCheckResult(
    check: IsolationCheck.noNetworkInterface,
    passed: true,
    evidence: 'no_interfaces ok: lo',
  ),
  isolationRedSocketCheck,
  IsolationCheckResult(
    check: IsolationCheck.seccompActive,
    passed: true,
    evidence: 'seccomp_applied ok: Seccomp:2;NoNewPrivs:1',
  ),
];

/// Ein laufender Client, der genau [checks] misst.
///
/// Nur `isolationChecks`: die Momentaufnahme traegt nie ein Ergebnis, weder im
/// Fake noch auf der Leitung. Sie kommen als eigene Ereignisse.
SandboxTestClient checkedClient(List<IsolationCheckResult> checks) =>
    runningClient()..isolationChecks = checks;

/// Der Befund, an dem ein Start scheitert.
const Diagnostic blockingFinding = Diagnostic(
  code: 'SANDBOX_001',
  severity: Severity.blocking,
  title: 'bwrap not found',
  why: 'bwrap is not on PATH, so no sandbox can be started.',
  fix: FixAction.copyCommand(command: 'sudo apt install bubblewrap'),
);

/// Baut den Bildschirm und pumpt, bis die erste Antwort steht.
Future<void> pumpSandbox(
  WidgetTester tester, {
  required SandboxTestClient client,
  String? chosenDirectory,
  HThemeMode mode = HThemeMode.dark,
  TextScaler textScaler = TextScaler.noScaling,
  Size size = const Size(1280, 800),
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    sandboxUnderTest(
      client: client,
      chosenDirectory: chosenDirectory,
      mode: mode,
      textScaler: textScaler,
    ),
  );
  // Ein Frame für `Sandbox(Status)`, einer für die Antwort, einer für den
  // Aufbau samt der Wartefrist von `HWait`.
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

/// Der Bildschirm in einem Baum ohne Shell.
Widget sandboxUnderTest({
  required SandboxTestClient client,
  String? chosenDirectory,
  HThemeMode mode = HThemeMode.dark,
  TextScaler textScaler = TextScaler.noScaling,
  List<Override> overrides = const <Override>[],
}) {
  final HTokens tokens = mode.resolve(Brightness.dark);
  return ProviderScope(
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      directoryChooserProvider.overrideWithValue(() async => chosenDirectory),
      ...overrides,
    ],
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
            color: tokens.colors.bg0,
            // Wie `app.dart`: kein Navigator, ein Overlay für alles, was
            // schwebt.
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(
                  builder: (BuildContext context) => const SandboxScreen(),
                ),
              ],
            ),
          ),
        ),
      ),
    ),
  );
}

/// Öffnet den Reiter mit diesem Schlüssel.
Future<void> openTab(WidgetTester tester, String key) async {
  await tester.tap(find.byKey(Key(key)));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 200));
}

/// Der Text des Sandbox-Providers, wie ihn der Bildschirm gerade sieht.
SandboxStatus statusOf(WidgetTester tester) {
  final ProviderContainer container = ProviderScope.containerOf(
    tester.element(find.byType(SandboxScreen)),
  );
  return container.read(sandboxStatusProvider).value ?? const SandboxStatus();
}
