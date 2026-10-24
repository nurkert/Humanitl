// Die Wahl eines Projektordners über den Provider (HUM-293, Review).
//
// Nur `SetConfig` kann eine Wahl ablehnen: Scheitert danach das neue Lesen
// der Konfiguration, ist der Ordner trotzdem gespeichert, und der Plan läuft.
// Und von zwei Wahlen, die gleichzeitig unterwegs sind, schreibt nur die
// jüngste die Momentaufnahme.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';

/// Der Befund, mit dem der Fake einen Ordner ablehnt.
const Diagnostic refused = Diagnostic(
  code: DiagnosticCodes.workDirRefused,
  severity: Severity.blocking,
  why: 'the project directory from the folder just chosen names /home/u',
);

/// Ein Fake, dessen `SetConfig` für einen Ordner auf ein Zeichen wartet und
/// dessen `GetConfig` auf Wunsch scheitert.
class ChooseClient extends FakeDaemonClient {
  ChooseClient() {
    sandbox = const SandboxStatus();
  }

  /// Ordner, deren `SetConfig` erst nach diesem Zeichen antwortet.
  final Map<String, Completer<void>> held = <String, Completer<void>>{};

  /// Ordner, die abgelehnt werden.
  final Set<String> refusing = <String>{};

  /// Ob `GetConfig` scheitert.
  bool configFails = false;

  /// Wie oft `Sandbox(Plan)` gerufen wurde.
  int plans = 0;

  @override
  Future<void> setConfig(
    String key,
    String value, {
    String? expectedServer,
  }) async {
    if (held[value] case final Completer<void> gate) {
      await gate.future;
    }
    if (key == FormKeys.sandboxWorkDir && refusing.contains(value)) {
      throw const DaemonException(refused);
    }
    await super.setConfig(key, value, expectedServer: expectedServer);
  }

  @override
  Future<ConfigValues> getConfig() async {
    if (configFails) {
      throw const DaemonException(
        Diagnostic(
          code: 'DAEMON_001',
          severity: Severity.error,
          why: 'the daemon went away',
        ),
      );
    }
    return super.getConfig();
  }

  /// Hält den nächsten Plan an, bis dieses Zeichen kommt; er antwortet dann
  /// mit der Momentaufnahme vom Zeitpunkt seines Aufrufs.
  Completer<void>? planGate;

  /// Erfüllt, sobald der angehaltene Plan gerufen wurde.
  final Completer<void> planHeld = Completer<void>();

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    WorkMode? workMode,
  }) async* {
    plans++;
    if (planGate case final Completer<void> gate) {
      planGate = null;
      final SandboxStatus at = sandbox;
      planHeld.complete();
      await gate.future;
      yield SandboxUpdate.status(at);
      return;
    }
    yield* super.planSandbox(profile: profile, workMode: workMode);
  }

  /// Die Ordner, die `SetConfig` angenommen hat, in der Reihenfolge der
  /// Schreibvorgänge.
  List<String> get stored => <String>[
    for (final (String key, String value) in configWrites)
      if (key == FormKeys.sandboxWorkDir) value,
  ];
}

ProviderContainer containerFor(ChooseClient client) {
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
  );
  addTearDown(container.dispose);
  return container;
}

void main() {
  test('a configuration that cannot be read again is no refusal', () async {
    final ChooseClient client = ChooseClient();
    final ProviderContainer container = containerFor(client);
    await container.read(sandboxStatusProvider.future);
    client.configFails = true;

    await container
        .read(sandboxStatusProvider.notifier)
        .chooseWorkDir('/home/u/thesis');

    final SandboxStatus status = container.read(sandboxStatusProvider).value!;
    expect(status.diagnostics, isEmpty);
    expect(status.workDirHost, '/home/u/thesis');
    expect(client.plans, 1, reason: 'the plan runs anyway');
  });

  test('a refusal of an older choice does not cover a newer one', () async {
    final ChooseClient client = ChooseClient()
      ..refusing.add('/home/u')
      ..held['/home/u'] = Completer<void>();
    final ProviderContainer container = containerFor(client);
    await container.read(sandboxStatusProvider.future);
    final SandboxStatusNotifier notifier = container.read(
      sandboxStatusProvider.notifier,
    );

    final Future<void> older = notifier.chooseWorkDir('/home/u');
    final Future<void> newer = notifier.chooseWorkDir('/home/u/thesis');
    client.held['/home/u']!.complete();
    await Future.wait(<Future<void>>[older, newer]);

    final SandboxStatus status = container.read(sandboxStatusProvider).value!;
    expect(status.workDirHost, '/home/u/thesis');
    expect(status.diagnostics, isEmpty);
  });

  test('a refused choice is not stored and stands as the finding', () async {
    final ChooseClient client = ChooseClient()..refusing.add('/home/u');
    final ProviderContainer container = containerFor(client);
    await container.read(sandboxStatusProvider.future);

    await container
        .read(sandboxStatusProvider.notifier)
        .chooseWorkDir('/home/u');

    final SandboxStatus status = container.read(sandboxStatusProvider).value!;
    expect(status.diagnostics, <Diagnostic>[refused]);
    expect(client.configWrites, isEmpty);
    expect(client.plans, 0);
  });

  test(
    'writes go out in pick order, the latest choice is the last write',
    () async {
      // A wartet in `SetConfig`; B, danach gewählt, schreibt erst nach A.
      final ChooseClient client = ChooseClient()
        ..held['/home/u/a'] = Completer<void>();
      final ProviderContainer container = containerFor(client);
      await container.read(sandboxStatusProvider.future);
      final SandboxStatusNotifier notifier = container.read(
        sandboxStatusProvider.notifier,
      );

      final Future<void> a = notifier.chooseWorkDir('/home/u/a');
      final Future<void> b = notifier.chooseWorkDir('/home/u/b');
      await Future<void>.delayed(Duration.zero);
      expect(client.stored, isEmpty, reason: 'B waits for A');
      client.held['/home/u/a']!.complete();
      await Future.wait(<Future<void>>[a, b]);

      expect(client.stored, <String>['/home/u/a', '/home/u/b']);
      expect(
        container.read(sandboxStatusProvider).value!.workDirHost,
        '/home/u/b',
      );
    },
  );

  test('a late event of an older plan does not cover a newer one', () async {
    final ChooseClient client = ChooseClient()..planGate = Completer<void>();
    final Completer<void> gate = client.planGate!;
    final ProviderContainer container = containerFor(client);
    await container.read(sandboxStatusProvider.future);
    final SandboxStatusNotifier notifier = container.read(
      sandboxStatusProvider.notifier,
    );

    final Future<void> a = notifier.chooseWorkDir('/home/u/a');
    await client.planHeld.future;
    await notifier.chooseWorkDir('/home/u/b');
    expect(
      container.read(sandboxStatusProvider).value!.workDirHost,
      '/home/u/b',
    );
    gate.complete();
    await a;

    expect(
      container.read(sandboxStatusProvider).value!.workDirHost,
      '/home/u/b',
      reason: "A's plan answered after B and must not apply",
    );
  });
}
