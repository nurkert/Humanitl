// Ein Projektordner aus den Einstellungen erreicht Sandbox-Kopf und Setup,
// ohne dass der Bildschirm wechselt (HUM-295).
//
// Die Einstellungen schreiben `sandbox.work_dir` über `configProvider`; die
// Momentaufnahme der Sandbox plant neu, sobald sich der gespeicherte Ordner
// ändert. Ohne diesen Plan zeigte der Kopf den alten Ordner, bis der
// Sandbox-Bereich wieder sichtbar würde.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';

import '../../harness/app_harness.dart';

/// Der Ordner, der vorher gespeichert ist.
const String before = '/home/u/projects/old';

/// Der Ordner, den die Ordnerauswahl der Einstellungen liefert.
const String after = '/home/u/projects/new';

/// Ein Daemon mit gesunder Maschine, gespeichertem Ordner [before] und einer
/// Sandbox im Zustand [state]; zählt die Pläne und die Statusabfragen.
class FolderClient extends FakeDaemonClient {
  FolderClient({SandboxState state = SandboxState.stopped}) {
    doctorReport = fakeDoctorOk();
    sandbox = SandboxStatus(state: state, workDirHost: before);
    configValues = const ConfigValues(
      values: <String, Object?>{
        'sandbox': <String, Object?>{'work_dir': before},
      },
      configPath: '/home/u/.config/humanitl/config.toml',
    );
  }

  /// Wie oft `Sandbox(Plan)` gerufen wurde.
  int plans = 0;

  @override
  Stream<SandboxUpdate> planSandbox({String? profile, WorkMode? workMode}) {
    plans++;
    return super.planSandbox(profile: profile, workMode: workMode);
  }
}

/// Baut die App, öffnet [digit] als Bereich und dann die Einstellungen.
Future<void> openSettingsOver(
  WidgetTester tester,
  FolderClient client,
  LogicalKeyboardKey digit, {
  Future<void> Function(WidgetTester)? beforeSheet,
}) async {
  await pumpApp(
    tester,
    client: client,
    overrides: [directoryChooserProvider.overrideWithValue(() async => after)],
  );
  await pressCtrl(tester, digit);
  await tester.pumpAndSettle();
  await beforeSheet?.call(tester);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
}

/// Wählt den Ordner in den Einstellungen und lässt `SetConfig`, das neue
/// Lesen der Konfiguration und den Plan durchlaufen.
Future<void> chooseInSettings(WidgetTester tester) async {
  final Finder choose = find.byKey(const Key('settings-workdir-choose'));
  await tester.ensureVisible(choose);
  await tester.pumpAndSettle();
  await tester.tap(choose);
  await tester.pumpAndSettle();
}

String pathText(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(ValueKey<String>(key))).data!;

void main() {
  testWidgets('a folder chosen in the settings shows in the sandbox header '
      'without switching screens', (WidgetTester tester) async {
    final FolderClient client = FolderClient();
    await openSettingsOver(tester, client, LogicalKeyboardKey.digit4);
    expect(pathText(tester, 'sandbox-workdir-path'), before);
    // Gezählt seit dem Bau des Fakes: Ein Plan auf den ersten gelesenen
    // Ordner stünde schon hier.
    expect(client.plans, 0, reason: 'the folder read first is no change');

    await chooseInSettings(tester);

    expect(
      tester.widget<Text>(find.byKey(const Key('settings-workdir'))).data,
      after,
    );
    expect(pathText(tester, 'sandbox-workdir-path'), after);
    expect(client.plans, 1, reason: 'exactly one new plan');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a folder chosen in the settings shows in the setup row '
      'without switching screens', (WidgetTester tester) async {
    final FolderClient client = FolderClient();
    await openSettingsOver(
      tester,
      client,
      LogicalKeyboardKey.digit6,
      beforeSheet: (WidgetTester tester) async {
        await openSetupStep(tester, 'project');
        await tester.pumpAndSettle();
      },
    );
    expect(pathText(tester, 'setup-workdir-path'), before);

    await chooseInSettings(tester);

    expect(pathText(tester, 'setup-workdir-path'), after);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a running sandbox keeps its folder and is not planned again', (
    WidgetTester tester,
  ) async {
    final FolderClient client = FolderClient(state: SandboxState.running);
    await openSettingsOver(tester, client, LogicalKeyboardKey.digit4);
    // Gezählt seit dem Bau des Fakes: Ein Plan auf den ersten gelesenen
    // Ordner stünde schon hier.
    expect(client.plans, 0, reason: 'the folder read first is no change');

    await chooseInSettings(tester);

    expect(
      tester.widget<Text>(find.byKey(const Key('settings-workdir'))).data,
      after,
    );
    expect(client.plans, 0);
    expect(pathText(tester, 'sandbox-workdir-path'), before);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
