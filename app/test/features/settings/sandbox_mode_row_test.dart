// Die Zeile `sandbox.mode` im Einstellungsformular (HUM-357): automatisch,
// direkt oder Wegwerf, geschrieben über `SetConfig`, und darunter, was der
// Daemon für dieses System daraus macht, samt Grund eines Rückfalls.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';

import '../../harness/app_harness.dart';

const Diagnostic fellBack = Diagnostic(
  code: 'SANDBOX_033',
  severity: Severity.warning,
  title: 'Disposable sandbox not available, running direct',
  why:
      '/run/user/1000/humanitl/disposable is not on tmpfs; sandbox.mode is '
      'not set (auto), so this run writes to the project folder directly',
);

FakeDaemonClient clientWith({
  FakeDaemonClient? base,
  SandboxMode? effective,
  Diagnostic? notice,
  String? written,
}) {
  final FakeDaemonClient client =
      base ??
      FakeDaemonClient(
        info: FakeDaemonClient.disposableInfo,
        script: const <ScriptedEvent>[],
      );
  client
    ..autoSandboxMode = effective ?? SandboxMode.direct
    ..autoSandboxNotice = notice;
  if (written != null) {
    client.configValues = ConfigValues(
      values: <String, Object?>{
        'sandbox': <String, Object?>{'mode': written},
      },
    );
  }
  return client;
}

Future<void> openSettings(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
}

Finder get row => find.byKey(const Key('settings-sandbox-mode'));

void main() {
  testWidgets('the row shows auto by default with what it resolves to, and '
      'writes the chosen word', (WidgetTester tester) async {
    final FakeDaemonClient client = clientWith(
      effective: SandboxMode.disposable,
    );
    await openSettings(tester, client);

    expect(tester.widget<HSegmented<String>>(row).selected, 'auto');
    expect(
      find.text('On this system the next start is disposable.'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('settings-sandbox-mode-reason')), findsNothing);

    final Finder direct = find.descendant(
      of: row,
      matching: find.text('Direct'),
    );
    await tester.ensureVisible(direct);
    await tester.pumpAndSettle();
    await tester.tap(direct);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, ('sandbox.mode', 'direct'));
    expect(tester.widget<HSegmented<String>>(row).selected, 'direct');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a default that fell back names the reason under the row', (
    WidgetTester tester,
  ) async {
    await openSettings(
      tester,
      clientWith(effective: SandboxMode.direct, notice: fellBack),
    );

    expect(tester.widget<HSegmented<String>>(row).selected, 'auto');
    expect(
      find.text('On this system the next start is direct.'),
      findsOneWidget,
    );
    final Text reason = tester.widget<Text>(
      find.byKey(const Key('settings-sandbox-mode-reason')),
    );
    expect(reason.data, contains('not on tmpfs'));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('writing the mode brings the resolved mode and reason up to '
      'date', (WidgetTester tester) async {
    // HUM-357, Review: Nach dem Schreiben stand im Formular sonst weiter der
    // alte aufgelöste Modus samt Grund. Mutationsbeweis: `storedModeRead`
    // ohne neue Momentaufnahme, rot.
    final FakeDaemonClient client = clientWith(
      effective: SandboxMode.direct,
      notice: fellBack,
    );
    await openSettings(tester, client);
    expect(
      find.byKey(const Key('settings-sandbox-mode-reason')),
      findsOneWidget,
    );

    final Finder disposable = find.descendant(
      of: row,
      matching: find.text('Disposable'),
    );
    await tester.ensureVisible(disposable);
    await tester.pumpAndSettle();
    await tester.tap(disposable);
    await tester.pumpAndSettle();
    expect(client.configWrites.last, ('sandbox.mode', 'disposable'));
    expect(
      find.text('On this system the next start is disposable.'),
      findsOneWidget,
    );
    expect(find.byKey(const Key('settings-sandbox-mode-reason')), findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('an explicit disposable is selected as written', (
    WidgetTester tester,
  ) async {
    await openSettings(
      tester,
      clientWith(effective: SandboxMode.disposable, written: 'disposable'),
    );
    expect(tester.widget<HSegmented<String>>(row).selected, 'disposable');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('without the capability there is no row', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, FakeDaemonClient.empty());
    expect(row, findsNothing);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
