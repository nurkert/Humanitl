// Goldens der Backend-Wahl (HUM-440), in den vier Varianten aus
// `support/golden_variants.dart`: die Zeile `sandbox.backend` der
// Einstellungen und der Abschnitt oben im Reiter Isolation, beide mit einem
// Podman, das hier nicht starten kann, samt Grund und Befehl des Daemons.
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

/// Ein Fake mit dem Backend des Systems und einem Podman ohne Programm.
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
        ],
      );

void main() {
  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'settings_sandbox_backend',
    constraints: const BoxConstraints.tightFor(width: 720, height: 240),
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
    constraints: const BoxConstraints.tightFor(width: 560, height: 380),
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
}
