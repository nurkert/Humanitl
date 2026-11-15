// Das Backend der nächsten Sitzung im Reiter Isolation (HUM-440): Vorgabe ist
// `sandbox.backend`, eine abweichende Wahl geht als `cli_overrides` mit dem
// Start, ein Backend, das nicht starten kann oder unter der Untergrenze liegt,
// ist nicht wählbar und nennt den Grund des Daemons, und was der Daemon nicht
// zulässt (`SANDBOX_048`), steht gar nicht da. Das VM-Backend der
// Kernel-Stufe ist seit HUM-449 zugelassen und wählbar, wenn der Daemon es
// verfügbar nennt. Läuft eine Sitzung, nennen
// Reiter und Kopf ihr Backend mit seiner Stufe.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/core/domain/domain.dart';
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
}

/// Ein Fake, der die Liste für ein Backend in [held] erst nennt, wenn der
/// Test den Completer erfüllt; alle anderen Listen kommen sofort.
class _HeldBackendsClient extends SandboxTestClient {
  /// Die zurückgehaltenen Antworten je Backend der Sitzung.
  final Map<String, Completer<SandboxBackendList>> held =
      <String, Completer<SandboxBackendList>>{};

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
