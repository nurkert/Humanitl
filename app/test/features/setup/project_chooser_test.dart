// Der Projektordner hat im Setup einen sichtbaren Knopf (HUM-282).
//
// Auf v0.0.6 stand in der Zeile „No project folder" als Geisterknopf, den
// niemand als Weg zur Ordnerauswahl erkannte, und die Karte `CONFIG_013`
// darunter bot keine Handlung. Jetzt ist der Pfad ein Text, daneben steht ein
// Knopf mit Ordnersymbol, und die Karte trägt denselben Weg als Knopf. Ein
// Ordnerdialog, der nicht aufgeht, wird eine Karte `UI_003` mit der
// Kommandozeile als Ausweg.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import '../../harness/app_harness.dart';

/// Der Ordner, den die Ordnerauswahl im Test liefert.
const String project = '/home/u/projects/app';

/// Ein Daemon ohne Projektordner, der jede Planung mitschreibt.
class NoFolderClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit gesunder Maschine und ohne Ordner.
  NoFolderClient() {
    doctorReport = fakeDoctorOk();
    sandbox = const SandboxStatus();
  }

  /// Jeder Ordner, nach dem `Sandbox(Plan)` gefragt wurde.
  final List<String?> planned = <String?>[];

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    String? workDir,
    WorkMode? workMode,
  }) {
    planned.add(workDir);
    return super.planSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
  }
}

/// Der Knopf des Pickers in der Projektzeile.
final Finder pickerButton = find.byKey(const Key('setup-workdir'));

/// Der Knopf der Abhilfe auf der Karte des Projektordners.
final Finder cardButton = find.descendant(
  of: find.byType(SetupScreen),
  matching: find.byKey(const Key('fix-choose-folder')),
);

/// Der Text der Zustandszeile einer Zeile.
String stateOf(WidgetTester tester, SetupCheckKind kind) =>
    tester.widget<Text>(find.byKey(Key('setup-state-${kind.name}'))).data!;

/// Baut die App über [client] und öffnet das Setup.
Future<void> openSetup(
  WidgetTester tester,
  NoFolderClient client, {
  required Future<String?> Function() chooser,
}) async {
  await pumpApp(
    tester,
    client: client,
    overrides: [directoryChooserProvider.overrideWithValue(chooser)],
  );
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
}

/// Drückt [finder] und lässt Dialog und Planung durchlaufen.
Future<void> press(WidgetTester tester, Finder finder) async {
  await tester.ensureVisible(finder);
  await tester.pump();
  await tester.tap(finder);
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('without a folder the row shows a visible "Choose folder…" '
      'button beside the path label', (WidgetTester tester) async {
    final NoFolderClient client = NoFolderClient();
    int asked = 0;
    await openSetup(
      tester,
      client,
      chooser: () async {
        asked += 1;
        return project;
      },
    );

    // Ohne Ordner steht genau ein „Choose folder" im offenen Schritt: der
    // Knopf des Befunds, als Primärknopf (Owner, 2026-10-24). Der Picker mit
    // seinem zweiten Knopf für dieselbe Wahl steht erst, wenn ein Ordner gilt.
    expect(pickerButton, findsNothing);
    final HButton button = tester.widget<HButton>(cardButton);
    expect(button.variant, HButtonVariant.primary);
    expect(button.enabled, isTrue);
    expect(find.text('Choose folder'), findsOneWidget);
    expect(find.text('Choose folder…'), findsNothing);

    await press(tester, cardButton);

    expect(asked, 1);
    expect(client.planned, <String?>[project]);
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    // Grün steht der Schritt als eine Zeile (HUM-287); wer den Ordner ändern
    // will, öffnet ihn. Mit Ordner heißt der Knopf „Change…", und der Pfad
    // steht daneben, ganz beim Verweilen.
    await openSetupStep(tester, 'project');
    expect(
      find.descendant(of: pickerButton, matching: find.text('Change…')),
      findsOneWidget,
    );
    final Finder path = find.byKey(
      const ValueKey<String>('setup-workdir-path'),
    );
    expect(tester.widget<Text>(path).data, project);
    expect(
      find.ancestor(of: path, matching: find.byType(HButton)),
      findsNothing,
    );
    expect(
      find.ancestor(
        of: path,
        matching: find.byWidgetPredicate(
          (Widget w) => w is HoverLabel && w.label == project,
        ),
      ),
      findsOneWidget,
    );
  });

  testWidgets('the CONFIG_013 card offers "Choose folder" and it opens the '
      'dialog', (WidgetTester tester) async {
    final NoFolderClient client = NoFolderClient();
    int asked = 0;
    await openSetup(
      tester,
      client,
      chooser: () async {
        asked += 1;
        return project;
      },
    );

    expect(find.text(DiagnosticCodes.noProjectFolder), findsOneWidget);
    expect(cardButton, findsOneWidget);
    expect(
      find.descendant(of: cardButton, matching: find.text('Choose folder')),
      findsOneWidget,
    );
    expect(find.text('Change sandbox.work_dir'), findsNothing);

    await press(tester, cardButton);

    expect(asked, 1);
    expect(client.planned, <String?>[project]);
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(find.text(DiagnosticCodes.noProjectFolder), findsNothing);
  });

  testWidgets('a cancelled dialog changes nothing and reports nothing', (
    WidgetTester tester,
  ) async {
    final NoFolderClient client = NoFolderClient();
    await openSetup(tester, client, chooser: () async => null);

    await press(tester, cardButton);

    expect(client.planned, isEmpty);
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(find.text(DiagnosticCodes.folderDialogFailed), findsNothing);
  });

  testWidgets('a failing folder dialog shows UI_003 with the command line', (
    WidgetTester tester,
  ) async {
    final NoFolderClient client = NoFolderClient();
    bool fail = true;
    await openSetup(
      tester,
      client,
      chooser: () async {
        if (fail) {
          throw PlatformException(code: 'portal', message: 'no portal');
        }
        return project;
      },
    );

    await press(tester, cardButton);

    expect(tester.takeException(), isNull);
    expect(client.planned, isEmpty);
    expect(find.text(DiagnosticCodes.folderDialogFailed), findsOneWidget);
    expect(find.text('The folder dialog did not open'), findsOneWidget);
    // Der Ausweg steht als Kopierzeile da, nicht nur im Satz.
    expect(find.byKey(const Key('setup-fix-copy-notice')), findsOneWidget);
    expect(find.text('humanitl run'), findsOneWidget);
    // Der Befund der Zeile bleibt daneben stehen: Ein Ordner fehlt weiter.
    expect(find.text(DiagnosticCodes.noProjectFolder), findsOneWidget);

    // Geht der Dialog beim nächsten Versuch auf, verschwindet die Karte.
    fail = false;
    await press(tester, cardButton);

    expect(client.planned, <String?>[project]);
    expect(find.text(DiagnosticCodes.folderDialogFailed), findsNothing);
  });

  testWidgets('a dialog failure does not stand while a session runs', (
    WidgetTester tester,
  ) async {
    final NoFolderClient client = NoFolderClient();
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
    await openSetup(tester, client, chooser: () async => project);
    ProviderScope.containerOf(tester.element(find.byType(SetupScreen)))
        .read(folderDialogFailureProvider.notifier)
        .failed(PlatformException(code: 'portal'));
    await tester.pump();

    expect(find.text(DiagnosticCodes.folderDialogFailed), findsNothing);
  });
}
