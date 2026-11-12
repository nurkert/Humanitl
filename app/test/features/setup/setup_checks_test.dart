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
          line(
            DoctorCheckId.sandboxTool,
            DoctorStatus.fail,
            code: 'DOCTOR_001',
          ),
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
        line(DoctorCheckId.sandboxTool, DoctorStatus.ok),
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
        line(DoctorCheckId.sandboxTool, DoctorStatus.ok),
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
        line(DoctorCheckId.sandboxTool, DoctorStatus.fail, code: 'DOCTOR_001'),
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
        line(DoctorCheckId.sandboxTool, DoctorStatus.ok),
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
        report(<DoctorCheck>[line(DoctorCheckId.sandboxTool, DoctorStatus.ok)]),
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
        report(<DoctorCheck>[line(DoctorCheckId.sandboxTool, DoctorStatus.ok)]),
      ),
      sandbox: const AsyncValue<SandboxStatus>.data(SandboxStatus()),
      sandboxCall: SandboxCall.plan,
      llm: LlmProbeState.idle,
    );
    expect(state[SetupCheckKind.project].state, SetupCheckState.failed);
    expect(state.canStart, isFalse);
  });

  test('an error of the plan is not a green start', () {
    // `SandboxStatus.blocking` antwortet nur auf `Severity.blocking`. Ein
    // Befund, den der Daemon `error` nennt, ist ebenso ein Fehlschlag, und
    // eine Zeile, die darueber gruen bliebe, widerspraeche dem Daemon. Ein
    // Platzhalter, der sich nicht anlegen liess, ist kein Fehler des
    // Ordners: Er steht im Schritt der Sandbox (HUM-384).
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
        report(<DoctorCheck>[line(DoctorCheckId.sandboxTool, DoctorStatus.ok)]),
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
    expect(state[SetupCheckKind.project].state, SetupCheckState.ok);
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.failed);
    expect(state[SetupCheckKind.sandbox].diagnostic?.code, 'SANDBOX_011');
    expect(state[SetupCheckKind.sandbox].planFinding, isTrue);
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
        report(<DoctorCheck>[line(DoctorCheckId.sandboxTool, DoctorStatus.ok)]),
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

  /// Der Fall vom Bildschirm (HUM-384): Ein alter Daemon liest ein neueres
  /// gebündeltes Profil und lehnt dessen `sandbox.backend` ab.
  const Diagnostic backendRefused = Diagnostic(
    code: 'CONFIG_003',
    severity: Severity.blocking,
    why:
        'sandbox.backend = auto, the only backend of this build is another one',
  );

  SetupState planWith(
    List<Diagnostic> diagnostics, {
    List<DoctorCheck>? doctor,
    SandboxCall call = SandboxCall.plan,
  }) => setupChecks(
    daemon: const DaemonLinkUp(
      DaemonInfo(daemonVersion: '0.0.0', protoMajor: 1, protoMinor: 4),
    ),
    doctor: AsyncValue<DoctorReport>.data(
      report(
        doctor ??
            <DoctorCheck>[line(DoctorCheckId.sandboxTool, DoctorStatus.ok)],
      ),
    ),
    sandbox: AsyncValue<SandboxStatus>.data(
      SandboxStatus(workDirHost: '/home/u/proj', diagnostics: diagnostics),
    ),
    sandboxCall: call,
    llm: LlmProbeState.idle,
  );

  test('a plan finding that is not about the folder leaves the folder '
      'green and blocks the sandbox step', () {
    final SetupState state = planWith(<Diagnostic>[backendRefused]);
    expect(state[SetupCheckKind.project].state, SetupCheckState.ok);
    expect(state[SetupCheckKind.project].diagnostic, isNull);
    expect(state[SetupCheckKind.project].detail, '/home/u/proj');
    final SetupCheck machine = state[SetupCheckKind.sandbox];
    expect(machine.state, SetupCheckState.failed);
    expect(machine.diagnostic, backendRefused);
    expect(machine.planFinding, isTrue);
    expect(state.canStart, isFalse);
  });

  test('a finding with the folder fix stays on the folder, whatever its '
      'code', () {
    const Diagnostic moved = Diagnostic(
      code: 'SANDBOX_011',
      severity: Severity.blocking,
      why: 'the project folder moved',
      fix: FixAction.changeSetting(key: 'sandbox.work_dir', value: ''),
    );
    final SetupState state = planWith(<Diagnostic>[moved]);
    expect(state[SetupCheckKind.project].state, SetupCheckState.failed);
    expect(state[SetupCheckKind.project].diagnostic, moved);
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.ok);
  });

  test('a failing doctor line keeps the sandbox step before a plan '
      'finding', () {
    final SetupState state = planWith(
      <Diagnostic>[backendRefused],
      doctor: <DoctorCheck>[
        line(DoctorCheckId.sandboxTool, DoctorStatus.fail, code: 'DOCTOR_001'),
      ],
    );
    expect(state[SetupCheckKind.sandbox].diagnostic?.code, 'DOCTOR_001');
    expect(state[SetupCheckKind.sandbox].planFinding, isFalse);
    expect(state[SetupCheckKind.project].state, SetupCheckState.ok);
  });

  test('a refused start is no plan finding of the sandbox step', () {
    // Die Befunde einer Geste stehen unter dem Knopf (`sandboxFailure`),
    // nicht in einem Schritt.
    final SetupState state = planWith(<Diagnostic>[
      backendRefused,
    ], call: SandboxCall.start);
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.ok);
    expect(state.sandboxFailure?.diagnostic, backendRefused);
  });

  /// `docs/UX.md` 7: Jeder abgeleitete Provider gibt einen Typ mit
  /// Wertgleichheit zurueck. Ohne sie ist jede Antwort ein neues, nie gleiches
  /// Objekt, und `SetupHost`, seine vier Zeilen und die neun Zeilen darunter
  /// bauen bei jeder Aenderung irgendeines der vier Provider neu -- auch
  /// waehrend der Abschnitt gar nicht sichtbar ist -- und `ShellScreen`s
  /// `ref.listen` wacht mit ihnen auf.
  test('two folds over the same answers are the same state', () {
    final DoctorReport doctor = report(<DoctorCheck>[
      line(DoctorCheckId.sandboxTool, DoctorStatus.ok),
      line('userns', DoctorStatus.ok),
    ]);
    expect(checksOver(doctor), checksOver(doctor));
    expect(checksOver(doctor).hashCode, checksOver(doctor).hashCode);

    // Und eine Zeile, die sich wirklich aendert, ist ein anderer Zustand.
    final SetupState other = checksOver(
      report(<DoctorCheck>[
        line(DoctorCheckId.sandboxTool, DoctorStatus.fail, code: 'DOCTOR_001'),
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

  // HUM-436: Die Zeilen des gewählten Backends zählen für die Zeile der
  // Maschine mit; die eines anderen Backends kommen höchstens gelb und
  // halten keinen Start auf.
  DoctorReport withBackends(List<DoctorBackendGroup> backends) => DoctorReport(
    checks: <DoctorCheck>[
      for (final String id in <String>['userns', 'seccomp', 'llm'])
        line(id, DoctorStatus.ok),
    ],
    backends: backends,
  );

  test('a failing line of the chosen backend makes the row red', () {
    final SetupState state = checksOver(
      withBackends(<DoctorBackendGroup>[
        DoctorBackendGroup(
          name: 'chosen',
          stage: 'chosen',
          selected: true,
          checks: <DoctorCheck>[
            line(DoctorBackendCheckId.admitted, DoctorStatus.ok),
            line(
              DoctorBackendCheckId.probe,
              DoctorStatus.fail,
              code: 'DOCTOR_014',
            ),
          ],
        ),
      ]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.failed);
    expect(state[SetupCheckKind.sandbox].diagnostic?.code, 'DOCTOR_014');
    expect(state.canStart, isFalse);
  });

  test('a backend that is not chosen does not hold the start', () {
    final SetupState state = checksOver(
      withBackends(<DoctorBackendGroup>[
        DoctorBackendGroup(
          name: 'chosen',
          selected: true,
          checks: <DoctorCheck>[
            line(DoctorBackendCheckId.admitted, DoctorStatus.ok),
            line(DoctorBackendCheckId.available, DoctorStatus.ok),
          ],
        ),
        DoctorBackendGroup(
          name: 'other',
          checks: <DoctorCheck>[
            line(
              DoctorBackendCheckId.admitted,
              DoctorStatus.warn,
              code: 'SANDBOX_048',
            ),
          ],
        ),
      ]),
    );
    expect(state[SetupCheckKind.sandbox].state, SetupCheckState.ok);
    expect(state.canStart, isTrue);
    expect(state.unmeasuredLines, 0);
  });
}
