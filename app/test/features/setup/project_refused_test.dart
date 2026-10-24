// Ein Projektordner, den der Start ablehnt, steht schon im Setup als
// `SANDBOX_006`, mit der Abhilfe, einen anderen Ordner zu wählen (HUM-262).
//
// Der Befund vom Bildschirmfoto: Das Setup zeigte „Project folder ok
// /home/nburkert", erst der Start lehnte ab, und die Karte hatte keinen Fix.
// Der Daemon prüft den Ordner jetzt schon in `Sandbox(Status)` und
// `Sandbox(Plan)` mit derselben Regel wie der Start; der Fake unten antwortet
// so, wie er es tut, Satz und Abhilfe eingeschlossen
// (`daemon/crates/ipc/tests/sandbox_rpc.rs`,
// `a_configured_folder_that_holds_the_config_is_refused_by_the_plan`).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import '../../harness/app_harness.dart';

/// Das Heimatverzeichnis des gespielten Rechners.
const String home = '/home/u';

/// Der Ordner, den die Ordnerauswahl im Test liefert.
const String project = '/home/u/projects/app';

/// Der Befund, den der Daemon für [dir] schickt, wenn [dir] die eigene
/// Konfiguration enthält: Code, Schwere, Satz und Abhilfe wie in
/// `MountPolicy::check_work_dir_from`. [source] ist die Quelle des Pfades,
/// die der Satz seit HUM-293 nennt.
Diagnostic refusal(String dir, String source) => Diagnostic(
  code: DiagnosticCodes.workDirRefused,
  severity: Severity.blocking,
  why:
      'the project directory from $source names $dir: $dir contains '
      "$dir/.config/humanitl, Humanitl's own configuration directory: "
      'config.toml, rules.yaml and the profiles; choose the folder of a '
      'single project instead, such as ~/projects/xyz',
  fix: const FixAction.changeSetting(key: 'sandbox.work_dir', value: ''),
);

/// Die Quelle eines gespeicherten Ordners im Satz des Daemons.
const String storedSource =
    'sandbox.work_dir in $home/.config/humanitl/config.toml';

/// Ein Daemon, in dessen `config.toml` das Heimatverzeichnis als
/// Projektordner steht, und der es so ablehnt wie der echte: beim Plan, und
/// schon beim Speichern (`SetConfig`, HUM-293).
class HomeRefusingClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit gesunder Maschine und [home] als Projektordner.
  HomeRefusingClient() {
    doctorReport = fakeDoctorOk();
    sandbox = sandbox.copyWith(workDirHost: home);
  }

  /// Jeder Ordner, den die Oberfläche als `sandbox.work_dir` speichern
  /// wollte, auch ein abgelehnter.
  final List<String> asked = <String>[];

  /// Jeder Ordner, den `SetConfig` angenommen hat.
  List<String> get stored => <String>[
    for (final (String key, String value) in configWrites)
      if (key == FormKeys.sandboxWorkDir) value,
  ];

  @override
  Future<void> setConfig(
    String key,
    String value, {
    String? expectedServer,
  }) async {
    if (key == FormKeys.sandboxWorkDir) {
      asked.add(value);
      if (value == home) {
        // Wie `config_rpc::check_chosen_work_dir`: abgelehnt, nichts
        // geschrieben.
        throw DaemonException(refusal(home, 'the folder just chosen'));
      }
    }
    await super.setConfig(key, value, expectedServer: expectedServer);
  }

  @override
  Stream<SandboxUpdate> sandboxStatus() async* {
    if (sandbox.workDirHost == home) {
      // Wie `snapshot_or_diagnostic`: erst der Befund, dann der Zustand
      // `failed`, der den abgelehnten Ordner weiter nennt.
      yield SandboxUpdate.diagnostic(refusal(home, storedSource));
      // Eine Sitzung, die noch läuft, bleibt `running`; sonst `failed`.
      yield SandboxUpdate.status(
        sandbox.isUp ? sandbox : sandbox.copyWith(state: SandboxState.failed),
      );
      return;
    }
    yield* super.sandboxStatus();
  }

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    WorkMode? workMode,
  }) async* {
    if (sandbox.workDirHost == home) {
      yield SandboxUpdate.diagnostic(refusal(home, storedSource));
      yield SandboxUpdate.status(sandbox.copyWith(state: SandboxState.failed));
      return;
    }
    yield* super.planSandbox(profile: profile, workMode: workMode);
  }
}

/// Der Text der Zustandszeile einer Zeile.
String stateOf(WidgetTester tester, SetupCheckKind kind) =>
    tester.widget<Text>(find.byKey(Key('setup-state-${kind.name}'))).data!;

/// Ob der Start-Knopf gedrückt werden kann.
bool startEnabled(WidgetTester tester) =>
    tester.widget<HButton>(find.byKey(const Key('setup-start'))).enabled;

/// Der Knopf der Abhilfe auf der Karte des Projektordners.
final Finder chooseFix = find.descendant(
  of: find.byType(SetupScreen),
  matching: find.byKey(const Key('fix-choose-folder')),
);

/// Baut die App über [client] und öffnet das Setup.
Future<void> openSetup(
  WidgetTester tester,
  HomeRefusingClient client, {
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

/// Lässt Dialog, `SetConfig`, das neue Lesen der Konfiguration und den Plan
/// durchlaufen; jeder Schritt ist ein eigener Mikrotask.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

void main() {
  testWidgets('a home folder that holds the config shows SANDBOX_006 before '
      'the start, with a fix', (WidgetTester tester) async {
    final HomeRefusingClient client = HomeRefusingClient();
    await openSetup(tester, client, chooser: () async => project);

    // Nicht „ok": Die Zeile sperrt den Start, bevor jemand ihn versucht.
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(startEnabled(tester), isFalse);
    expect(
      find.descendant(
        of: find.byType(SetupScreen),
        matching: find.text(DiagnosticCodes.workDirRefused),
      ),
      findsOneWidget,
    );
    // Der Satz des Daemons sagt, warum und was stattdessen.
    expect(find.textContaining('~/projects/xyz'), findsOneWidget);
    // Und die Karte trägt eine Abhilfe, die etwas tut, kein Abzeichen.
    expect(chooseFix, findsOneWidget);
    expect(find.text('Choose another folder'), findsOneWidget);
    expect(find.text('Change sandbox.work_dir'), findsNothing);
  });

  testWidgets('the fix opens the folder chooser and plans the chosen folder', (
    WidgetTester tester,
  ) async {
    final HomeRefusingClient client = HomeRefusingClient();
    int asked = 0;
    await openSetup(
      tester,
      client,
      chooser: () async {
        asked += 1;
        return project;
      },
    );

    await tester.tap(chooseFix);
    await settle(tester);

    expect(asked, 1);
    expect(client.stored, <String>[project]);
    // Der neue Ordner kommt durch, die Karte verschwindet mit ihrem Befund.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(chooseFix, findsNothing);
  });

  testWidgets('while a sandbox runs the fix is locked and says why', (
    WidgetTester tester,
  ) async {
    // Gesperrt wie der Picker: Der Ordner einer laufenden Sitzung ändert sich
    // nicht. Der Knopf bleibt stehen, ist aus und sagt beim Verweilen den
    // Satz des gesperrten Pickers (HUM-262, HUM-282).
    final HomeRefusingClient client = HomeRefusingClient();
    int asked = 0;
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
    await openSetup(
      tester,
      client,
      chooser: () async {
        asked += 1;
        return project;
      },
    );

    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(chooseFix, findsOneWidget);
    expect(tester.widget<HButton>(chooseFix).enabled, isFalse);
    expect(find.text('Change sandbox.work_dir'), findsNothing);
    expect(
      find.ancestor(
        of: chooseFix,
        matching: find.byWidgetPredicate(
          (Widget w) =>
              w is HoverLabel &&
              w.label ==
                  'The project folder cannot change while the sandbox runs.',
        ),
      ),
      findsOneWidget,
    );
    await tester.tap(chooseFix, warnIfMissed: false);
    await tester.pump();
    expect(asked, 0);
    expect(client.asked, isEmpty);
  });

  testWidgets('a cancelled chooser changes nothing', (
    WidgetTester tester,
  ) async {
    final HomeRefusingClient client = HomeRefusingClient();
    await openSetup(tester, client, chooser: () async => null);

    await tester.tap(chooseFix);
    await settle(tester);

    expect(client.asked, isEmpty);
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
  });

  testWidgets('a refused folder is not stored and shows SANDBOX_006 in the '
      'step', (WidgetTester tester) async {
    // Der Weg über den Picker (HUM-293): Die Wahl geht als `sandbox.work_dir`
    // an `SetConfig`, der Daemon lehnt das Heimatverzeichnis ab, und nichts
    // wird gespeichert. Der Befund steht im Schritt mit derselben Abhilfe
    // wie beim Öffnen, und der Ordner davor bleibt der gespeicherte.
    final HomeRefusingClient client = HomeRefusingClient()
      ..sandbox = defaultSandbox_().copyWith(workDirHost: project);
    await openSetup(tester, client, chooser: () async => home);
    expect(stateOf(tester, SetupCheckKind.project), 'ok');

    // Grün steht der Schritt als eine Zeile (HUM-287); der Picker steht im
    // geöffneten Schritt.
    await openSetupStep(tester, 'project');
    await tester.tap(find.byKey(const Key('setup-workdir')));
    await settle(tester);

    expect(client.asked, <String>[home], reason: 'SetConfig was called');
    expect(client.stored, isEmpty, reason: 'the refused folder is not stored');
    expect(client.sandbox.workDirHost, project);
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(
      find.descendant(
        of: find.byType(SetupScreen),
        matching: find.text(DiagnosticCodes.workDirRefused),
      ),
      findsOneWidget,
    );
    expect(find.textContaining('the folder just chosen'), findsOneWidget);
    expect(chooseFix, findsOneWidget);
  });

  testWidgets('choosing a folder stores it with SetConfig', (
    WidgetTester tester,
  ) async {
    // HUM-293: Der Ordner hat eine Quelle, `sandbox.work_dir`. Ohne den
    // Aufruf von `SetConfig` startete die Kommandozeile woanders als die
    // Oberfläche.
    final HomeRefusingClient client = HomeRefusingClient()
      ..sandbox = defaultSandbox_().copyWith(workDirHost: '/home/u/old');
    await openSetup(tester, client, chooser: () async => project);
    await openSetupStep(tester, 'project');
    await tester.tap(find.byKey(const Key('setup-workdir')));
    await settle(tester);

    expect(client.configWrites, contains((FormKeys.sandboxWorkDir, project)));
    expect(client.sandbox.workDirHost, project);
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
  });
}
