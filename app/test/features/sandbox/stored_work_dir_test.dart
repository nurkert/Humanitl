// Der gespeicherte Projektordner und der Sandbox-Schnappschuss (HUM-295).
//
// Die Shell reicht jeden lesbaren Wert von `sandbox.work_dir` an
// `storedWorkDirRead`; [follow] tut hier dasselbe. Der Schnappschuss plant
// neu, sobald der gespeicherte Ordner nicht der ist, mit dem er geplant
// wurde. Eine Änderung, die warten muss -- auf eine laufende Wahl, auf den
// ersten Schnappschuss, auf das Ende einer laufenden Sandbox --, geht nicht
// verloren, und eine ältere Antwort ersetzt nie eine neuere.

import 'dart:async';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';

/// Ein Fake, dessen Plan und Statusabfrage auf Wunsch anhalten.
class GatedClient extends FakeDaemonClient {
  GatedClient({SandboxState state = SandboxState.stopped}) {
    sandbox = SandboxStatus(state: state, workDirHost: '/home/u/start');
    configValues = const ConfigValues(
      values: <String, Object?>{
        'sandbox': <String, Object?>{'work_dir': '/home/u/start'},
      },
      configPath: '/home/u/.config/humanitl/config.toml',
    );
  }

  /// Wie oft `Sandbox(Plan)` gerufen wurde.
  int plans = 0;

  /// Ob `GetConfig` scheitert.
  bool configFails = false;

  /// Hält den nächsten Plan an; er antwortet dann mit der Momentaufnahme vom
  /// Zeitpunkt seines Aufrufs.
  Completer<void>? planGate;

  /// Hält die nächste Statusabfrage an, ebenso.
  Completer<void>? statusGate;

  /// Erfüllt, sobald ein angehaltener Aufruf da ist.
  Completer<void> held = Completer<void>();

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

  @override
  Stream<SandboxUpdate> planSandbox({
    String? profile,
    WorkMode? workMode,
  }) async* {
    plans++;
    if (planGate case final Completer<void> gate) {
      planGate = null;
      final SandboxStatus at = sandbox;
      held.complete();
      await gate.future;
      yield SandboxUpdate.status(at);
      return;
    }
    yield* super.planSandbox(profile: profile, workMode: workMode);
  }

  @override
  Stream<SandboxUpdate> sandboxStatus() async* {
    if (statusGate case final Completer<void> gate) {
      statusGate = null;
      final SandboxStatus at = sandbox;
      held.complete();
      await gate.future;
      yield SandboxUpdate.status(at);
      return;
    }
    yield* super.sandboxStatus();
  }
}

ProviderContainer containerFor(GatedClient client) {
  final ProviderContainer container = ProviderContainer(
    overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
  );
  addTearDown(container.dispose);
  return container;
}

/// Reicht jeden lesbaren Ordner an den Schnappschuss, wie die Shell.
void follow(ProviderContainer container) {
  container.listen<String?>(
    configProvider.select(
      (AsyncValue<ConfigValues> config) => config.value?.sandboxWorkDir,
    ),
    (String? _, String? workDir) {
      if (workDir != null) {
        container
            .read(sandboxStatusProvider.notifier)
            .storedWorkDirRead(workDir);
      }
    },
    fireImmediately: true,
  );
}

/// Lässt die Mikrotasks der Pläne durchlaufen.
Future<void> settle() async {
  for (int i = 0; i < 5; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

/// Schreibt [dir] wie die Einstellungen.
Future<void> writeInSettings(ProviderContainer container, String dir) =>
    container.read(configProvider.notifier).write(FormKeys.sandboxWorkDir, dir);

String? shown(ProviderContainer container) =>
    container.read(sandboxStatusProvider).value!.workDirHost;

void main() {
  test(
    'the first folder read and the same folder again plan nothing',
    () async {
      final GatedClient client = GatedClient();
      final ProviderContainer container = containerFor(client);
      follow(container);
      await container.read(sandboxStatusProvider.future);
      await container.read(configProvider.future);
      await settle();
      expect(client.plans, 0, reason: 'the first value is no change');

      await container.read(configProvider.notifier).refresh();
      container
          .read(sandboxStatusProvider.notifier)
          .storedWorkDirRead('/home/u/start');
      await settle();
      expect(client.plans, 0, reason: 'the same folder again is no change');
    },
  );

  test('a first folder that arrives late plans nothing', () async {
    final GatedClient client = GatedClient()..configFails = true;
    final ProviderContainer container = containerFor(client);
    follow(container);
    await container.read(sandboxStatusProvider.future);
    await settle();

    client.configFails = false;
    await container.read(configProvider.notifier).refresh();
    await settle();

    expect(
      container.read(configProvider).value!.sandboxWorkDir,
      '/home/u/start',
    );
    expect(client.plans, 0);
  });

  test('a folder from the settings plans once and shows', () async {
    final GatedClient client = GatedClient();
    final ProviderContainer container = containerFor(client);
    follow(container);
    await container.read(sandboxStatusProvider.future);
    await container.read(configProvider.future);

    await writeInSettings(container, '/home/u/new');
    await settle();

    expect(client.plans, 1);
    expect(shown(container), '/home/u/new');
  });

  test('a late refresh does not replace the plan asked for after it', () async {
    final GatedClient client = GatedClient();
    final ProviderContainer container = containerFor(client);
    follow(container);
    await container.read(sandboxStatusProvider.future);
    await container.read(configProvider.future);

    // Die Statusabfrage nimmt den alten Ordner mit und antwortet erst nach
    // dem Plan für den neuen.
    client.statusGate = Completer<void>();
    final Completer<void> gate = client.statusGate!;
    final Future<void> refresh = container
        .read(sandboxStatusProvider.notifier)
        .refresh();
    await client.held.future;
    await writeInSettings(container, '/home/u/new');
    await settle();
    expect(shown(container), '/home/u/new');

    gate.complete();
    await refresh;
    await settle();

    expect(shown(container), '/home/u/new');
  });

  test(
    'a folder from the settings during a choice is planned after it',
    () async {
      final GatedClient client = GatedClient();
      final ProviderContainer container = containerFor(client);
      follow(container);
      await container.read(sandboxStatusProvider.future);
      await container.read(configProvider.future);
      final SandboxStatusNotifier notifier = container.read(
        sandboxStatusProvider.notifier,
      );

      // Der Plan der Wahl A hält an; währenddessen schreiben die
      // Einstellungen B.
      client.planGate = Completer<void>();
      final Completer<void> gate = client.planGate!;
      final Future<void> a = notifier.chooseWorkDir('/home/u/a');
      await client.held.future;
      await writeInSettings(container, '/home/u/b');
      await settle();
      expect(client.plans, 1, reason: 'no plan races the choice');

      gate.complete();
      await a;
      await settle();

      expect(client.plans, 2, reason: 'B is planned once the choice ends');
      expect(shown(container), '/home/u/b');
    },
  );

  test('a choice plans once, not again for the folder it read back', () async {
    final GatedClient client = GatedClient();
    final ProviderContainer container = containerFor(client);
    follow(container);
    await container.read(sandboxStatusProvider.future);
    await container.read(configProvider.future);

    await container
        .read(sandboxStatusProvider.notifier)
        .chooseWorkDir('/home/u/a');
    await settle();

    expect(client.plans, 1);
    expect(shown(container), '/home/u/a');
  });

  test('a change before the first snapshot waits for it', () async {
    final GatedClient client = GatedClient()..statusGate = Completer<void>();
    final Completer<void> gate = client.statusGate!;
    final ProviderContainer container = containerFor(client);
    final SandboxStatusNotifier notifier = container.read(
      sandboxStatusProvider.notifier,
    );
    await client.held.future;

    notifier
      ..storedWorkDirRead('/home/u/start')
      ..storedWorkDirRead('/home/u/new');
    await settle();
    expect(client.plans, 0, reason: 'whether a sandbox runs is unknown');

    gate.complete();
    await container.read(sandboxStatusProvider.future);
    await settle();

    expect(client.plans, 1);
  });

  test('a change while the sandbox runs is planned once it stops', () async {
    final GatedClient client = GatedClient(state: SandboxState.running);
    final ProviderContainer container = containerFor(client);
    follow(container);
    await container.read(sandboxStatusProvider.future);
    await container.read(configProvider.future);

    await writeInSettings(container, '/home/u/new');
    await settle();
    expect(client.plans, 0, reason: 'a running sandbox keeps its folder');
    expect(shown(container), '/home/u/start');

    await container.read(sandboxStatusProvider.notifier).stop();
    await settle();

    expect(client.plans, 1, reason: 'planned on the way to stopped');
  });
}
