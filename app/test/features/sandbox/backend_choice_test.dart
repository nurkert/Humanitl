// Das Backend der nächsten Sitzung im Reiter Isolation (HUM-440): Vorgabe ist
// `sandbox.backend`, eine abweichende Wahl geht als `cli_overrides` mit dem
// Start, ein Backend, das nicht starten kann oder unter der Untergrenze liegt,
// ist nicht wählbar und nennt den Grund des Daemons, und was der Daemon nicht
// zulässt (`SANDBOX_048`), steht gar nicht da. Das VM-Backend der
// Kernel-Stufe ist seit HUM-449 zugelassen und wählbar, wenn der Daemon es
// verfügbar nennt. Läuft eine Sitzung, nennen
// Reiter und Kopf ihr Backend mit seiner Stufe.

import 'dart:async';
import 'dart:ui' show Tristate;

import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/sandbox_backends_feed.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/providers/backend_choice.dart';

import 'harness.dart';

/// Container ist nicht installiert: `SANDBOX_034` mit dem Befehl zum Kopieren.
const Diagnostic containerMissing = Diagnostic(
  code: 'SANDBOX_034',
  severity: Severity.blocking,
  why: 'container was not found on PATH',
  fix: FixAction.copyCommand(command: 'sudo apt install container'),
);

/// Container liegt unter der Untergrenze: `SANDBOX_035`.
const Diagnostic belowMinimum = Diagnostic(
  code: 'SANDBOX_035',
  severity: Severity.blocking,
  why: 'container offers namespaces, below sandbox.min_isolation kernel',
);

/// Ein zweites Backend, das starten kann. Der Name ist erfunden: Die
/// Oberfläche urteilt über keinen Namen, alles kommt aus der Liste.
const SandboxBackendInfo availableContainer = SandboxBackendInfo(
  name: 'container',
  isolation: IsolationLevel.namespaces,
  available: true,
);

/// Die Liste des Fakes mit [container] an zweiter Stelle.
SandboxBackendList listWith(
  SandboxBackendInfo container, {
  IsolationLevel? minimum = IsolationLevel.policy,
}) => SandboxBackendList(
  minIsolation: minimum,
  autoBackend: DoctorCheckId.sandboxTool,
  backends: <SandboxBackendInfo>[
    FakeSandboxBackends.defaultSandboxBackends.backends.first,
    container,
  ],
);

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

Future<void> openIsolation(
  WidgetTester tester,
  SandboxTestClient client,
) async {
  await pumpSandbox(tester, client: client);
  await openTab(tester, 'sandbox-tab-isolation');
  await settle(tester);
}

Finder get choice => find.byKey(const Key('sandbox-backend-choice'));

/// Das Segment mit dem Wert [value] in der Wahl.
HSegment<String> segment(WidgetTester tester, String value) =>
    tester.widget<HSegment<String>>(
      find.descendant(
        of: choice,
        matching: find.byWidgetPredicate(
          (Widget w) => w is HSegment<String> && w.option.value == value,
        ),
      ),
    );

Future<void> tapChoice(WidgetTester tester, String label) async {
  final Finder target = find.descendant(of: choice, matching: find.text(label));
  await tester.ensureVisible(target);
  await tester.pump();
  await tester.tap(target, warnIfMissed: false);
  await settle(tester);
}

Future<void> startAndStop(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('sandbox-start')));
  await settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-stop')));
  await settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
  await settle(tester);
}

/// Linux und macOS: Der Abschnitt hängt an keiner Plattform, das Laden
/// gilt auf beiden (HUM-510).
final TargetPlatformVariant linuxAndMac = TargetPlatformVariant(
  <TargetPlatform>{TargetPlatform.linux, TargetPlatform.macOS},
);

/// Container in Prüfung (`DOCTOR_012`, HUM-502): Die Liste fragt nach
/// [sandboxBackendsCheckingRetry] wieder.
const SandboxBackendInfo stillChecking = SandboxBackendInfo(
  name: 'container',
  isolation: IsolationLevel.namespaces,
  diagnostics: <Diagnostic>[
    Diagnostic(
      code: DiagnosticCodes.doctorNotPerformed,
      severity: Severity.warning,
      why: 'still being checked',
    ),
  ],
);

/// Die Farbe des Texts unter [key].
Color? lineColor(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(Key(key))).style?.color;

/// Wählt ein anderes Projekt, wie der Ordnerwähler es tut: `SetConfig` auf
/// `sandbox.work_dir`, danach liest die App die Konfiguration neu.
Future<void> switchProject(WidgetTester tester, Finder section) async {
  unawaited(
    ProviderScope.containerOf(tester.element(section))
        .read(configProvider.notifier)
        .write(FormKeys.sandboxWorkDir, '/home/dev/other-project'),
  );
  await settle(tester);
}

/// Das Segment, auf dem der Fokus steht, oder null, wenn er nicht in der
/// Wahl liegt.
HSegment<String>? focusedSegment() {
  final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
  if (focused == null ||
      find
          .descendant(of: choice, matching: find.byWidget(focused.widget))
          .evaluate()
          .isEmpty) {
    return null;
  }
  return focused.findAncestorWidgetOfExactType<HSegment<String>>();
}

/// Das Bedienelement, auf das Tab (oder mit [backward] Umschalt-Tab) vom
/// Segment [from] aus führt, solange die Wahl bedienbar ist.
Future<FocusNode?> neighbourOf(
  WidgetTester tester,
  String from, {
  bool backward = false,
}) async {
  Focus.of(
    tester.element(
      find.descendant(
        of: find.byWidgetPredicate(
          (Widget w) => w is HSegment<String> && w.option.value == from,
        ),
        matching: find.byType(Text),
      ),
    ),
  ).requestFocus();
  await tester.pump();
  expect(focusedSegment()?.option.value, from);
  await _tab(tester, backward: backward);
  expect(focusedSegment(), isNull);
  return FocusManager.instance.primaryFocus;
}

/// Rollt [target] in die Mitte seiner Ansicht.
Future<void> _centre(WidgetTester tester, Finder target) async {
  unawaited(Scrollable.ensureVisible(tester.element(target), alignment: 0.5));
  await tester.pumpAndSettle();
}

/// Drückt Tab, mit [backward] Umschalt-Tab.
Future<void> _tab(WidgetTester tester, {bool backward = false}) async {
  if (backward) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyEvent(LogicalKeyboardKey.tab);
  if (backward) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.pump();
}

/// Wählt Container per Enter, mit zurückgehaltener Liste für Container,
/// und prüft, dass die Sperre den Fokus aus der Wahl nimmt (HUM-560).
/// [beforeChoice] läuft, wenn die Wahl sichtbar und noch bedienbar ist.
Future<Completer<SandboxBackendList>> _chooseContainerByKeyboard(
  WidgetTester tester,
  _HeldBackendsClient client, {
  Future<void> Function()? beforeChoice,
}) async {
  await openIsolation(tester, client);
  final Finder target = find.descendant(
    of: choice,
    matching: find.text('container'),
  );
  await tester.ensureVisible(target);
  await tester.pumpAndSettle();
  if (beforeChoice != null) {
    // Die Wahl in der Mitte der Ansicht, nicht halb unter den Reitern: Dort
    // fiele sie für die Leserichtung in die Zeile der Reiter.
    await _centre(tester, target);
    await beforeChoice();
    // Tab rollt zum Ziel; die Wahl steht danach wieder, wo sie stand.
    await _centre(tester, target);
  }
  Focus.of(tester.element(target)).requestFocus();
  await tester.pump();
  expect(focusedSegment()?.option.value, 'container');

  final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
  client.held['container'] = held;
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await settle(tester);
  expect(client.sandboxBackendsAsked, contains('container'));
  expect(held.isCompleted, isFalse);
  expect(tester.widget<HSegmented<String>>(choice).selected, 'container');
  expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);
  expect(focusedSegment(), isNull);
  return held;
}

void main() {
  testWidgets('the choice starts at the setting, and only a different '
      'backend goes along with the start', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);

    expect(tester.widget<HSegmented<String>>(choice).selected, 'auto');
    expect(
      find.byKey(const Key('sandbox-backend-level-container')),
      findsOneWidget,
    );
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('sandbox-backend-level-container')),
          )
          .data,
      'container: shared kernel, own namespaces',
    );

    // Ohne Wahl kein Eintrag in `cli_overrides`.
    await startAndStop(tester);
    expect(client.startBackends, <String?>[null]);

    await openTab(tester, 'sandbox-tab-isolation');
    await settle(tester);
    await tapChoice(tester, 'container');
    expect(tester.widget<HSegmented<String>>(choice).selected, 'container');
    await startAndStop(tester);
    expect(client.startBackends, <String?>[null, 'container']);
    // Die Einstellung bleibt, wie sie war.
    expect(
      client.configWrites.where(
        ((String, String) w) => w.$1 == FormKeys.sandboxBackend,
      ),
      isEmpty,
    );

    // Zurück auf die Vorgabe: wieder kein Eintrag.
    await openTab(tester, 'sandbox-tab-isolation');
    await settle(tester);
    await tapChoice(tester, 'Automatic');
    await startAndStop(tester);
    expect(client.startBackends, <String?>[null, 'container', null]);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend that cannot start is not selectable and names the '
      "daemon's reason with its fix", (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(
        const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.namespaces,
          diagnostics: <Diagnostic>[containerMissing],
        ),
      );
    await openIsolation(tester, client);

    expect(segment(tester, 'container').enabled, isFalse);
    expect(segment(tester, DoctorCheckId.sandboxTool).enabled, isTrue);
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('sandbox-backend-reason-container')),
          )
          .data,
      contains('container was not found on PATH'),
    );
    expect(
      find.byKey(const Key('sandbox-backend-fix-container')),
      findsOneWidget,
    );

    await tapChoice(tester, 'container');
    expect(tester.widget<HSegmented<String>>(choice).selected, 'auto');
    await startAndStop(tester);
    expect(client.startBackends, <String?>[null]);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend below the lower bound is not selectable either, and '
      'the lower bound is shown', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(
        const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.namespaces,
          diagnostics: <Diagnostic>[belowMinimum],
        ),
        minimum: IsolationLevel.kernel,
      );
    await openIsolation(tester, client);

    expect(segment(tester, 'container').enabled, isFalse);
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('sandbox-backend-reason-container')),
          )
          .data,
      contains('below sandbox.min_isolation'),
    );
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-minimum')))
          .data,
      'Minimum isolation: own kernel in a micro VM',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-447: Seit HUM-449 ist die Kernel-Stufe zugelassen. Ihr VM-Backend
  // steht mit seiner Stufe da und ist wählbar, wenn der Daemon es verfügbar
  // nennt; fehlt eine Voraussetzung, nennt die Zeile sie mit dem Befehl des
  // Daemons. Der Name kommt aus dem Fake, damit er in der App an einer
  // Stelle steht (HUM-145).
  const String kernel = FakeSandboxBackends.kernelBackendName;

  testWidgets('the VM backend stands with the level kernel and is selectable '
      'when the daemon names it available', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient();
    expect(
      client.sandboxBackendList,
      same(FakeSandboxBackends.defaultSandboxBackends),
    );
    await openIsolation(tester, client);

    expect(segment(tester, kernel).enabled, isTrue);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-level-$kernel')))
          .data,
      '$kernel: own kernel in a micro VM',
    );
    expect(
      find.byKey(const Key('sandbox-backend-reason-$kernel')),
      findsNothing,
    );

    await tapChoice(tester, kernel);
    expect(tester.widget<HSegmented<String>>(choice).selected, kernel);
    await startAndStop(tester);
    expect(client.startBackends, <String?>[kernel]);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the VM backend without a prerequisite is not selectable and '
      'names what is missing with the fix', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(
        FakeSandboxBackends.kernelBackendUnavailable,
      );
    await openIsolation(tester, client);

    expect(segment(tester, kernel).enabled, isFalse);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-level-$kernel')))
          .data,
      '$kernel: own kernel in a micro VM',
    );
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-reason-$kernel')))
          .data,
      contains('/dev/kvm readable and writable'),
    );
    expect(
      find.byKey(const Key('sandbox-backend-fix-$kernel')),
      findsOneWidget,
    );

    await tapChoice(tester, kernel);
    expect(tester.widget<HSegmented<String>>(choice).selected, 'auto');
    await startAndStop(tester);
    expect(client.startBackends, <String?>[null]);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend the daemon does not admit is no choice at all', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(
        const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.kernel,
          diagnostics: <Diagnostic>[FakeSandboxBackends.notAdmitted],
        ),
      );
    await openIsolation(tester, client);

    // Mit nur einem zugelassenen Backend gibt es nichts zu wählen.
    expect(choice, findsNothing);
    expect(find.text('container'), findsNothing);
    expect(
      find.byKey(const Key('sandbox-backend-level-container')),
      findsNothing,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('on macOS the daemon lists Seatbelt alone: nothing to choose, '
      'but its line and the lower bound stand', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = const SandboxBackendList(
        minIsolation: IsolationLevel.policy,
        autoBackend: 'seatbelt',
        backends: <SandboxBackendInfo>[
          SandboxBackendInfo(
            name: 'seatbelt',
            isolation: IsolationLevel.policy,
            available: true,
            selected: true,
          ),
        ],
      );
    await openIsolation(tester, client);
    expect(choice, findsNothing);
    expect(find.byKey(const Key('sandbox-backend')), findsOneWidget);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-level-seatbelt')))
          .data,
      'seatbelt: kernel policy over the real paths',
    );
    expect(find.byKey(const Key('sandbox-backend-minimum')), findsOneWidget);
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('choosing a backend asks for the snapshot of a start on it, and '
      'the start button follows that answer', (WidgetTester tester) async {
    // Der Daemon sagt zu einem Start mit Container, was ihm fehlt; die Liste
    // selbst nennt Container verfügbar. Der Knopf richtet sich nach dem
    // Schnappschuss, nicht nach der Liste.
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(availableContainer)
      ..planBackendFindings['container'] = const <Diagnostic>[containerMissing];
    await openIsolation(tester, client);
    HButton startButton() =>
        tester.widget<HButton>(find.byKey(const Key('sandbox-start')));
    expect(startButton().onPressed, isNotNull);

    await tapChoice(tester, 'container');
    expect(client.planBackends, contains('container'));
    expect(client.sandboxBackendsAsked, contains('container'));
    expect(startButton().onPressed, isNull);

    // Zurück auf die Vorgabe: der Schnappschuss ohne `Plan.backend`, und
    // der Knopf geht wieder.
    await tapChoice(tester, 'Automatic');
    expect(startButton().onPressed, isNotNull);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('auto cannot be chosen when the backend it means cannot start', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.policy,
        autoBackend: DoctorCheckId.sandboxTool,
        backends: <SandboxBackendInfo>[
          const SandboxBackendInfo(
            name: DoctorCheckId.sandboxTool,
            isolation: IsolationLevel.namespaces,
            selected: true,
            diagnostics: <Diagnostic>[belowMinimum],
          ),
          availableContainer,
        ],
      );
    await openIsolation(tester, client);
    expect(segment(tester, 'auto').enabled, isFalse);
    expect(segment(tester, 'container').enabled, isTrue);
    expect(
      find.byKey(Key('sandbox-backend-reason-${DoctorCheckId.sandboxTool}')),
      findsOneWidget,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('an unavailable backend without a reason says that it is not '
      'available', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(
        const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.namespaces,
        ),
      );
    await openIsolation(tester, client);
    expect(
      tester
          .widget<Text>(
            find.byKey(const Key('sandbox-backend-reason-container')),
          )
          .data,
      'Not available on this system.',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a running session names its backend and level in the tab and '
      'in the header', (WidgetTester tester) async {
    final SandboxTestClient client = runningClient();
    client.sandbox = client.sandbox.copyWith(
      backend: 'container',
      isolationLevel: IsolationLevel.namespaces,
    );
    await openIsolation(tester, client);

    expect(choice, findsNothing);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('sandbox-backend-current')))
          .data,
      'container: shared kernel, own namespaces',
    );
    expect(
      tester
          .widget<HBadge>(find.byKey(const Key('sandbox-backend-badge')))
          .text,
      'container · namespaces',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-502: Solange der Daemon ein Backend noch prüft (`DOCTOR_012`), ist
  // nichts über es bekannt, und eine Wahl bleibt stehen, statt auf die
  // Einstellung und wieder zurück zu springen. Ein Backend, dem etwas fehlt,
  // fällt dagegen auf die Einstellung zurück. Mutationsbeweis: ohne
  // `checking` in der Prüfung der Wahl zeigt die erste Erwartung `auto`.
  test('a choice stands while the daemon is still checking its backend', () {
    const ConfigValues config = ConfigValues(values: <String, Object?>{});
    const SandboxBackendInfo checking = SandboxBackendInfo(
      name: 'container',
      isolation: IsolationLevel.namespaces,
      diagnostics: <Diagnostic>[
        Diagnostic(
          code: DiagnosticCodes.doctorNotPerformed,
          severity: Severity.warning,
          why: 'still being checked',
        ),
      ],
    );
    expect(checking.checking, isTrue);
    expect(listWith(checking).checking, isTrue);
    expect(
      shownSandboxBackend('container', config, listWith(checking)),
      'container',
    );
    expect(
      shownSandboxBackend(
        'container',
        config,
        listWith(
          const SandboxBackendInfo(
            name: 'container',
            diagnostics: <Diagnostic>[containerMissing],
          ),
        ),
      ),
      FormKeys.sandboxBackendAuto,
    );
    expect(listWith(availableContainer).checking, isFalse);
    // Steht vor der Prüfung ein Grund, der einen Start ausschließt, gilt er:
    // Die Liste fragt weiter, aber die Wahl fällt auf die Einstellung.
    const SandboxBackendInfo belowAndChecking = SandboxBackendInfo(
      name: 'container',
      isolation: IsolationLevel.namespaces,
      diagnostics: <Diagnostic>[
        belowMinimum,
        Diagnostic(
          code: DiagnosticCodes.doctorNotPerformed,
          severity: Severity.warning,
          why: 'still being checked',
        ),
      ],
    );
    expect(belowAndChecking.checking, isTrue);
    expect(belowAndChecking.undecided, isFalse);
    expect(
      shownSandboxBackend('container', config, listWith(belowAndChecking)),
      FormKeys.sandboxBackendAuto,
    );
  });

  testWidgets('the section stays while the list for a new choice loads, and '
      'shows that list once it arrives', (WidgetTester tester) async {
    // HUM-504: Die Liste für einen Start auf Container ist ein neues
    // Mitglied der Familie ohne Wert. Bis der Daemon antwortet, bleibt die
    // zuletzt gezeigte Liste stehen, mit Container schon gewählt, ohne dass
    // der Abschnitt zusammenfällt oder seine Höhe ändert. Die Lage misst der
    // Test nicht: `tapChoice` rollt die Wahl erst ins Bild.
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    final Size before = tester.getSize(section);

    final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
    client.held['container'] = held;
    await tapChoice(tester, 'container');
    expect(client.sandboxBackendsAsked, contains('container'));
    expect(held.isCompleted, isFalse);

    expect(section, findsOneWidget);
    expect(tester.getSize(section), before);
    expect(tester.widget<HSegmented<String>>(choice).selected, 'container');
    expect(
      find.byKey(const Key('sandbox-backend-level-container')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('sandbox-backend-level-extra')), findsNothing);
    // Die stehende Liste gehört zum vorigen Plan: gedämpft und nicht wählbar.
    final HTokens tokens = HTheme.of(tester.element(section));
    Color? lineColor(String key) =>
        tester.widget<Text>(find.byKey(Key(key))).style?.color;
    expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);
    expect(lineColor('sandbox-backend-level-container'), tokens.colors.fg2);
    expect(lineColor('sandbox-backend-minimum'), tokens.colors.fg2);

    // Die Antwort kommt: Der Abschnitt zeigt jetzt die neue Liste.
    held.complete(
      SandboxBackendList(
        minIsolation: IsolationLevel.policy,
        autoBackend: DoctorCheckId.sandboxTool,
        backends: <SandboxBackendInfo>[
          ...listWith(availableContainer).backends,
          const SandboxBackendInfo(
            name: 'extra',
            isolation: IsolationLevel.namespaces,
            available: true,
          ),
        ],
      ),
    );
    await settle(tester);
    expect(section, findsOneWidget);
    expect(
      find.byKey(const Key('sandbox-backend-level-extra')),
      findsOneWidget,
    );
    expect(tester.widget<HSegmented<String>>(choice).selected, 'container');
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(lineColor('sandbox-backend-level-container'), tokens.colors.fg1);
    expect(lineColor('sandbox-backend-minimum'), tokens.colors.fg1);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a failed refresh hides the section, even though the provider '
      'still holds the previous list', (WidgetTester tester) async {
    // HUM-504: Riverpod behält nach einer gescheiterten Auffrischung den
    // alten Wert neben dem Fehler. Der Fehler gewinnt, wie bei einem Daemon
    // ohne den Aufruf.
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    expect(section, findsOneWidget);

    client.sandboxBackendsFailure = containerMissing;
    ProviderScope.containerOf(tester.element(section))
        .invalidate(sandboxBackendsProvider(null));
    await settle(tester);
    expect(section, findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-510: Das erneute Fragen nach einem Backend in Prüfung (HUM-502)
  // frischt dasselbe Mitglied mit demselben Plan auf. Die stehende Liste ist
  // die aktuelle: nicht gedämpft, die Wahl bleibt bedienbar, nichts flackert
  // alle 2 s. Mutationsbeweis: `sandboxBackendsOutdated` als `isLoading`
  // macht diesen Test rot.
  testWidgets('asking again for a backend still being checked neither dims '
      'the list nor locks the choice', (WidgetTester tester) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(stillChecking);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    final HTokens tokens = HTheme.of(tester.element(section));
    expect(
      find.byKey(const Key('sandbox-backend-checking-container')),
      findsOneWidget,
    );

    final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
    client.held[null] = held;
    final int asked = client.sandboxBackendsAsked.length;
    await tester.pump(sandboxBackendsCheckingRetry);
    await settle(tester);
    expect(client.sandboxBackendsAsked.length, greaterThan(asked));
    expect(held.isCompleted, isFalse);

    // Die Frage ist unterwegs, die Liste gilt weiter.
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(
      lineColor(tester, 'sandbox-backend-level-container'),
      tokens.colors.fg1,
    );
    expect(lineColor(tester, 'sandbox-backend-minimum'), tokens.colors.fg1);
    expect(
      find.byKey(const Key('sandbox-backend-checking-container')),
      findsOneWidget,
    );

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(
      find.byKey(const Key('sandbox-backend-checking-container')),
      findsNothing,
    );
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(segment(tester, 'container').option.enabled, isTrue);
  }, variant: linuxAndMac);

  // HUM-510: Ein anderes Projekt ist eine Änderung der Konfiguration;
  // Riverpod lädt dasselbe Mitglied neu und behält die alte Liste. Sie
  // gehört zum vorigen Plan: gedämpft und ohne Wahl wie in HUM-504, bis die
  // Antwort für das neue Projekt da ist. Mutationsbeweis:
  // `sandboxBackendsOutdated` als `false` macht diesen Test rot.
  testWidgets('a project switch shows the previous list muted and locks the '
      'choice until the answer for the new project is there', (
    WidgetTester tester,
  ) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    final HTokens tokens = HTheme.of(tester.element(section));
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    final Size before = tester.getSize(section);

    final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
    client.held[null] = held;
    await switchProject(tester, section);
    expect(held.isCompleted, isFalse);

    expect(section, findsOneWidget);
    expect(tester.getSize(section), before);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);
    expect(
      lineColor(tester, 'sandbox-backend-level-container'),
      tokens.colors.fg2,
    );
    expect(lineColor(tester, 'sandbox-backend-minimum'), tokens.colors.fg2);

    // Das neue Projekt hebt die Untergrenze: Container liegt darunter.
    held.complete(
      listWith(
        const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.namespaces,
          diagnostics: <Diagnostic>[belowMinimum],
        ),
        minimum: IsolationLevel.kernel,
      ),
    );
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(segment(tester, 'container').option.enabled, isFalse);
    expect(
      lineColor(tester, 'sandbox-backend-level-container'),
      tokens.colors.fg1,
    );
    expect(lineColor(tester, 'sandbox-backend-minimum'), tokens.colors.fg1);
  }, variant: linuxAndMac);

  // HUM-510, Spezifikation 2: Die Gründe, warum ein Backend nicht starten
  // kann, stehen während des Ladens gedämpft da, nicht als aktuelle Warnung,
  // und danach wieder in der Farbe der Warnung; ebenso der Satz eines
  // Backends ohne Grund. Mutationsbeweis: `warningColor` ohne `pending`
  // macht diesen Test rot.
  testWidgets('the reasons stand muted while the list loads and in the '
      'warning colour once it is there', (WidgetTester tester) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.policy,
        autoBackend: DoctorCheckId.sandboxTool,
        backends: <SandboxBackendInfo>[
          ...listWith(
            const SandboxBackendInfo(
              name: 'container',
              isolation: IsolationLevel.namespaces,
              diagnostics: <Diagnostic>[containerMissing],
            ),
          ).backends,
          const SandboxBackendInfo(
            name: 'other',
            isolation: IsolationLevel.namespaces,
          ),
        ],
      );
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    final HTokens tokens = HTheme.of(tester.element(section));
    const String reason = 'sandbox-backend-reason-container';
    const String noReason = 'sandbox-backend-reason-other';
    expect(lineColor(tester, reason), tokens.colors.warningText);
    expect(lineColor(tester, noReason), tokens.colors.warningText);

    final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
    client.held[null] = held;
    await switchProject(tester, section);
    expect(lineColor(tester, reason), tokens.colors.fg2);
    expect(lineColor(tester, noReason), tokens.colors.fg2);

    held.complete(client.sandboxBackendList);
    await settle(tester);
    expect(lineColor(tester, reason), tokens.colors.warningText);
    expect(lineColor(tester, noReason), tokens.colors.warningText);
  }, variant: linuxAndMac);

  // HUM-510, Review: Scheitert das Neuladen nach einem Wechsel des Projekts,
  // hält Riverpod die alte Liste neben dem Fehler; der Abschnitt zeigt sie
  // nicht als aktuell, sondern verschwindet wie bei jedem Fehler (HUM-504).
  // Mutationsbeweis: die Liste ohne Prüfung von `hasError` macht diesen Test
  // rot.
  testWidgets('a failed reload after a project switch hides the previous '
      'list', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    expect(section, findsOneWidget);

    client.sandboxBackendsFailure = containerMissing;
    await switchProject(tester, section);
    expect(section, findsNothing);
  }, variant: linuxAndMac);

  // HUM-560: Eine Wahl per Tastatur fragt nach der Liste für das neue
  // Backend; bis sie da ist, sind die Segmente gesperrt, und ein gesperrtes
  // Segment verliert den Fokus. Danach steht er wieder auf dem gewählten
  // Segment. Mutationsbeweis: ohne die Rückgabe in `SandboxBackendSegments`
  // ist dieser Test rot.
  testWidgets('choosing a backend by keyboard keeps the focus in the choice '
      'across the reload', (WidgetTester tester) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    final Completer<SandboxBackendList> held = await _chooseContainerByKeyboard(
      tester,
      client,
    );

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(focusedSegment()?.option.value, 'container');
  }, variant: linuxAndMac);

  // HUM-560, Review: Wer während der Sperre ein anderes Bedienelement
  // anklickt oder fokussiert, hat den Fokus bewusst bewegt; die Rückgabe holt ihn nicht in die Wahl zurück.
  // Mutationsbeweis: eine Rückgabe ohne den Vergleich mit dem Knoten, auf
  // den die Sperre den Fokus schob, macht diesen Test rot.
  testWidgets('focus moved away while the choice is locked stays where it '
      'went', (WidgetTester tester) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    final Completer<SandboxBackendList> held = await _chooseContainerByKeyboard(
      tester,
      client,
    );
    final FocusNode? fallback = FocusManager.instance.primaryFocus;
    // Ein anderes Bedienelement nimmt den Fokus, wie nach einem Klick.
    final Finder start = find.byKey(const Key('sandbox-start'));
    Focus.of(
      tester.element(
        find.descendant(of: start, matching: find.byType(Text)).first,
      ),
    ).requestFocus();
    await tester.pump();
    final FocusNode? moved = FocusManager.instance.primaryFocus;
    expect(moved, isNotNull);
    expect(moved, isNot(same(fallback)));
    expect(focusedSegment(), isNull);

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(FocusManager.instance.primaryFocus, same(moved));
    expect(focusedSegment(), isNull);
  }, variant: linuxAndMac);

  // HUM-560, Review: Wer den Fokus während der Sperre fortbewegt und er
  // danach dorthin zurückfällt, wohin die Sperre ihn schob, hat ihn doch
  // bewegt; die Rückgabe bleibt aufgehoben. Mutationsbeweis: nur beim
  // Entsperren zu vergleichen macht diesen Test rot.
  testWidgets('focus moved away and back while the choice is locked is not '
      'pulled in', (WidgetTester tester) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    final Completer<SandboxBackendList> held = await _chooseContainerByKeyboard(
      tester,
      client,
    );
    final FocusNode? fallback = FocusManager.instance.primaryFocus;
    final Finder start = find.byKey(const Key('sandbox-start'));
    final FocusNode other = Focus.of(
      tester.element(
        find.descendant(of: start, matching: find.byType(Text)).first,
      ),
    )..requestFocus();
    await tester.pump();
    expect(FocusManager.instance.primaryFocus, same(other));
    // Zurück auf den Knoten, auf den die Sperre den Fokus schob.
    expect(fallback, isNotNull);
    fallback!.requestFocus();
    await tester.pump();
    expect(FocusManager.instance.primaryFocus, same(fallback));

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(FocusManager.instance.primaryFocus, same(fallback));
    expect(focusedSegment(), isNull);
  }, variant: linuxAndMac);

  // HUM-560, Review: Verschwindet die Wahl während der Sperre, etwa weil
  // eine Sitzung startet, wartet nichts mehr auf die Rückgabe: keine
  // Ausnahme, und die Antwort danach bewegt den Fokus nicht.
  testWidgets('a choice removed while locked restores nothing', (
    WidgetTester tester,
  ) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    final Completer<SandboxBackendList> held = await _chooseContainerByKeyboard(
      tester,
      client,
    );
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(find.byKey(const Key('sandbox-backend-current')), findsOneWidget);
    expect(choice, findsNothing);
    final FocusNode? before = FocusManager.instance.primaryFocus;

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.takeException(), isNull);
    expect(FocusManager.instance.primaryFocus, same(before));
  }, variant: linuxAndMac);

  // HUM-570: Während der Sperre steht der Fokus unter den `Shortcuts` der
  // App, und Tab führt zum nächsten Bedienelement nach der Wahl, Umschalt-Tab
  // zum vorigen, wie von einem bedienbaren Segment aus. Das Entsperren holt
  // ihn danach nicht zurück. Mutationsbeweis: ohne das Auffangen in
  // `SandboxBackendSegments` liegt der Fokus auf dem Wurzelbereich der
  // Ansicht, Tab bewegt ihn nicht, und dieser Test ist rot.
  for (final bool backward in <bool>[false, true]) {
    testWidgets('${backward ? 'Shift+Tab' : 'Tab'} moves on while the choice '
        'is locked, and the reload leaves the focus there', (
      WidgetTester tester,
    ) async {
      final _HeldBackendsClient client = _HeldBackendsClient()
        ..sandboxBackendList = listWith(availableContainer);
      FocusNode? expected;
      final Completer<SandboxBackendList> held =
          await _chooseContainerByKeyboard(
            tester,
            client,
            // Wohin Tab von der bedienbaren Wahl aus führt: vom letzten
            // Segment vorwärts, vom ersten rückwärts.
            beforeChoice: () async {
              expected = await neighbourOf(
                tester,
                backward ? FormKeys.sandboxBackendAuto : 'container',
                backward: backward,
              );
            },
          );
      expect(expected, isNotNull);
      final FocusNode? locked = FocusManager.instance.primaryFocus;
      await _tab(tester, backward: backward);
      expect(FocusManager.instance.primaryFocus, isNot(same(locked)));
      expect(FocusManager.instance.primaryFocus, same(expected));
      expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);

      held.complete(listWith(availableContainer));
      await settle(tester);
      expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
      expect(FocusManager.instance.primaryFocus, same(expected));
      expect(focusedSegment(), isNull);
    }, variant: linuxAndMac);
  }

  // HUM-570: Auch die zweite Wahl per Tastatur fängt den Fokus auf. Diesmal
  // fiele er über den Verlauf des Bereichs auf das zuvor gewählte, ebenfalls
  // gesperrte Segment, also in der Wahl, und kein Wechsel des Fokus meldete
  // die Sperre. Tab geht weiter, und das Entsperren holt ihn nicht zurück.
  // Mutationsbeweis: ohne das Anfordern von `_choice` beim Sperren ist
  // dieser Test rot.
  testWidgets('a second keyboard choice is caught again, and Tab moves on', (
    WidgetTester tester,
  ) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    final Completer<SandboxBackendList> first =
        await _chooseContainerByKeyboard(tester, client);
    first.complete(listWith(availableContainer));
    await settle(tester);
    expect(focusedSegment()?.option.value, 'container');

    final String other = DoctorCheckId.sandboxTool;
    Focus.of(
      tester.element(
        find.descendant(
          of: find.byWidgetPredicate(
            (Widget w) => w is HSegment<String> && w.option.value == other,
          ),
          matching: find.byType(Text),
        ),
      ),
    ).requestFocus();
    await tester.pump();
    expect(focusedSegment()?.option.value, other);
    final Completer<SandboxBackendList> second =
        Completer<SandboxBackendList>();
    client.held[other] = second;
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);
    final FocusNode? locked = FocusManager.instance.primaryFocus;
    expect(locked?.debugLabel, 'sandbox-backend-choice');

    await _tab(tester);
    final FocusNode? next = FocusManager.instance.primaryFocus;
    expect(next, isNot(same(locked)));
    expect(focusedSegment(), isNull);

    second.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(FocusManager.instance.primaryFocus, same(next));
  }, variant: linuxAndMac);

  // HUM-570, Review: Nimmt im selben Bild wie die Sperre ein anderes
  // Bedienelement den Fokus (Klick auf ein Feld) oder ein Dialog, bleibt er
  // dort: Die Anforderung der Sperre unterliegt der späteren, und das
  // Entsperren holt ihn nicht in die Wahl. Mutationsbeweis: ein erneutes
  // Anfordern, wenn der Fokus `_choice` nicht erreicht, macht diese Tests
  // rot.
  for (final bool dialog in <bool>[false, true]) {
    testWidgets('${dialog ? 'a dialog' : 'another control'} taking the focus '
        'in the locking frame keeps it', (WidgetTester tester) async {
      final _HeldBackendsClient client = _HeldBackendsClient()
        ..sandboxBackendList = listWith(availableContainer);
      await openIsolation(tester, client);
      final Finder target = find.descendant(
        of: choice,
        matching: find.text('container'),
      );
      await tester.ensureVisible(target);
      await tester.pumpAndSettle();
      Focus.of(tester.element(target)).requestFocus();
      await tester.pump();
      expect(focusedSegment()?.option.value, 'container');

      final Completer<SandboxBackendList> held =
          Completer<SandboxBackendList>();
      client.held['container'] = held;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
      final FocusNode other = FocusNode(debugLabel: 'hum-570-other');
      addTearDown(other.dispose);
      FocusNode wanted = other;
      if (dialog) {
        // Wie ein Dialog: ein eigener Fokusbereich über der Ansicht, der
        // sich den Fokus nimmt.
        final OverlayEntry entry = OverlayEntry(
          builder: (_) => FocusScope(
            autofocus: true,
            child: Focus(
              focusNode: other,
              autofocus: true,
              child: const SizedBox.shrink(),
            ),
          ),
        );
        addTearDown(
          () => entry
            ..remove()
            ..dispose(),
        );
        Overlay.of(tester.element(choice)).insert(entry);
      } else {
        // Etwas fordert im Bild der Sperre den Fokus für den Startknopf an.
        final FocusNode start = Focus.of(
          tester.element(
            find
                .descendant(
                  of: find.byKey(const Key('sandbox-start')),
                  matching: find.byType(Text),
                )
                .first,
          ),
        );
        wanted = start;
        WidgetsBinding.instance.addPostFrameCallback(
          (_) => start.requestFocus(),
        );
      }
      await tester.pump();
      // Die Sperre baut im selben Bild.
      expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);
      await settle(tester);
      expect(FocusManager.instance.primaryFocus, same(wanted));
      expect(focusedSegment(), isNull);

      held.complete(listWith(availableContainer));
      await settle(tester);
      expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
      expect(FocusManager.instance.primaryFocus, same(wanted));
      expect(focusedSegment(), isNull);
    }, variant: linuxAndMac);
  }

  // HUM-570, Review: Der Knoten um die Segmente hält nur den Fokus während
  // der Sperre; Bildschirmleser sehen ihn nicht. Mit Semantik ginge sein
  // Merkmal „fokussierbar“ in den nächsten Semantikknoten über der Wahl auf,
  // hier die rollbare Ansicht. Mutationsbeweis: `includeSemantics` auf true
  // macht diesen Test rot.
  testWidgets('the node around the segments adds no focusable semantics', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle semantics = tester.ensureSemantics();
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final SemanticsNode node = tester.getSemantics(choice);
    expect(node.getSemanticsData().flagsCollection.isFocused, Tristate.none);
    semantics.dispose();
  }, variant: linuxAndMac);

  // HUM-560: Wer nicht in der Wahl war, als sie gesperrt wurde, bekommt
  // dort nach dem Neuladen keinen Fokus. Mutationsbeweis: eine Rückgabe ohne
  // die Bedingung macht diesen Test rot.
  testWidgets('a reload does not pull focus into the choice from elsewhere', (
    WidgetTester tester,
  ) async {
    final _HeldBackendsClient client = _HeldBackendsClient()
      ..sandboxBackendList = listWith(availableContainer);
    await openIsolation(tester, client);
    final Finder section = find.byKey(const Key('sandbox-backend'));
    expect(focusedSegment(), isNull);

    final Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
    client.held[null] = held;
    await switchProject(tester, section);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isFalse);

    held.complete(listWith(availableContainer));
    await settle(tester);
    expect(tester.widget<HSegmented<String>>(choice).enabled, isTrue);
    expect(focusedSegment(), isNull);
  }, variant: linuxAndMac);
}

/// Ein Fake, der die Liste für ein Backend in [held] erst nennt, wenn der
/// Test den Completer erfüllt; alle anderen Listen kommen sofort.
class _HeldBackendsClient extends SandboxTestClient {
  /// Die zurückgehaltenen Antworten je Backend der Sitzung; `null` für die
  /// Liste ohne gewähltes Backend.
  final Map<String?, Completer<SandboxBackendList>> held =
      <String?, Completer<SandboxBackendList>>{};

  @override
  Future<SandboxBackendList> sandboxBackends({String? backend}) {
    final Completer<SandboxBackendList>? hold = held[backend];
    if (hold == null) {
      return super.sandboxBackends(backend: backend);
    }
    sandboxBackendsAsked.add(backend);
    return hold.future;
  }
}
