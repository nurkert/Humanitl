// Das Backend der nächsten Sitzung im Reiter Isolation (HUM-440): Vorgabe ist
// `sandbox.backend`, eine abweichende Wahl geht als `cli_overrides` mit dem
// Start, ein Backend, das nicht starten kann oder unter der Untergrenze liegt,
// ist nicht wählbar und nennt den Grund des Daemons, und was der Daemon nicht
// zulässt (`SANDBOX_048`), steht gar nicht da. Läuft eine Sitzung, nennen
// Reiter und Kopf ihr Backend mit seiner Stufe.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';

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
}
