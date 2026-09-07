// Der Sandbox-Bildschirm gegen einen echten Daemon (HUM-040).
//
// Nicht Teil des normalen Laufs, aus demselben Grund wie
// `test/features/tray/dbus_live_test.dart`: Der Test startet einen echten
// `humanitld`, und der startet eine echte Sandbox mit `bwrap`. Er läuft nur
// mit `make flutter-test-daemon`, also
// `HUMANITL_DAEMON_TESTS=1 flutter test test/features/sandbox/daemon_live_test.dart`.
//
// **Warum es ihn gibt.** Das Akzeptanzkriterium von HUM-040 lautete „manuell
// mit echtem Daemon: Start zeigt innerhalb 2 s `running`, Mounts-Tab listet
// `/work`, `/run/humanitl/proxy.sock`, `/etc/humanitl/ca.crt`, Env-Tab listet
// `HTTP_PROXY`". Die Widget-Tests daneben messen dasselbe gegen eine
// Attrappe: Sie zeigen, dass der Bildschirm zeichnet, was der Client sagt,
// aber nicht, dass ein echter Daemon dasselbe sagt. Genau diese Fuge ist der
// Ort, an dem eine Oberfläche gegen eine erfundene Antwort grün wird. Hier
// laufen deshalb die echten Provider gegen den echten Dienst; gezeichnet wird
// nichts, das prüfen die Goldens.
//
// **Was er nicht misst.** Pixel. Ob die drei Mounts im Reiter untereinander
// stehen und ob die Zeile mit `HTTP_PROXY` lesbar ist, sieht ein Auge oder ein
// Golden, nicht dieser Test.

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/features/sandbox/providers/sandbox_status_provider.dart';

/// Ob dieser Lauf einen echten Daemon starten darf.
bool get liveAllowed => Platform.environment['HUMANITL_DAEMON_TESTS'] == '1';

/// Die Frist, in der der Bildschirm `running` sehen muss.
///
/// Sie steht im Akzeptanzkriterium von HUM-040 und ist keine Vermutung über
/// diesen Rechner: Zwei Sekunden sind die Zusage an den Menschen, der auf
/// „Starten" drückt.
const Duration startBudget = Duration(seconds: 2);

/// Wie lange auf den Socket des Daemons gewartet wird.
const Duration socketPatience = Duration(seconds: 20);

/// Der Daemon dieses Laufs, mit allem, was er auf der Platte anfasst.
///
/// Alles liegt unter einem eigenen Wurzelverzeichnis: eigener
/// `XDG_RUNTIME_DIR`, eigener Datenpfad, eigene Konfiguration, eigenes
/// Projekt. Der Daemon des Menschen, der diesen Test startet, bleibt
/// unberührt, und am Ende bleibt nichts liegen.
class LiveDaemon {
  LiveDaemon._(this.root, this.process, this.socket);

  /// Startet einen Daemon und wartet, bis sein Socket da ist.
  ///
  /// Gibt `null` zurück, wenn das Binary fehlt: Wer `flutter test` ohne
  /// `cargo build` fährt, soll einen Grund lesen und keinen Stapel.
  static Future<LiveDaemon?> start() async {
    final File binary = File('${_repoRoot()}/daemon/target/debug/humanitld');
    if (!binary.existsSync()) {
      return null;
    }
    final Directory root = Directory.systemTemp.createTempSync('hum-live-');
    try {
      return await _startIn(root, binary);
    } catch (_) {
      // Was zwischen `createTempSync` und dem laufenden Daemon schiefgeht --
      // ein Binary ohne Ausführungsrecht, ein fehlgeschlagener `fork` --,
      // ließe sonst einen Baum in `/tmp` stehen. Der Fehler geht weiter, das
      // Verzeichnis nicht.
      if (root.existsSync()) {
        root.deleteSync(recursive: true);
      }
      rethrow;
    }
  }

  /// Der eigentliche Start, in einem Verzeichnis, das der Aufrufer aufräumt.
  static Future<LiveDaemon> _startIn(Directory root, File binary) async {
    for (final String path in <String>[
      'runtime/humanitl',
      'data',
      'config/humanitl',
      'state',
      'home/project',
    ]) {
      Directory('${root.path}/$path').createSync(recursive: true);
    }
    // Der Daemon besteht auf `0700` für das Verzeichnis von Socket und Token
    // (`DAEMON_004`), und `Directory.create` legt nach der umask an. Dart hat
    // kein `chmod`, also übernimmt es das Programm, das es kann.
    await Process.run('chmod', <String>[
      '700',
      '${root.path}/runtime/humanitl',
    ]);
    final String socket = '${root.path}/runtime/humanitl/daemon.sock';
    final Process process = await Process.start(
      binary.path,
      <String>['--socket', socket],
      environment: <String, String>{
        'XDG_RUNTIME_DIR': '${root.path}/runtime',
        'XDG_DATA_HOME': '${root.path}/data',
        'XDG_CONFIG_HOME': '${root.path}/config',
        'XDG_STATE_HOME': '${root.path}/state',
        'HOME': '${root.path}/home',
        'PATH': Platform.environment['PATH'] ?? '/usr/local/bin:/usr/bin:/bin',
      },
    );
    // `utf8.decode` und nicht `String.fromCharCodes`: Die Befunde des Daemons
    // sind deutsche Sätze, und aus einem Umlaut würde sonst Zeichensalat --
    // ausgerechnet in der Meldung, die den Fehlschlag erklären soll.
    final StringBuffer said = StringBuffer();
    process.stdout.listen(
      (List<int> bytes) => said.write(utf8.decode(bytes, allowMalformed: true)),
    );
    process.stderr.listen(
      (List<int> bytes) => said.write(utf8.decode(bytes, allowMalformed: true)),
    );

    // Ein Daemon, der sofort wieder endet (fehlende Bibliothek, kaputte
    // Konfiguration, Panik), soll das sagen und nicht zwanzig Sekunden lang
    // schweigen: Sein Ende beendet auch das Warten.
    bool exited = false;
    unawaited(process.exitCode.then((int _) => exited = true));
    final DateTime until = DateTime.now().add(socketPatience);
    while (!File(socket).existsSync() &&
        !exited &&
        DateTime.now().isBefore(until)) {
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    if (!File(socket).existsSync()) {
      process.kill(ProcessSignal.sigkill);
      // Das Verzeichnis räumt der Aufrufer weg (`start`), sonst stünde hier
      // dieselbe Zeile zweimal.
      fail('the daemon never opened $socket; it said: $said');
    }
    return LiveDaemon._(root, process, socket);
  }

  /// Das Wurzelverzeichnis dieses Laufs.
  final Directory root;

  /// Der Prozess selbst.
  final Process process;

  /// Der gRPC-Socket, den die Anwendung anspricht.
  final String socket;

  /// Das Projektverzeichnis, das die Sitzung als `/work` bekommt.
  String get project => '${root.path}/home/project';

  /// Beendet den Daemon und räumt sein Verzeichnis weg.
  Future<void> stop() async {
    process.kill(ProcessSignal.sigterm);
    await process.exitCode.timeout(
      const Duration(seconds: 10),
      onTimeout: () {
        process.kill(ProcessSignal.sigkill);
        return -1;
      },
    );
    if (root.existsSync()) {
      root.deleteSync(recursive: true);
    }
  }
}

/// Das Verzeichnis des Repositories, von `app/` aus gesehen.
String _repoRoot() => Directory.current.path.endsWith('/app')
    ? Directory.current.parent.path
    : Directory.current.path;

void main() {
  if (!liveAllowed) {
    test('daemon_live_tests_are_opt_in', () {
      // Kein `skip:`: Ein übersprungener Test steht als solcher im Bericht und
      // erklärt sich selbst, ein grüner ohne Zusicherung nicht.
    }, skip: 'set HUMANITL_DAEMON_TESTS=1 (make flutter-test-daemon)');
    return;
  }

  // Nicht `late`: Scheitert der Aufbau, bevor eines davon steht, liefe der
  // Abbau in einen `LateInitializationError` und ließe den Daemon samt seinem
  // Verzeichnis liegen -- die Meldung wäre dann der Fehler des Abbaus und
  // nicht der Grund, aus dem der Aufbau scheiterte.
  LiveDaemon? daemon;
  ProviderContainer? container;
  SandboxStatus running = const SandboxStatus();
  Duration untilRunning = Duration.zero;

  /// Ob der Provider den Übergang nach `running` wirklich gemeldet hat.
  ///
  /// Ohne diese Frage wäre die Frist unten einseitig: `Duration.zero` ist
  /// kleiner als zwei Sekunden, und ein Test, der eine Null durchgehen lässt,
  /// misst nichts. Gemessen mit genau dieser Mutation am 2026-09-07.
  bool sawRunning = false;

  /// Der laufende Start, damit der Abbau sein Ende abwarten kann.
  Future<void>? starting;

  setUpAll(() async {
    final LiveDaemon? started = await LiveDaemon.start();
    if (started == null) {
      fail('daemon/target/debug/humanitld is missing; run cargo build first');
    }
    daemon = started;
    final ProviderContainer opened = ProviderContainer(
      overrides: <Override>[
        launchOptionsProvider.overrideWithValue(
          LaunchOptions(socketPath: started.socket),
        ),
      ],
    );
    container = opened;
    // Der erste Zug des Bildschirms: fragen, was der Daemon gerade weiß.
    await opened.read(sandboxStatusProvider.future);
    // Dann das, was der Mensch tut: ein Verzeichnis wählen und starten.
    final SandboxStatusNotifier sandbox = opened.read(
      sandboxStatusProvider.notifier,
    );
    await sandbox.plan(workDir: started.project);

    // **Gemessen wird die Zeit bis `running`, nicht die Dauer des Aufrufs.**
    // `start()` läuft, bis der Strom des Daemons endet; das ist die Lebenszeit
    // der Sitzung und nicht das, was ein Mensch vor dem Bildschirm erlebt. Die
    // Zusage von HUM-040 gilt dem Augenblick, in dem der Ring auf `running`
    // springt, und den meldet der Provider.
    final Stopwatch watch = Stopwatch()..start();
    final Completer<Duration> reachedRunning = Completer<Duration>();
    final ProviderSubscription<AsyncValue<SandboxStatus>> watching = opened
        .listen(sandboxStatusProvider, (
          AsyncValue<SandboxStatus>? _,
          AsyncValue<SandboxStatus> next,
        ) {
          if (next.value?.state == SandboxState.running &&
              !reachedRunning.isCompleted) {
            sawRunning = true;
            reachedRunning.complete(watch.elapsed);
          }
        });
    //
    // **Und der Start wird nicht abgewartet.** `Sandbox(Start)` hält seinen
    // Strom offen, bis der Agent endet und die Zusammenfassung geschrieben
    // ist; `await sandbox.start()` bliebe also für die ganze Sitzung stehen.
    // Heute käme der Test damit durch, weil in dieser Sandbox kein
    // `opencode` auf dem PATH liegt und der Start des Agenten sofort
    // fehlschlägt -- auf einem Rechner, auf dem er liegt, hinge der Lauf.
    // Gewartet wird deshalb auf das, was gemessen werden soll, und das Ende
    // des Stroms holt der Abbau.
    starting = sandbox.start();
    try {
      await Future.any(<Future<void>>[reachedRunning.future, starting!])
          .timeout(socketPatience, onTimeout: () {});
      untilRunning = reachedRunning.isCompleted
          ? await reachedRunning.future
          : watch.elapsed;
    } finally {
      watching.close();
      watch.stop();
    }

    // **Ein Fehlschlag des Starts wird hier laut, nicht später leise.**
    // `SandboxStatusNotifier` fängt einen `DaemonException` und legt ihn in
    // den Zustand; `await start()` kehrt dann normal zurück, und ein
    // `?? const SandboxStatus()` machte daraus eine leere Momentaufnahme, in
    // der jede Zusicherung über Befunde wahr ist, weil es keine gibt.
    final AsyncValue<SandboxStatus> status = opened.read(sandboxStatusProvider);
    if (status.hasError) {
      fail('the start failed: ${status.error}\n${status.stackTrace}');
    }
    running = status.requireValue;
  });

  tearDownAll(() async {
    try {
      if (container != null) {
        await container!.read(sandboxStatusProvider.notifier).stop();
      }
      // Erst das `stop`, dann das Ende des Starts: Der Strom des Starts endet
      // mit der Sitzung, und ein Fehler daraus soll nicht verschwinden.
      await starting;
    } catch (_) {
      // Der Daemon endet gleich ohnehin; ein gescheitertes `stop` darf das
      // Aufräumen darunter nicht verhindern.
    } finally {
      try {
        container?.dispose();
      } finally {
        await daemon?.stop();
      }
    }
  });

  test('a_real_start_is_running_within_two_seconds', () {
    expect(
      running.diagnostics.where(
        (Diagnostic finding) => finding.severity == Severity.blocking,
      ),
      isEmpty,
      reason: 'nothing blocked the start: ${running.diagnostics}',
    );
    expect(running.state, SandboxState.running, reason: 'the sandbox is up');
    expect(
      sawRunning,
      isTrue,
      reason:
          'the provider reported the step to `running`, so the time below '
          'is a measurement and not a default',
    );
    expect(
      untilRunning,
      greaterThan(Duration.zero),
      reason: 'starting a sandbox takes time; a zero would be a missing clock',
    );
    expect(
      untilRunning,
      lessThan(startBudget),
      reason: 'the screen sees `running` inside the budget of HUM-040',
    );
  });

  test('a_real_start_mounts_the_three_paths_the_screen_names', () {
    // Erst der Beleg, dass diese Momentaufnahme vom Start kommt und nicht vom
    // `plan` davor: Ohne ihn stünde dieselbe Liste auch dann hier, wenn nie
    // eine Sandbox gelaufen wäre.
    expect(running.state, SandboxState.running);
    expect(running.sandboxId, isNotNull, reason: 'a real sandbox ran');
    final List<String> targets = running.mounts
        .map((MountEntry mount) => mount.dst)
        .toList();
    for (final String path in <String>[
      '/work',
      '/run/humanitl/proxy.sock',
      '/etc/humanitl/ca.crt',
    ]) {
      expect(targets, contains(path), reason: 'the daemon named $path');
    }
    // Und `/work` ist wirklich das gewählte Verzeichnis und nicht irgendeines.
    final MountEntry work = running.mounts.firstWhere(
      (MountEntry mount) => mount.dst == '/work',
    );
    expect(work.src, daemon?.project);
    expect(running.workDirHost, daemon?.project);
  });

  test('a_real_start_carries_the_proxy_in_the_environment', () {
    expect(running.state, SandboxState.running);
    expect(running.sandboxId, isNotNull, reason: 'a real sandbox ran');
    final Iterable<EnvEntry> proxy = running.env.where(
      (EnvEntry entry) => entry.key == 'HTTP_PROXY',
    );
    expect(proxy, isNotEmpty, reason: 'the environment names HTTP_PROXY');
    expect(
      proxy.first.value,
      contains('127.0.0.1'),
      reason: 'and it points at the bridge inside the sandbox',
    );
  });
}
