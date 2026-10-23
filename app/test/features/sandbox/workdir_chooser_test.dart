// Der Kopf des Sandbox-Bereichs hat denselben sichtbaren Ordnerknopf wie das
// Setup (HUM-282): Pfad als Text, daneben „Choose folder…" oder „Change…",
// gesperrt mit dem Satz des Pickers, solange eine Sitzung läuft. Ein
// Ordnerdialog, der nicht aufgeht, wird eine Karte `UI_003`.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Der Knopf des Pickers im Kopf.
final Finder pickerButton = find.byKey(const Key('sandbox-workdir'));

/// Der Pfad neben dem Knopf.
final Finder pickerPath = find.byKey(
  const ValueKey<String>('sandbox-workdir-path'),
);

/// Ein Client mit [state] und [workDir] als Projektordner.
SandboxTestClient clientWith(SandboxState state, {String? workDir}) {
  final SandboxTestClient client = SandboxTestClient();
  client.sandbox = client.sandbox.copyWith(state: state, workDirHost: workDir);
  return client;
}

void main() {
  testWidgets('without a folder the header shows "Choose folder…"', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = clientWith(SandboxState.stopped);
    await pumpSandbox(
      tester,
      client: client,
      chosenDirectory: '/home/nik/clients/beta',
    );

    final HButton button = tester.widget<HButton>(pickerButton);
    expect(button.variant, isNot(HButtonVariant.ghost));
    expect(button.enabled, isTrue);
    expect(
      find.descendant(of: pickerButton, matching: find.text('Choose folder…')),
      findsOneWidget,
    );
    expect(tester.widget<Text>(pickerPath).data, 'No project folder');

    await tester.tap(pickerButton);
    await tester.pump();
    await tester.pump();

    expect(client.plans, <(String?, WorkMode?)>[
      ('/home/nik/clients/beta', null),
    ]);
  });

  testWidgets('with a folder the header reads "Change…" beside the path', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: clientWith(SandboxState.stopped, workDir: '/home/nik/clients/a'),
    );

    expect(
      find.descendant(of: pickerButton, matching: find.text('Change…')),
      findsOneWidget,
    );
    expect(tester.widget<Text>(pickerPath).data, '/home/nik/clients/a');
    expect(
      find.ancestor(of: pickerPath, matching: find.byType(HButton)),
      findsNothing,
    );
  });

  testWidgets('a narrow header keeps the button as its glyph, label on hover', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: clientWith(SandboxState.stopped, workDir: '/home/nik/clients/a'),
      size: const Size(900, 600),
    );

    expect(tester.takeException(), isNull);
    expect(tester.widget<HButton>(pickerButton).enabled, isTrue);
    expect(
      find.descendant(of: pickerButton, matching: find.text('Change…')),
      findsNothing,
    );
    expect(
      find.ancestor(
        of: pickerButton,
        matching: find.byWidgetPredicate(
          (Widget w) => w is HoverLabel && w.label == 'Change…',
        ),
      ),
      findsOneWidget,
    );
  });

  testWidgets('while a session runs the button is locked and says why', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = clientWith(
      SandboxState.running,
      workDir: '/home/nik/clients/a',
    );
    await pumpSandbox(
      tester,
      client: client,
      chosenDirectory: '/home/nik/clients/beta',
    );

    expect(tester.widget<HButton>(pickerButton).enabled, isFalse);
    expect(
      find.ancestor(
        of: pickerButton,
        matching: find.byWidgetPredicate(
          (Widget w) =>
              w is HoverLabel &&
              w.label ==
                  'The project folder cannot change while the sandbox runs.',
        ),
      ),
      findsOneWidget,
    );
    await tester.tap(pickerButton, warnIfMissed: false);
    await tester.pump();
    expect(client.plans, isEmpty);
  });

  testWidgets('a failing folder dialog shows UI_003 with the command line', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = clientWith(SandboxState.stopped);
    await pumpSandbox(
      tester,
      client: client,
      chooser: () async => throw PlatformException(code: 'portal'),
    );
    expect(find.text(DiagnosticCodes.folderDialogFailed), findsNothing);

    await tester.tap(pickerButton);
    await tester.pump();
    await tester.pump();

    expect(tester.takeException(), isNull);
    expect(client.plans, isEmpty);
    expect(find.text(DiagnosticCodes.folderDialogFailed), findsOneWidget);
    expect(find.text('The folder dialog did not open'), findsOneWidget);
    expect(find.text('humanitl run'), findsOneWidget);
    // Der übersetzte Satz ist der Grund, der rohe Text der Plattform steht
    // darunter.
    expect(
      find.text(
        'The desktop could not show a folder dialog. Open a terminal in the '
        'project folder and run humanitl run there.',
      ),
      findsOneWidget,
    );
    expect(find.textContaining('PlatformException(portal'), findsOneWidget);
  });

  testWidgets('a dialog failure does not stand while a session runs', (
    WidgetTester tester,
  ) async {
    // Gescheitert im Setup, dann über `humanitl run` gestartet: Der Picker
    // ist gesperrt, und eine Karte, die niemand mehr wegbekommt, bliebe
    // stehen.
    await pumpSandbox(
      tester,
      client: clientWith(SandboxState.running, workDir: '/home/nik/clients/a'),
    );
    ProviderScope.containerOf(tester.element(find.byType(SandboxScreen)))
        .read(folderDialogFailureProvider.notifier)
        .failed(PlatformException(code: 'portal'));
    await tester.pump();

    expect(find.text(DiagnosticCodes.folderDialogFailed), findsNothing);
  });

  testWidgets('the header does not overflow at double text size', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(
      tester,
      client: clientWith(SandboxState.stopped, workDir: '/home/nik/clients/a'),
      textScaler: const TextScaler.linear(2),
    );

    expect(tester.takeException(), isNull);
    expect(pickerButton, findsOneWidget);
  });
}
