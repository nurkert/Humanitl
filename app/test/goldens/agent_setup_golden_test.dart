// Goldens der geführten Einrichtung eines Agenten im Startfenster
// (GitHub #111): das ganze Startfenster mit Leiste, bei 1000 und 1400 px
// Breite. Die Inferenz nach `LLM_012` (Schritt braucht dich, zwei
// gespeicherte Server, einer lokal und Standard), Installieren und Anmelden
// eines isolierten Agenten ohne Installation, und Aktuell halten mit zwei
// älteren Versionen. Je in den vier Varianten aus
// `support/golden_variants.dart`.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/features/start/providers/agent_setup.dart';
import 'package:humanitl/features/start/setup/setup_steps.dart';
import 'package:humanitl/features/start/start_screen.dart';

import '../harness/fixed_now.dart';
import 'support/golden_variants.dart';

const String _id = 'p-web';

/// The setup as the notifier would hold it, fixed.
class _FixedSetup extends AgentSetupNotifier {
  _FixedSetup(this._state);

  final AgentSetup _state;

  @override
  AgentSetup? build() => _state;
}

ProposalField _field(String id, String value, List<(String, String)> options) =>
    ProposalField(
      id: id,
      value: value,
      options: <ProposalOption>[
        for (final (String option, String label) in options)
          ProposalOption(value: option, label: label, available: true),
      ],
    );

final StartProposal _proposal = StartProposal(
  path: '/home/dev/web',
  displayName: 'web',
  projectId: _id,
  fields: <ProposalField>[
    _field('agent', 'opencode', <(String, String)>[('opencode', 'OpenCode')]),
    _field('isolation', 'shared_kernel', <(String, String)>[
      ('shared_kernel', 'shared kernel'),
    ]),
    _field('llm_server', 'local', <(String, String)>[
      ('local', 'local'),
      ('office', 'office'),
    ]),
  ],
);

const List<SavedServer> _servers = <SavedServer>[
  SavedServer(
    name: 'local',
    endpoint: 'http://localhost:11434',
    models: <String>['qwen3-coder'],
    active: true,
    local: true,
  ),
  SavedServer(
    name: 'office',
    endpoint: 'https://llm.office.example:8443/v1',
    models: <String>['gpt-oss-120b'],
  ),
];

AgentDepot _opencode({bool installed = true, bool loggedIn = true}) =>
    FakeAgents.fakeAgent(
      'opencode',
      'OpenCode',
      depotSupported: true,
      source: AgentSource.depot,
      version: installed ? '1.2.0' : '',
      versions: installed
          ? const <String>['1.2.0', '1.1.4', '1.0.9']
          : const <String>[],
      loggedIn: loggedIn,
      loginDeclared: true,
      sizeBytes: installed ? 96000000 : 0,
    );

Widget _window(
  GoldenVariant variant, {
  required AgentDepot depot,
  required SetupStepId step,
  List<Diagnostic> findings = const <Diagnostic>[],
}) {
  final FakeDaemonClient client = FakeDaemonClient()
    ..projects = <ProjectEntry>[
      ProjectEntry(
        projectId: _id,
        path: '/home/dev/web',
        displayName: 'web',
        lastAgent: 'opencode',
        runs: 3,
        hasChoice: true,
      ),
    ]
    ..agentsWithLogin = <String>{'opencode'}
    ..agentsStatus = AgentsStatus(agents: <AgentDepot>[depot]);
  final AgentSetup state = AgentSetup(
    target: AgentSetupTarget(
      path: '/home/dev/web',
      projectId: _id,
      displayName: 'web',
      agent: 'opencode',
      findings: findings,
    ),
    step: step,
    proposal: _proposal,
    servers: const SavedServers(servers: _servers, active: 'local'),
    loading: false,
  );
  return goldenFrame(
    variant: variant,
    background: variant.tokens.colors.bg0,
    overrides: <Override>[
      daemonClientProvider.overrideWithValue(client),
      nowProvider.overrideWith(() => FixedNow(DateTime.utc(2026, 11, 27))),
      agentSetupProvider.overrideWith(() => _FixedSetup(state)),
    ],
    child: StartScreen(
      settings: const SizedBox.shrink(),
      cloudConfirm: (
        CloudInference cloud,
        VoidCallback cancel,
        VoidCallback confirm,
      ) => const SizedBox.shrink(),
    ),
  );
}

const Diagnostic _llm012 = Diagnostic(
  code: 'LLM_012',
  severity: Severity.blocking,
  why: 'no model server is set for OpenCode',
);

void main() {
  goldenVariants(
    'start_agent_setup_inference_1000',
    constraints: const BoxConstraints.tightFor(width: 1000, height: 1160),
    builder: (GoldenVariant variant) => _window(
      variant,
      depot: _opencode(),
      step: SetupStepId.inference,
      findings: const <Diagnostic>[_llm012],
    ),
  );

  goldenVariants(
    'start_agent_setup_inference_1400',
    constraints: const BoxConstraints.tightFor(width: 1400, height: 900),
    builder: (GoldenVariant variant) => _window(
      variant,
      depot: _opencode(),
      step: SetupStepId.inference,
      findings: const <Diagnostic>[_llm012],
    ),
  );

  goldenVariants(
    'start_agent_setup_install_1000',
    constraints: const BoxConstraints.tightFor(width: 1000, height: 800),
    builder: (GoldenVariant variant) => _window(
      variant,
      depot: _opencode(installed: false, loggedIn: false),
      step: SetupStepId.install,
    ),
  );

  goldenVariants(
    'start_agent_setup_update_1400',
    constraints: const BoxConstraints.tightFor(width: 1400, height: 800),
    builder: (GoldenVariant variant) =>
        _window(variant, depot: _opencode(), step: SetupStepId.update),
  );
}
