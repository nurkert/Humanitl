// Die vier Zeilen als reine Funktion (HUM-044).
//
// `setupChecks` ist die eine Stelle, an der elf Zeilen des Daemons und zwei
// Stromantworten zu vier Zeilen werden. Sie faltet und urteilt nicht, und
// genau das steht hier: Eine rote Zeile wird nie gruen, eine ungemessene nie
// eine der beiden, und der Start-Knopf geht nur an, wenn alle vier Zeilen
// gruen sind.

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/shell/providers/setup_state.dart';

/// Eine Zeile des Doctors.
DoctorCheck line(
  String id,
  DoctorStatus status, {
  String code = '',
  String evidence = 'measured',
}) => DoctorCheck(
  id: id,
  status: status,
  evidence: evidence,
  diagnostic: code.isEmpty
      ? null
      : Diagnostic(code: code, severity: Severity.warning, why: 'because'),
);

/// Ein Bericht aus [lines], sonst gruen.
DoctorReport report(List<DoctorCheck> lines) => DoctorReport(checks: lines);

/// Die vier Zeilen ueber einem Bericht, mit erreichbarem Daemon und einem
/// Projektordner, den der Daemon nennt.
SetupState checksOver(DoctorReport doctor) => setupChecks(
  daemon: const DaemonLinkUp(
    DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
  ),
  doctor: AsyncValue<DoctorReport>.data(doctor),
  sandbox: const AsyncValue<SandboxStatus>.data(
    SandboxStatus(workDirHost: '/home/u/proj'),
  ),
  sandboxCall: SandboxCall.plan,
  llm: LlmProbeState.idle,
);

void main() {
  test(
    'a failing line makes the row red, however many green ones stand by',
    () {
      final SetupState state = checksOver(
        report(<DoctorCheck>[
          line('bwrap', DoctorStatus.fail, code: 'DOCTOR_001'),
          line('userns', DoctorStatus.ok),
          line('seccomp', DoctorStatus.ok),
          line('runtime_dir', DoctorStatus.ok),
        ]),
      );
      expect(state[SetupCheckKind.sandbox].state, SetupCheckState.failed);
      expect(state[SetupCheckKind.sandbox].diagnostic?.code, 'DOCTOR_001');
      expect(state.canStart, isFalse);
    },
  );

  test('an unmeasured line keeps the row out of green and the start shut', () {
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        line('bwrap', DoctorStatus.ok),
        line(
          'seccomp',
          DoctorStatus.warn,
          code: DiagnosticCodes.doctorNotPerformed,
          evidence: 'not measured: /proc/self/status could not be read',
        ),
      ]),
    );
    // Nicht gruen: niemand hat nachgesehen.
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.unmeasured);
    // Und deshalb auch kein Start: Die Spezifikation verlangt alle vier gruen,
    // und eine Pruefung, die nicht laufen konnte, ist keine bestandene.
    expect(state.canStart, isFalse);
    expect(state.unmeasuredLines, 1);
  });

  test('a measured line that is merely off keeps the start shut', () {
    // Der Fall, den `blocksStart` frueher durchgelassen hat: gemessen, nicht
    // rot, nicht gruen. Ohne diesen Test faellt eine Rueckkehr zu
    // "nur failed und checking sperren" durch jede andere Pruefung hier.
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        line('bwrap', DoctorStatus.ok),
        line('disk_space', DoctorStatus.warn, code: 'DOCTOR_011'),
      ]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.warn);
    expect(state.canStart, isFalse);
    // Und gemessen heisst gemessen: die Zahl auf der Knopfzeile bleibt bei
    // null, sonst behauptete der Satz darunter eine Luecke, die es nicht gibt.
    expect(state.unmeasuredLines, 0);
  });

  test('a failure outranks an unmeasured line', () {
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        line(
          'seccomp',
          DoctorStatus.warn,
          code: DiagnosticCodes.doctorNotPerformed,
        ),
        line('bwrap', DoctorStatus.fail, code: 'DOCTOR_001'),
      ]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.failed);
    expect(state.canStart, isFalse);
  });

  test('a status this build does not know is never taken for ok', () {
    final SetupState state = checksOver(
      report(<DoctorCheck>[line('twelfth', DoctorStatus.unknown)]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.unmeasured);
    expect(state.canStart, isFalse);
    expect(state.unmeasuredLines, 1);
  });

  test('the daemon line is judged by the client and counted by nobody', () {
    // Der Daemon meldet seine eigene Zeile auf jedem Rechner als nicht
    // versucht -- er kann von innen nicht sagen, ob ein Client ihn erreicht.
    // Wird sie mitgezaehlt, behauptet die Zeile unter dem Knopf auf jedem
    // gesunden Rechner eine Luecke, und der gruene Satz ist unerreichbar.
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        line('bwrap', DoctorStatus.ok),
        line('llm', DoctorStatus.ok),
        line(
          'daemon',
          DoctorStatus.warn,
          code: DiagnosticCodes.doctorNotPerformed,
          evidence: 'not measured: a daemon does not probe itself',
        ),
      ]),
    );
    expect(state.unmeasuredLines, 0);
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.ok);
    expect(state.canStart, isTrue);
  });

  test('the two lines with a row of their own are not counted twice', () {
    // Nur `daemon` und `llm`: Die Maschinenzeile hat dann nichts, worueber sie
    // urteilen koennte, und sagt das, statt gruen zu sein.
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        line('daemon', DoctorStatus.fail, code: 'DOCTOR_006'),
        line('llm', DoctorStatus.fail, code: 'DOCTOR_008'),
      ]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.unmeasured);
    // Und die Daemon-Zeile kommt vom Client, nicht aus dem Bericht.
    expect(state[SetupCheckKind.daemon].state, SetupCheckState.ok);
  });

  test('everything green is everything green', () {
    // Auch die Modell-Zeile: Ohne sie ist die Liste nicht vollstaendig, und
    // `canStart` sagt das -- der Endpunkt wird sonst nur beurteilt, wenn
    // jemand ihn hat ansprechen lassen.
    final SetupState state = checksOver(
      report(<DoctorCheck>[
        for (final String id in <String>['bwrap', 'userns', 'seccomp', 'llm'])
          line(id, DoctorStatus.ok),
      ]),
    );
    expect(state.worst, SetupCheckState.ok);
    expect(state.canStart, isTrue);
    expect(state.unmeasuredLines, 0);
  });

  test('a blocked project folder blocks the start and names the finding', () {
    const Diagnostic refused = Diagnostic(
      code: DiagnosticCodes.workDirRefused,
      severity: Severity.blocking,
      why: '/etc is not a directory of yours',
    );
    final SetupState state = setupChecks(
      daemon: const DaemonLinkUp(
        DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
      ),
      doctor: AsyncValue<DoctorReport>.data(
        report(<DoctorCheck>[line('bwrap', DoctorStatus.ok)]),
      ),
      sandbox: const AsyncValue<SandboxStatus>.data(
        SandboxStatus(workDirHost: '/etc', diagnostics: <Diagnostic>[refused]),
      ),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(state[SetupCheckKind.project].state, SetupCheckState.failed);
    expect(
      state[SetupCheckKind.project].diagnostic?.code,
      DiagnosticCodes.workDirRefused,
    );
    expect(state.canStart, isFalse);
  });

  test('a folder nobody chose blocks the start and is not a gap', () {
    // Kein Ordner ist eine bekannte Tatsache und keine ausgefallene Messung:
    // Der Agent arbeitet in genau einem Ordner, und solange keiner genannt
    // ist, gibt es nichts zu starten (HUM-044, Diagnose-Tabelle).
    final SetupState state = setupChecks(
      daemon: const DaemonLinkUp(
        DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
      ),
      doctor: AsyncValue<DoctorReport>.data(
        report(<DoctorCheck>[line('bwrap', DoctorStatus.ok)]),
      ),
      sandbox: const AsyncValue<SandboxStatus>.data(SandboxStatus()),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(state[SetupCheckKind.project].state, SetupCheckState.failed);
    expect(state.canStart, isFalse);
  });

  test('an error about the folder is not a green folder', () {
    // `SandboxStatus.blocking` antwortet nur auf `Severity.blocking`. Ein
    // Befund, den der Daemon `error` nennt, ist ebenso ein Fehlschlag, und
    // eine Zeile, die darueber gruen bliebe, widerspraeche dem Daemon.
    const Diagnostic failed = Diagnostic(
      code: 'SANDBOX_011',
      severity: Severity.error,
      why: 'the socket placeholder could not be created',
    );
    final SetupState state = setupChecks(
      daemon: const DaemonLinkUp(
        DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
      ),
      doctor: AsyncValue<DoctorReport>.data(
        report(<DoctorCheck>[line('bwrap', DoctorStatus.ok)]),
      ),
      sandbox: const AsyncValue<SandboxStatus>.data(
        SandboxStatus(
          workDirHost: '/home/u/proj',
          diagnostics: <Diagnostic>[failed],
        ),
      ),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(state[SetupCheckKind.project].state, SetupCheckState.failed);
    expect(state[SetupCheckKind.project].diagnostic?.code, 'SANDBOX_011');
    expect(state.canStart, isFalse);
  });

  test('the gravest finding about the folder is the one on the card', () {
    const Diagnostic minor = Diagnostic(
      code: 'SANDBOX_011',
      severity: Severity.error,
      why: 'the socket placeholder could not be created',
    );
    const Diagnostic refused = Diagnostic(
      code: DiagnosticCodes.workDirRefused,
      severity: Severity.blocking,
      why: '/etc is not a directory of yours',
    );
    final SetupState state = setupChecks(
      daemon: const DaemonLinkUp(
        DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
      ),
      doctor: AsyncValue<DoctorReport>.data(
        report(<DoctorCheck>[line('bwrap', DoctorStatus.ok)]),
      ),
      sandbox: const AsyncValue<SandboxStatus>.data(
        SandboxStatus(
          workDirHost: '/etc',
          diagnostics: <Diagnostic>[minor, refused],
        ),
      ),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(
      state[SetupCheckKind.project].diagnostic?.code,
      DiagnosticCodes.workDirRefused,
    );
  });

  /// `docs/UX.md` 7: Jeder abgeleitete Provider gibt einen Typ mit
  /// Wertgleichheit zurueck. Ohne sie ist jede Antwort ein neues, nie gleiches
  /// Objekt, und `SetupHost`, seine vier Zeilen und die neun Zeilen darunter
  /// bauen bei jeder Aenderung irgendeines der vier Provider neu -- auch
  /// waehrend der Abschnitt gar nicht sichtbar ist -- und `ShellScreen`s
  /// `ref.listen` wacht mit ihnen auf.
  test('two folds over the same answers are the same state', () {
    final DoctorReport doctor = report(<DoctorCheck>[
      line('bwrap', DoctorStatus.ok),
      line('userns', DoctorStatus.ok),
    ]);
    expect(checksOver(doctor), checksOver(doctor));
    expect(checksOver(doctor).hashCode, checksOver(doctor).hashCode);

    // Und eine Zeile, die sich wirklich aendert, ist ein anderer Zustand.
    final SetupState other = checksOver(
      report(<DoctorCheck>[
        line('bwrap', DoctorStatus.fail, code: 'DOCTOR_001'),
        line('userns', DoctorStatus.ok),
      ]),
    );
    expect(checksOver(doctor), isNot(other));
  });

  test('a fold that changed nothing does not wake the screen', () async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk();
    final ProviderContainer container = ProviderContainer.test(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client),
        connectionHeartbeatProvider.overrideWithValue(null),
        connectionReconnectProvider.overrideWithValue(null),
      ],
    );
    int wakes = 0;
    container.listen(setupStateProvider, (SetupState? _, SetupState _) {
      wakes++;
    });
    for (int i = 0; i < 12; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    final SetupState settled = container.read(setupStateProvider);

    wakes = 0;
    container.invalidate(setupStateProvider);
    expect(container.read(setupStateProvider), settled);
    for (int i = 0; i < 12; i++) {
      await Future<void>.delayed(Duration.zero);
    }
    expect(wakes, 0);
  });

  test('no answer yet is not an answer', () {
    final SetupState state = setupChecks(
      daemon: const DaemonLinkConnecting(),
      doctor: const AsyncValue<DoctorReport>.loading(),
      sandbox: const AsyncValue<SandboxStatus>.loading(),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(state.worst, SetupCheckState.checking);
    expect(state.canStart, isFalse);
  });
}
