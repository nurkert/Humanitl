// Die Abhilfe eines abgelehnten Projektordners im Sandbox-Bereich (HUM-262).
//
// Der Bereich hat im Kopf denselben Picker wie das Setup. Ein `SANDBOX_006`
// mit `ChangeSetting` auf `sandbox.work_dir` wird deshalb auch hier ein Knopf,
// der die Ordnerauswahl öffnet und den gewählten Ordner plant, gesperrt wie
// der Picker, solange die Sandbox läuft.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Der Befund, den der Daemon für ein Heimatverzeichnis als Projektordner
/// schickt (`MountPolicy::check_work_dir`).
const Diagnostic refusedHome = Diagnostic(
  code: DiagnosticCodes.workDirRefused,
  severity: Severity.blocking,
  why:
      'sandbox.work_dir names /home/nik: /home/nik contains '
      "/home/nik/.config/humanitl, Humanitl's own configuration directory: "
      'config.toml, rules.yaml and the profiles; choose the folder of a single '
      'project instead, such as ~/projects/xyz',
  fix: FixAction.changeSetting(key: 'sandbox.work_dir', value: ''),
);

/// Ein Client, dessen Schnappschuss den Ordner mit [state] ablehnt.
SandboxTestClient refusingClient(SandboxState state) {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(
    state: state,
    workDirHost: '/home/nik',
    diagnostics: const <Diagnostic>[refusedHome],
  );
  return client;
}

void main() {
  testWidgets('the fix of a refused folder opens the chooser and plans it', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = refusingClient(SandboxState.failed);
    await pumpSandbox(
      tester,
      client: client,
      chosenDirectory: '/home/nik/clients/beta',
    );

    final Finder fix = find.byKey(const Key('fix-choose-folder'));
    expect(fix, findsOneWidget);
    expect(find.text('Change sandbox.work_dir'), findsNothing);

    await tester.tap(fix);
    await tester.pump();
    await tester.pump();

    expect(client.plans, <(String?, WorkMode?)>[
      ('/home/nik/clients/beta', null),
    ]);
  });

  testWidgets('while the sandbox runs the fix is locked and says why', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = refusingClient(SandboxState.running);
    await pumpSandbox(
      tester,
      client: client,
      chosenDirectory: '/home/nik/clients/beta',
    );

    final Finder fix = find.byKey(const Key('fix-choose-folder'));
    expect(fix, findsOneWidget);
    expect(tester.widget<HButton>(fix).enabled, isFalse);
    expect(find.text('Change sandbox.work_dir'), findsNothing);
    expect(
      find.ancestor(
        of: fix,
        matching: find.byWidgetPredicate(
          (Widget w) =>
              w is HoverLabel &&
              w.label ==
                  'The project folder cannot change while the sandbox runs.',
        ),
      ),
      findsOneWidget,
    );
    await tester.tap(fix, warnIfMissed: false);
    await tester.pump();
    expect(client.plans, isEmpty);
  });
}
