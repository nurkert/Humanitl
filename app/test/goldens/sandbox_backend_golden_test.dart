// Goldens der Backend-Wahl (HUM-440), in den vier Varianten aus
// `support/golden_variants.dart`: die Zeile `sandbox.backend` der
// Einstellungen und der Abschnitt oben im Reiter Isolation, beide mit einem
// Podman, das hier nicht starten kann, samt Grund und Befehl des Daemons, und
// mit dem VM-Backend der Kernel-Stufe, dem eine Voraussetzung fehlt
// (HUM-447). Dazu die Zeile der Einstellungen mit einem Backend, das der
// Daemon noch prüft (HUM-502).
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/backend_section.dart';
import 'package:humanitl/features/settings/widgets/sandbox_backend_row.dart';

import 'support/golden_variants.dart';

/// Ein Fake mit dem Backend des Systems, einem Podman ohne Programm und dem
/// VM-Backend der Kernel-Stufe ohne Zugriff auf das KVM-Gerät.
FakeDaemonClient backendsClient() =>
    FakeDaemonClient(script: const <ScriptedEvent>[])
      ..sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.namespaces,
        backends: <SandboxBackendInfo>[
          FakeSandboxBackends.defaultSandboxBackends.backends.first,
          const SandboxBackendInfo(
            name: 'podman',
            isolation: IsolationLevel.namespaces,
            diagnostics: <Diagnostic>[
              Diagnostic(
                code: 'SANDBOX_034',
                severity: Severity.blocking,
                why: 'podman was not found on PATH.',
                fix: FixAction.copyCommand(command: 'sudo apt install podman'),
              ),
            ],
          ),
          FakeSandboxBackends.kernelBackendUnavailable,
        ],
      );

/// Wie [backendsClient], nur dass der Daemon das zweite Backend noch prüft
/// (HUM-502): Seine Suche lief bei Ablauf der Frist der App noch. Der Name
/// ist erfunden; die Zeile urteilt über keinen Namen.
FakeDaemonClient checkingClient() =>
    FakeDaemonClient(script: const <ScriptedEvent>[])
      ..sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.namespaces,
        backends: <SandboxBackendInfo>[
          FakeSandboxBackends.defaultSandboxBackends.backends.first,
          const SandboxBackendInfo(
            name: 'engine',
            isolation: IsolationLevel.namespaces,
            diagnostics: <Diagnostic>[
              Diagnostic(
                code: DiagnosticCodes.doctorNotPerformed,
                severity: Severity.warning,
                why:
                    'still being checked: the search for the backend engine '
                    'has run for 3500 ms, longer than this answer waits',
                fix: FixAction.copyCommand(
                  command: 'humanitl sandbox backends',
                ),
              ),
            ],
          ),
          FakeSandboxBackends.kernelBackendUnavailable,
        ],
      );

void main() {
  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'settings_sandbox_backend',
    constraints: const BoxConstraints.tightFor(width: 720, height: 640),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      padding: const EdgeInsets.all(12),
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(backendsClient()),
      ],
      child: const Align(
        alignment: Alignment.topCenter,
        child: SandboxBackendRow(config: ConfigValues.empty),
      ),
    ),
  );

  goldenVariants(
    'sandbox_backend_choice',
    // 540 statt 500: In Deutsch steht der Grund eines Backends mit dem
    // Hinweis „Unübersetzte Meldung:" davor und braucht eine Zeile mehr
    // (HUM-569). Die Sektion scrollt im Panel.
    constraints: const BoxConstraints.tightFor(width: 560, height: 540),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      padding: const EdgeInsets.all(12),
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(backendsClient()),
      ],
      child: const Align(
        alignment: Alignment.topLeft,
        child: SandboxBackendSection(status: SandboxStatus()),
      ),
    ),
  );

  goldenVariants(
    'settings_sandbox_backend_checking',
    constraints: const BoxConstraints.tightFor(width: 720, height: 640),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      padding: const EdgeInsets.all(12),
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(checkingClient()),
      ],
      child: const Align(
        alignment: Alignment.topCenter,
        child: SandboxBackendRow(config: ConfigValues.empty),
      ),
    ),
  );
}
