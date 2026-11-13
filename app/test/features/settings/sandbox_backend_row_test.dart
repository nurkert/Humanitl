// Die Zeile `sandbox.backend` in den Einstellungen (HUM-440): automatisch oder
// eines der Backends, die `SandboxBackends` zulässt, je mit seiner Stufe,
// geschrieben über `SetConfig`. Ein Backend, das nicht starten kann oder
// unter der Untergrenze liegt, ist nicht wählbar und nennt den Grund des
// Daemons; eines, das der Daemon nicht zulässt (`SANDBOX_048`), steht nicht
// da.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/registry/setting_entry.dart';

import '../../harness/app_harness.dart';
import '../../harness/settings_harness.dart';

/// Ein zweites Backend, das starten kann. Der Name ist erfunden: Die Zeile
/// urteilt über keinen Namen, alles kommt aus der Liste.
const SandboxBackendInfo availableContainer = SandboxBackendInfo(
  name: 'container',
  isolation: IsolationLevel.namespaces,
  available: true,
);

FakeDaemonClient clientWith({
  SandboxBackendInfo? container = availableContainer,
  String? written,
}) {
  final FakeDaemonClient client = FakeDaemonClient(
    script: const <ScriptedEvent>[],
  );
  if (container != null) {
    client.sandboxBackendList = SandboxBackendList(
      minIsolation: IsolationLevel.policy,
      autoBackend: DoctorCheckId.sandboxTool,
      backends: <SandboxBackendInfo>[
        FakeSandboxBackends.defaultSandboxBackends.backends.first,
        container,
      ],
    );
  }
  if (written != null) {
    client.configValues = ConfigValues(
      values: <String, Object?>{
        'sandbox': <String, Object?>{'backend': written},
      },
    );
  }
  return client;
}

Future<void> openSettings(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.comma);
  await tester.pumpAndSettle();
  await showSettingsCategory(tester, SettingsCategory.sandbox);
}

const Key control = Key('settings-sandbox-backend');

Finder get row => find.byKey(control);

HSegment<String> segment(WidgetTester tester, String value) =>
    tester.widget<HSegment<String>>(
      find.descendant(
        of: row,
        matching: find.byWidgetPredicate(
          (Widget w) => w is HSegment<String> && w.option.value == value,
        ),
      ),
    );

String textOf(WidgetTester tester, String key) =>
    tester.widget<Text>(find.byKey(Key(key))).data!;

void main() {
  testWidgets('the row offers auto and every admitted backend with its level, '
      'and writes the chosen name through SetConfig', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = clientWith();
    await openSettings(tester, client);

    final HSegmented<String> segmented = tester.widget<HSegmented<String>>(row);
    expect(segmented.selected, 'auto');
    expect(
      segmented.options.map((HSegmentOption<String> o) => o.value),
      <String>['auto', DoctorCheckId.sandboxTool, 'container'],
    );
    expect(
      textOf(tester, 'settings-sandbox-backend-level-container'),
      'container: shared kernel, own namespaces',
    );

    await tapSegment(tester, control, 'container');
    expect(client.configWrites.last, (FormKeys.sandboxBackend, 'container'));
    expect(tester.widget<HSegmented<String>>(row).selected, 'container');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend that cannot start is not selectable and names the '
      "daemon's reason and fix", (WidgetTester tester) async {
    final FakeDaemonClient client = clientWith(
      container: const SandboxBackendInfo(
        name: 'container',
        isolation: IsolationLevel.namespaces,
        diagnostics: <Diagnostic>[
          Diagnostic(
            code: 'SANDBOX_034',
            severity: Severity.blocking,
            why: 'container was not found on PATH',
            fix: FixAction.copyCommand(command: 'sudo apt install container'),
          ),
        ],
      ),
    );
    await openSettings(tester, client);

    expect(segment(tester, 'container').enabled, isFalse);
    expect(segment(tester, 'auto').enabled, isTrue);
    expect(
      textOf(tester, 'settings-sandbox-backend-reason-container'),
      contains('container was not found on PATH'),
    );
    expect(
      find.byKey(const Key('settings-sandbox-backend-fix-container')),
      findsOneWidget,
    );

    final int before = client.configWrites.length;
    final Finder container = find.descendant(
      of: row,
      matching: find.text('container'),
    );
    await tester.ensureVisible(container);
    await tester.pumpAndSettle();
    await tester.tap(container, warnIfMissed: false);
    await tester.pumpAndSettle();
    expect(client.configWrites, hasLength(before));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend below the lower bound is not selectable either', (
    WidgetTester tester,
  ) async {
    await openSettings(
      tester,
      clientWith(
        container: const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.namespaces,
          diagnostics: <Diagnostic>[
            Diagnostic(
              code: 'SANDBOX_035',
              severity: Severity.blocking,
              why:
                  'container offers namespaces, below '
                  'sandbox.min_isolation kernel',
            ),
          ],
        ),
      ),
    );

    expect(segment(tester, 'container').enabled, isFalse);
    expect(
      textOf(tester, 'settings-sandbox-backend-reason-container'),
      contains('below sandbox.min_isolation'),
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend the daemon does not admit is not offered', (
    WidgetTester tester,
  ) async {
    await openSettings(
      tester,
      clientWith(
        container: const SandboxBackendInfo(
          name: 'container',
          isolation: IsolationLevel.kernel,
          diagnostics: <Diagnostic>[FakeSandboxBackends.notAdmitted],
        ),
      ),
    );

    // Übrig bleibt das Backend des Systems: nichts zu wählen, nur seine Stufe.
    expect(row, findsNothing);
    expect(
      find.byKey(const Key('settings-sandbox-backend-level-container')),
      findsNothing,
    );
    expect(
      find.byKey(
        const Key(
          'settings-sandbox-backend-level-${DoctorCheckId.sandboxTool}',
        ),
      ),
      findsOneWidget,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a backend written in config.toml is selected as written', (
    WidgetTester tester,
  ) async {
    await openSettings(tester, clientWith(written: 'container'));
    expect(tester.widget<HSegmented<String>>(row).selected, 'container');
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('a daemon that cannot list its backends says so under the row', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = clientWith()
      ..sandboxBackendsFailure = const Diagnostic(
        code: 'IPC_006',
        severity: Severity.error,
        why: 'the daemon does not know SandboxBackends',
      );
    await openSettings(tester, client);
    expect(row, findsNothing);
    expect(
      textOf(tester, 'settings-sandbox-backend-failed'),
      contains('does not know SandboxBackends'),
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
