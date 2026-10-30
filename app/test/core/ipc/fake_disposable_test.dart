// Der Fake der Wegwerf-Sandbox spielt den Daemon (HUM-351): Ein leerer Modus
// beim Start heißt `sandbox.mode` aus der Konfiguration, und ein Abbruch
// mitten im Schreiben nimmt die geschriebenen Änderungen nach ihrer Kennung
// aus der Liste, nie nach dem angezeigten Pfad.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

void main() {
  test('an empty start mode follows sandbox.mode like the daemon', () async {
    final FakeDaemonClient client = FakeDaemonClient.disposable()
      ..configValues = const ConfigValues(
        values: <String, Object?>{
          'sandbox': <String, Object?>{'mode': 'disposable'},
        },
      );
    final List<SandboxUpdate> updates = await client.startSandbox().toList();

    expect(
      updates.whereType<SandboxUpdateDiagnostic>().map(
        (SandboxUpdateDiagnostic d) => d.diagnostic.code,
      ),
      contains('SANDBOX_030'),
      reason:
          'the configuration makes it a disposable start while changes wait',
    );

    // Ohne ausdrücklichen Modus gilt, was der Fake als aufgelösten Modus
    // nennt: hier `disposable`, also lehnt auch die Vorgabe ab, solange
    // Änderungen warten (HUM-357).
    final FakeDaemonClient byDefault = FakeDaemonClient.disposable();
    final List<SandboxUpdate> refused = await byDefault.startSandbox().toList();
    expect(
      refused.whereType<SandboxUpdateDiagnostic>().map(
        (SandboxUpdateDiagnostic d) => d.diagnostic.code,
      ),
      contains('SANDBOX_030'),
    );

    final FakeDaemonClient direct = FakeDaemonClient.disposable()
      ..configValues = const ConfigValues(
        values: <String, Object?>{
          'sandbox': <String, Object?>{'mode': 'direct'},
        },
      );
    final List<SandboxUpdate> started = await direct.startSandbox().toList();
    expect(started.whereType<SandboxUpdateDiagnostic>(), isEmpty);
  });

  test('a partial write takes the written changes out by id', () async {
    // Zwei Änderungen, deren gesäuberte Pfade gleich aussehen: Der Daemon
    // zeigt beide als `a?b`, die Kennungen unterscheiden sie.
    final FakeDaemonClient client = FakeDaemonClient.disposable()
      ..pendingChanges = const DisposableChanges(
        sandboxId: 'sb',
        workDir: '/p',
        changes: <DisposableChange>[
          DisposableChange(
            id: '1111111111111111',
            path: 'a?b',
            kind: DisposableChangeKind.added,
            mangled: true,
          ),
          DisposableChange(
            id: '2222222222222222',
            path: 'a?b',
            kind: DisposableChangeKind.added,
            mangled: true,
          ),
        ],
      )
      ..applyBreaksAt = '2222222222222222';

    final List<SandboxUpdate> updates = await client
        .applyChanges(
          sandboxId: 'sb',
          ids: const <String>['1111111111111111', '2222222222222222'],
          overwriteIds: const <String>[],
        )
        .toList();

    expect(
      (updates.last as SandboxUpdateChangesOutcome).outcome.outcome,
      DisposableOutcomeKind.partial,
    );
    expect(
      client.pendingChanges!.changes.map((DisposableChange c) => c.id),
      <String>['2222222222222222'],
    );
  });
}
