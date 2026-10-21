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
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
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
/// `MountPolicy::check_work_dir`.
Diagnostic refusal(String dir) => Diagnostic(
  code: DiagnosticCodes.workDirRefused,
  severity: Severity.blocking,
  why:
      'sandbox.work_dir names $dir: $dir contains $dir/.config/humanitl, '
      "Humanitl's own configuration directory: config.toml, rules.yaml and "
      'the profiles; choose the folder of a single project instead, such as '
      '~/projects/xyz',
  fix: const FixAction.changeSetting(key: 'sandbox.work_dir', value: ''),
);

/// Ein Daemon, in dessen `config.toml` das Heimatverzeichnis als
/// Projektordner steht, und der es so ablehnt wie der echte.
class HomeRefusingClient extends FakeDaemonClient {
  /// Erzeugt den Fake mit gesunder Maschine und [home] als Projektordner.
  HomeRefusingClient() {
    doctorReport = fakeDoctorOk();
    sandbox = sandbox.copyWith(workDirHost: home);
  }

  /// Jeder Ordner, nach dem `Sandbox(Plan)` gefragt wurde.
  final List<String?> planned = <String?>[];

  @override
  Stream<SandboxUpdate> sandboxStatus() async* {
    if (sandbox.workDirHost == home) {
      // Wie `snapshot_or_diagnostic`: erst der Befund, dann der Zustand
      // `failed`, der den abgelehnten Ordner weiter nennt.
      yield SandboxUpdate.diagnostic(refusal(home));
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
    String? workDir,
    WorkMode? workMode,
  }) async* {
    planned.add(workDir);
    final String? effective = workDir ?? sandbox.workDirHost;
    if (effective == home) {
      yield SandboxUpdate.diagnostic(refusal(home));
      yield SandboxUpdate.status(sandbox.copyWith(state: SandboxState.failed));
      return;
    }
    yield* super.planSandbox(
      profile: profile,
      workDir: workDir,
      workMode: workMode,
    );
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
    await tester.pump();
    await tester.pump();

    expect(asked, 1);
    expect(client.planned, <String?>[project]);
    // Der neue Ordner kommt durch, die Karte verschwindet mit ihrem Befund.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(chooseFix, findsNothing);
  });

  testWidgets('while a sandbox runs the fix offers no chooser', (
    WidgetTester tester,
  ) async {
    // Gesperrt wie der Picker: Der Ordner einer laufenden Sitzung ändert sich
    // nicht, und ein Knopf, der dann wählen ließe, täte nichts.
    final HomeRefusingClient client = HomeRefusingClient();
    client.sandbox = client.sandbox.copyWith(
      state: SandboxState.running,
      sandboxId: FakeDaemonClient.defaultSandbox,
    );
    await openSetup(tester, client, chooser: () async => project);

    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(chooseFix, findsNothing);
    expect(find.text('Change sandbox.work_dir'), findsOneWidget);
  });

  testWidgets('a cancelled chooser changes nothing', (
    WidgetTester tester,
  ) async {
    final HomeRefusingClient client = HomeRefusingClient();
    await openSetup(tester, client, chooser: () async => null);

    await tester.tap(chooseFix);
    await tester.pump();

    expect(client.planned, isEmpty);
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
  });

  testWidgets('picking the home folder in the picker shows the same refusal', (
    WidgetTester tester,
  ) async {
    // Der Weg über den Picker endete schon vorher in `SANDBOX_006`; jetzt
    // trägt die Karte dort dieselbe Abhilfe wie beim Öffnen.
    final HomeRefusingClient client = HomeRefusingClient()
      ..sandbox = defaultSandbox_().copyWith(workDirHost: project);
    await openSetup(tester, client, chooser: () async => home);
    expect(stateOf(tester, SetupCheckKind.project), 'ok');

    await tester.tap(find.byKey(const Key('setup-workdir')));
    await tester.pump();
    await tester.pump();

    expect(client.planned, <String?>[home]);
    expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');
    expect(chooseFix, findsOneWidget);
  });
}
