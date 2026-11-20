// Die Zeile `sandbox.backend` in den Einstellungen (HUM-440): automatisch oder
// eines der Backends, die `SandboxBackends` zulässt, je mit seiner Stufe,
// geschrieben über `SetConfig`. Ein Backend, das nicht starten kann oder
// unter der Untergrenze liegt, ist nicht wählbar und nennt den Grund des
// Daemons; eines, das der Daemon nicht zulässt (`SANDBOX_048`), steht nicht
// da.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ipc/sandbox_backends_feed.dart';
import 'package:humanitl/core/ui/ui.dart';

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

/// Ob der Fokus auf einem Segment der Zeile steht.
bool focusInRow(WidgetTester tester) {
  final BuildContext? focused = FocusManager.instance.primaryFocus?.context;
  return focused != null &&
      find
          .descendant(of: row, matching: find.byWidget(focused.widget))
          .evaluate()
          .isNotEmpty;
}

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

  // HUM-447: Das VM-Backend der Kernel-Stufe ist seit HUM-449 zugelassen und
  // nur mit `available = true` wählbar. Der Name kommt aus dem Fake (HUM-145).
  const String kernel = FakeSandboxBackends.kernelBackendName;

  testWidgets('the VM backend is offered with the level kernel and written '
      'when the daemon names it available', (WidgetTester tester) async {
    final FakeDaemonClient client = clientWith(container: null);
    await openSettings(tester, client);

    expect(
      tester
          .widget<HSegmented<String>>(row)
          .options
          .map((HSegmentOption<String> o) => o.value),
      <String>[
        'auto',
        for (final SandboxBackendInfo backend
            in FakeSandboxBackends.defaultSandboxBackends.backends)
          backend.name,
      ],
    );
    expect(segment(tester, kernel).enabled, isTrue);
    expect(
      textOf(tester, 'settings-sandbox-backend-level-$kernel'),
      '$kernel: own kernel in a micro VM',
    );
    await tapSegment(tester, control, kernel);
    expect(client.configWrites.last, (FormKeys.sandboxBackend, kernel));
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the VM backend without a prerequisite is not selectable and '
      'names what is missing', (WidgetTester tester) async {
    final FakeDaemonClient client = clientWith(
      container: FakeSandboxBackends.kernelBackendUnavailable,
    );
    await openSettings(tester, client);

    expect(segment(tester, kernel).enabled, isFalse);
    expect(
      textOf(tester, 'settings-sandbox-backend-reason-$kernel'),
      contains('/dev/kvm readable and writable'),
    );
    expect(
      find.byKey(const Key('settings-sandbox-backend-fix-$kernel')),
      findsOneWidget,
    );
    final int before = client.configWrites.length;
    final Finder entry = find.descendant(of: row, matching: find.text(kernel));
    await tester.ensureVisible(entry);
    await tester.pumpAndSettle();
    await tester.tap(entry, warnIfMissed: false);
    await tester.pumpAndSettle();
    expect(client.configWrites, hasLength(before));
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

  // HUM-502: Eine Suche, die der Daemon in der Frist der App nicht beendet
  // hat, kommt als `DOCTOR_012`. Die Zeile sagt „wird noch geprüft“ statt
  // eines Grundes, das Backend ist noch nicht wählbar, und die Liste fragt
  // nach kurzer Zeit von selbst wieder. Mutationsbeweis: ohne das erneute
  // Fragen bleibt die Zeile stehen; ohne [SandboxBackendInfo.checking] steht
  // der allgemeine Satz von `DOCTOR_012` da.
  testWidgets('a backend the daemon is still checking says so and the list '
      'asks again by itself', (WidgetTester tester) async {
    final FakeDaemonClient client = clientWith(
      container: const SandboxBackendInfo(
        name: 'container',
        isolation: IsolationLevel.namespaces,
        diagnostics: <Diagnostic>[
          Diagnostic(
            code: DiagnosticCodes.doctorNotPerformed,
            severity: Severity.warning,
            why:
                'still being checked: the search for the backend container '
                'has run for 3500 ms, longer than this answer waits',
            fix: FixAction.copyCommand(command: 'humanitl sandbox backends'),
          ),
        ],
      ),
    );
    await openSettings(tester, client);

    expect(
      textOf(tester, 'settings-sandbox-backend-checking-container'),
      'Still being checked …',
    );
    expect(
      find.byKey(const Key('settings-sandbox-backend-reason-container')),
      findsNothing,
    );
    expect(segment(tester, 'container').enabled, isFalse);

    final int asked = client.sandboxBackendsCalls;
    client.sandboxBackendList = SandboxBackendList(
      minIsolation: IsolationLevel.policy,
      autoBackend: DoctorCheckId.sandboxTool,
      backends: <SandboxBackendInfo>[
        FakeSandboxBackends.defaultSandboxBackends.backends.first,
        availableContainer,
      ],
    );
    await tester.pump(sandboxBackendsCheckingRetry);
    await tester.pumpAndSettle();

    expect(client.sandboxBackendsCalls, greaterThan(asked));
    expect(
      find.byKey(const Key('settings-sandbox-backend-checking-container')),
      findsNothing,
    );
    expect(segment(tester, 'container').enabled, isTrue);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  // HUM-510: Nach einer Änderung der Konfiguration lädt Riverpod die Liste
  // der Einstellung neu und behält bis zur Antwort die alte. Sie steht
  // gedämpft da, und die Zeile lässt nichts wählen; das erneute Fragen nach
  // einem Backend in Prüfung (HUM-502) dagegen dämpft nichts. Mutationsbeweis:
  // `sandboxBackendsOutdated` als `false` macht den ersten Teil rot, als
  // `isLoading` den zweiten.
  testWidgets(
    'a changed configuration mutes the list and locks the row until '
    'the new answer, asking again for a checked backend does not',
    (WidgetTester tester) async {
      final _HeldClient client = _HeldClient()
        ..sandboxBackendList = SandboxBackendList(
          minIsolation: IsolationLevel.policy,
          autoBackend: DoctorCheckId.sandboxTool,
          backends: <SandboxBackendInfo>[
            FakeSandboxBackends.defaultSandboxBackends.backends.first,
            const SandboxBackendInfo(
              name: 'container',
              isolation: IsolationLevel.namespaces,
              diagnostics: <Diagnostic>[
                Diagnostic(
                  code: DiagnosticCodes.doctorNotPerformed,
                  severity: Severity.warning,
                  why: 'still being checked',
                ),
              ],
            ),
          ],
        );
      await openSettings(tester, client);
      final HTokens tokens = HTheme.of(tester.element(row));
      const String level = 'settings-sandbox-backend-level-container';
      Color? colorOf(String key) =>
          tester.widget<Text>(find.byKey(Key(key))).style?.color;
      expect(colorOf(level), tokens.colors.fg1);

      // Erneutes Fragen, derselbe Plan: nichts gedämpft, nichts gesperrt.
      Completer<SandboxBackendList> held = Completer<SandboxBackendList>();
      client.held = held;
      final int asked = client.sandboxBackendsCalls;
      await tester.pump(sandboxBackendsCheckingRetry);
      await tester.pump();
      await tester.pump();
      expect(client.sandboxBackendsCalls, greaterThan(asked));
      expect(held.isCompleted, isFalse);
      expect(colorOf(level), tokens.colors.fg1);
      expect(tester.widget<HSegmented<String>>(row).enabled, isTrue);
      client.sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.policy,
        autoBackend: DoctorCheckId.sandboxTool,
        backends: <SandboxBackendInfo>[
          FakeSandboxBackends.defaultSandboxBackends.backends.first,
          availableContainer,
        ],
      );
      held.complete(client.sandboxBackendList);
      await tester.pumpAndSettle();
      expect(
        find.byKey(const Key('settings-sandbox-backend-checking-container')),
        findsNothing,
      );

      // Ein anderes Projekt: dieselbe Liste, ein anderer Plan.
      held = Completer<SandboxBackendList>();
      client.held = held;
      unawaited(
        ProviderScope.containerOf(tester.element(row))
            .read(configProvider.notifier)
            .write(FormKeys.sandboxWorkDir, '/home/dev/other-project'),
      );
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      expect(held.isCompleted, isFalse);
      expect(colorOf(level), tokens.colors.fg2);
      expect(tester.widget<HSegmented<String>>(row).enabled, isFalse);

      held.complete(client.sandboxBackendList);
      await tester.pumpAndSettle();
      expect(colorOf(level), tokens.colors.fg1);
      expect(tester.widget<HSegmented<String>>(row).enabled, isTrue);
      // Wer nicht in der Zeile war, bekommt dort keinen Fokus.
      expect(focusInRow(tester), isFalse);
    },
    variant: TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );

  // HUM-510, Review: Scheitert das Neuladen nach einem Wechsel des Projekts,
  // hält Riverpod die alte Liste neben dem Fehler. Die Zeile nennt den
  // Fehler, statt die Liste des vorigen Projekts als aktuell zu zeigen.
  // Mutationsbeweis: `answer.value` ohne `hasError` macht diesen Test rot.
  testWidgets(
    'a failed reload after a project switch shows the failure, not '
    'the previous list',
    (WidgetTester tester) async {
      final _HeldClient client = _HeldClient()
        ..sandboxBackendList = clientWith().sandboxBackendList;
      await openSettings(tester, client);
      expect(row, findsOneWidget);

      client.sandboxBackendsFailure = const Diagnostic(
        code: 'IPC_006',
        severity: Severity.error,
        why: 'the daemon does not know SandboxBackends',
      );
      unawaited(
        ProviderScope.containerOf(tester.element(row))
            .read(configProvider.notifier)
            .write(FormKeys.sandboxWorkDir, '/home/dev/other-project'),
      );
      await tester.pumpAndSettle();

      expect(row, findsNothing);
      expect(
        find.byKey(const Key('settings-sandbox-backend-level-container')),
        findsNothing,
      );
      expect(
        textOf(tester, 'settings-sandbox-backend-failed'),
        contains('does not know SandboxBackends'),
      );
    },
    variant: TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );

  // HUM-510, Review: Wer ein Backend mit der Tastatur wählt, schreibt
  // `sandbox.backend`; die Liste lädt danach neu und sperrt die Segmente
  // kurz. Der Fokus bleibt danach in der Zeile, damit die Tastatur dort
  // weitermachen kann.
  testWidgets(
    'choosing a backend by keyboard keeps the focus in the row '
    'across the reload',
    (WidgetTester tester) async {
      final _HeldClient client = _HeldClient()
        ..sandboxBackendList = clientWith().sandboxBackendList;
      await openSettings(tester, client);
      final Finder target = find.descendant(
        of: row,
        matching: find.text('container'),
      );
      await tester.ensureVisible(target);
      await tester.pumpAndSettle();
      Focus.of(tester.element(target)).requestFocus();
      await tester.pump();
      expect(focusInRow(tester), isTrue);

      final Completer<SandboxBackendList> held =
          Completer<SandboxBackendList>();
      client.held = held;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      expect(held.isCompleted, isFalse);
      expect(tester.widget<HSegmented<String>>(row).selected, 'container');
      expect(tester.widget<HSegmented<String>>(row).enabled, isFalse);

      held.complete(client.sandboxBackendList);
      await tester.pumpAndSettle();
      expect(tester.widget<HSegmented<String>>(row).enabled, isTrue);
      expect(focusInRow(tester), isTrue);
    },
    variant: TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );

  // HUM-560, Review: Wer während der Sperre ein anderes Feld fokussiert, hat
  // den Fokus bewusst bewegt; nach dem Neuladen bleibt er dort. Mutations-
  // beweis: eine Rückgabe ohne den Vergleich mit dem Knoten, auf den die
  // Sperre den Fokus schob, macht diesen Test rot.
  testWidgets(
    'focus moved to another field while the row is locked stays there',
    (WidgetTester tester) async {
      final _HeldClient client = _HeldClient()
        ..sandboxBackendList = clientWith().sandboxBackendList;
      await openSettings(tester, client);
      final Finder target = find.descendant(
        of: row,
        matching: find.text('container'),
      );
      await tester.ensureVisible(target);
      await tester.pumpAndSettle();
      Focus.of(tester.element(target)).requestFocus();
      await tester.pump();
      expect(focusInRow(tester), isTrue);

      final Completer<SandboxBackendList> held =
          Completer<SandboxBackendList>();
      client.held = held;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      expect(tester.widget<HSegmented<String>>(row).enabled, isFalse);
      expect(focusInRow(tester), isFalse);

      final FocusNode field = tester
          .widget<EditableText>(find.byType(EditableText).first)
          .focusNode;
      field.requestFocus();
      await tester.pump();
      expect(FocusManager.instance.primaryFocus, same(field));

      held.complete(client.sandboxBackendList);
      await tester.pumpAndSettle();
      expect(tester.widget<HSegmented<String>>(row).enabled, isTrue);
      expect(FocusManager.instance.primaryFocus, same(field));
      expect(focusInRow(tester), isFalse);
    },
    variant: TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );

  // HUM-560, Review: Fällt der Fokus nach dem Feld dorthin zurück, wohin
  // die Sperre ihn schob, bleibt die Rückgabe trotzdem aufgehoben.
  // Mutationsbeweis: nur beim Entsperren zu vergleichen macht diesen Test
  // rot.
  testWidgets(
    'focus moved away and back while the row is locked is not pulled in',
    (WidgetTester tester) async {
      final _HeldClient client = _HeldClient()
        ..sandboxBackendList = clientWith().sandboxBackendList;
      await openSettings(tester, client);
      final Finder target = find.descendant(
        of: row,
        matching: find.text('container'),
      );
      await tester.ensureVisible(target);
      await tester.pumpAndSettle();
      Focus.of(tester.element(target)).requestFocus();
      await tester.pump();
      expect(focusInRow(tester), isTrue);

      final Completer<SandboxBackendList> held =
          Completer<SandboxBackendList>();
      client.held = held;
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));
      expect(tester.widget<HSegmented<String>>(row).enabled, isFalse);
      final FocusNode? fallback = FocusManager.instance.primaryFocus;

      final FocusNode field = tester
          .widget<EditableText>(find.byType(EditableText).first)
          .focusNode;
      field.requestFocus();
      await tester.pump();
      expect(FocusManager.instance.primaryFocus, same(field));
      // Zurück auf den Knoten, auf den die Sperre den Fokus schob.
      expect(fallback, isNotNull);
      fallback!.requestFocus();
      await tester.pump();
      expect(FocusManager.instance.primaryFocus, same(fallback));

      held.complete(client.sandboxBackendList);
      await tester.pumpAndSettle();
      expect(tester.widget<HSegmented<String>>(row).enabled, isTrue);
      expect(FocusManager.instance.primaryFocus, same(fallback));
      expect(focusInRow(tester), isFalse);
    },
    variant: TargetPlatformVariant(<TargetPlatform>{
      TargetPlatform.linux,
      TargetPlatform.macOS,
    }),
  );
}

/// Ein Fake, der die nächste Liste erst nennt, wenn der Test [held] erfüllt.
class _HeldClient extends FakeDaemonClient {
  _HeldClient() : super(script: const <ScriptedEvent>[]);

  /// Die zurückgehaltene Antwort, oder null für sofortige Antworten.
  Completer<SandboxBackendList>? held;

  @override
  Future<SandboxBackendList> sandboxBackends({String? backend}) {
    final Completer<SandboxBackendList>? hold = held;
    if (hold == null || hold.isCompleted) {
      return super.sandboxBackends(backend: backend);
    }
    sandboxBackendsCalls++;
    return hold.future;
  }
}
