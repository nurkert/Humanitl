// Die Wegwerf-Sandbox als Vorgabe (HUM-357): Der Schalter im Kopf folgt dem
// Modus, den der Daemon für einen Start ohne Wahl nennt, der Start schickt
// nur eine Wahl, die von dieser Vorgabe abweicht, und ein Rückfall auf
// `direct` sagt im Hinweis des Schalters, warum.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/ui.dart';

import 'harness.dart';

/// Der Befund, den der Daemon nennt, wenn `auto` auf `direct` zurückfällt.
const Diagnostic fellBack = Diagnostic(
  code: 'SANDBOX_033',
  severity: Severity.warning,
  title: 'Disposable sandbox not available, running direct',
  why:
      'the sandbox backend is version 0.9.0; a disposable sandbox needs '
      '--overlay; sandbox.mode is not set (auto), so this run writes to the '
      'project folder directly',
);

/// Ein Daemon mit `sandbox.disposable`, dessen Momentaufnahme [effective]
/// und [notice] nennt, ohne wartende Änderungen.
SandboxTestClient clientWith(SandboxMode effective, {Diagnostic? notice}) {
  final SandboxTestClient client = SandboxTestClient(
    info: FakeDaemonClient.disposableInfo,
  );
  client
    ..autoSandboxMode = effective
    ..autoSandboxNotice = notice;
  return client;
}

Future<void> settle(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 600));
}

bool switchOn(WidgetTester tester) =>
    tester.widget<HCheckbox>(find.byKey(const Key('sandbox-disposable'))).value;

String switchHint(WidgetTester tester) => tester
    .widget<HoverLabel>(
      find.ancestor(
        of: find.byKey(const Key('sandbox-disposable')),
        matching: find.byType(HoverLabel),
      ),
    )
    .label;

Future<void> startAndStop(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('sandbox-start')));
  await settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-stop')));
  await settle(tester);
  await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
  await settle(tester);
}

void main() {
  testWidgets('the switch starts at the mode the daemon resolved, and the '
      'start sends nothing unless it was changed', (WidgetTester tester) async {
    final SandboxTestClient client = clientWith(SandboxMode.disposable);
    await pumpSandbox(tester, client: client);

    expect(switchOn(tester), isTrue, reason: 'auto is disposable here');
    await startAndStop(tester);
    expect(client.startModes, <SandboxMode?>[null]);

    // Abgewählt: eine Wahl, die von der Vorgabe abweicht, geht mit.
    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    expect(switchOn(tester), isFalse);
    await startAndStop(tester);
    expect(client.startModes, <SandboxMode?>[null, SandboxMode.direct]);

    // Wieder an: gleich der Vorgabe, also wieder ein leeres Feld. Ein
    // ausdrückliches `disposable` fiele sonst nie mehr zurück.
    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    expect(switchOn(tester), isTrue);
    await startAndStop(tester);
    expect(client.startModes, <SandboxMode?>[null, SandboxMode.direct, null]);
  });

  testWidgets('a default that fell back to direct is off and says why', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = clientWith(
      SandboxMode.direct,
      notice: fellBack,
    );
    await pumpSandbox(tester, client: client);

    expect(switchOn(tester), isFalse);
    expect(switchHint(tester), contains('version 0.9.0'));
    expect(switchHint(tester), startsWith('Disposable is not available here'));

    // Wer selbst wählt, braucht den Grund nicht mehr.
    await tester.tap(find.byKey(const Key('sandbox-disposable')));
    await settle(tester);
    expect(switchHint(tester), isNot(contains('version 0.9.0')));
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(client.startModes, <SandboxMode?>[SandboxMode.disposable]);
  });

  testWidgets('an explicit sandbox.mode wins over the snapshot until the '
      'next plan', (WidgetTester tester) async {
    final SandboxTestClient client = clientWith(SandboxMode.disposable);
    client.configValues = const ConfigValues(
      values: <String, Object?>{
        'sandbox': <String, Object?>{'mode': 'direct'},
      },
    );
    await pumpSandbox(tester, client: client);

    expect(switchOn(tester), isFalse);
    await tester.tap(find.byKey(const Key('sandbox-start')));
    await settle(tester);
    expect(client.startModes, <SandboxMode?>[null]);
  });
}
