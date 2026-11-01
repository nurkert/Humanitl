// Ein Befund des Plans steht dort, worüber er etwas sagt (HUM-384).
//
// Der Fall vom Bildschirm: Ein alter Daemon liest ein neueres gebündeltes
// Profil und lehnt dessen `sandbox.backend` mit `CONFIG_003` ab. Das Setup
// hängte den Befund an die Zeile „Project folder" und schrieb „This folder
// cannot be used" darüber, obwohl der Ordner in Ordnung war; die einzige
// angebotene Abhilfe war die Ordnerauswahl. Jetzt bleibt der Ordner grün, und
// der Befund steht mit Code und Grund im Schritt der Sandbox, unter einer
// Überschrift, die nichts über den Ordner behauptet. Ein Befund über den
// Ordner (`SANDBOX_006` mit der Abhilfe „anderen Ordner wählen") bleibt, wo
// er war.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_screen.dart';

import '../../harness/app_harness.dart';

/// Der Projektordner des gespielten Rechners.
const String project = '/home/u/projects/app';

/// Der Befund des alten Daemons über das neuere Profil.
const Diagnostic backendRefused = Diagnostic(
  code: 'CONFIG_003',
  severity: Severity.blocking,
  why: 'sandbox.backend = auto, the only backend of this build is another one',
);

/// Der Befund über einen Ordner, den der Daemon nicht einhängt.
const Diagnostic folderRefused = Diagnostic(
  code: DiagnosticCodes.workDirRefused,
  severity: Severity.blocking,
  why: 'the project directory /home/u/projects/app is not writable',
  fix: FixAction.changeSetting(key: 'sandbox.work_dir', value: ''),
);

/// Ein Daemon, dessen Status und Plan [finding] melden, solange [refusing]
/// gilt; danach antwortet er wie der Fake.
class PlanRefusingClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit gesunder Maschine und [project] als Ordner.
  PlanRefusingClient(this.finding) {
    doctorReport = fakeDoctorOk();
    sandbox = sandbox.copyWith(workDirHost: project);
  }

  /// Was Status und Plan melden.
  final Diagnostic finding;

  /// Ob Status und Plan [finding] noch melden.
  bool refusing = true;

  /// Wie oft die Oberfläche einen Plan verlangt hat.
  int plans = 0;

  Stream<SandboxUpdate> _refused() async* {
    yield SandboxUpdate.diagnostic(finding);
    yield SandboxUpdate.status(sandbox.copyWith(state: SandboxState.failed));
  }

  @override
  Stream<SandboxUpdate> sandboxStatus() =>
      refusing ? _refused() : super.sandboxStatus();

  @override
  Stream<SandboxUpdate> planSandbox({String? profile, WorkMode? workMode}) {
    plans += 1;
    return refusing
        ? _refused()
        : super.planSandbox(profile: profile, workMode: workMode);
  }
}

/// Der Text der Zustandszeile einer Zeile.
String stateOf(WidgetTester tester, SetupCheckKind kind) =>
    tester.widget<Text>(find.byKey(Key('setup-state-${kind.name}'))).data!;

/// Die Überschrift eines offenen Schritts, oder null.
String? headlineOf(WidgetTester tester, SetupCheckKind kind) {
  final Finder headline = find.byKey(Key('setup-headline-${kind.name}'));
  return headline.evaluate().isEmpty
      ? null
      : tester.widget<Text>(headline).data;
}

/// Der Text [text] im offenen Schritt [kind].
Finder inStep(SetupCheckKind kind, String text) => find.descendant(
  of: find.byKey(Key('setup-open-${kind.name}')),
  matching: find.text(text),
);

/// Baut die App über [client], öffnet das Setup und lässt die Antworten
/// durchlaufen.
Future<void> openSetup(WidgetTester tester, PlanRefusingClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
  await openSetupStep(tester, SetupCheckKind.project.name);
  await openSetupStep(tester, SetupCheckKind.sandbox.name);
}

void main() {
  testWidgets('a plan finding that is not about the folder stands under the '
      'sandbox step, not the folder', (WidgetTester tester) async {
    final PlanRefusingClient client = PlanRefusingClient(backendRefused);
    await openSetup(tester, client);

    // Der Ordner war in Ordnung und bleibt es.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(find.text('This folder cannot be used'), findsNothing);
    expect(inStep(SetupCheckKind.project, 'CONFIG_003'), findsNothing);
    expect(find.byKey(const Key('fix-choose-folder')), findsNothing);

    // Der Befund steht wahr im Schritt der Sandbox, mit Code und Grund.
    expect(stateOf(tester, SetupCheckKind.sandbox), 'blocks the start');
    expect(
      headlineOf(tester, SetupCheckKind.sandbox),
      'The sandbox cannot start with this setup',
    );
    expect(inStep(SetupCheckKind.sandbox, 'CONFIG_003'), findsOneWidget);
    expect(
      find.descendant(
        of: find.byKey(const Key('setup-open-sandbox')),
        matching: find.textContaining('the only backend of this build'),
      ),
      findsOneWidget,
    );
    expect(
      find.descendant(
        of: find.byType(SetupScreen),
        matching: find.text(
          'Sandbox check blocks the start. Fix it above, then this button '
          'turns on.',
        ),
      ),
      findsOneWidget,
    );
  });

  testWidgets('check again asks for a new plan, and a cured plan clears the '
      'step', (WidgetTester tester) async {
    final PlanRefusingClient client = PlanRefusingClient(backendRefused);
    await openSetup(tester, client);
    final int before = client.plans;

    client.refusing = false;
    final Finder recheck = find.byKey(const Key('setup-sandbox-recheck'));
    await tester.ensureVisible(recheck);
    await tester.pump();
    await tester.tap(recheck);
    for (int i = 0; i < 4; i++) {
      await tester.pump();
    }

    expect(client.plans, before + 1);
    expect(stateOf(tester, SetupCheckKind.sandbox), 'ok');
    expect(find.text('CONFIG_003'), findsNothing);
  });

  testWidgets('a finding about the folder still names the folder', (
    WidgetTester tester,
  ) async {
    final PlanRefusingClient client = PlanRefusingClient(folderRefused);
    await openSetup(tester, client);

    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(
      headlineOf(tester, SetupCheckKind.project),
      'This folder cannot be used',
    );
    expect(
      inStep(SetupCheckKind.project, DiagnosticCodes.workDirRefused),
      findsOneWidget,
    );
    expect(find.byKey(const Key('fix-choose-folder')), findsOneWidget);
    // Der Schritt der Sandbox sagt nichts über den Ordner.
    expect(stateOf(tester, SetupCheckKind.sandbox), 'ok');
    expect(
      inStep(SetupCheckKind.sandbox, DiagnosticCodes.workDirRefused),
      findsNothing,
    );
  });
}
