// Die geführte Einrichtung eines Agenten im Startfenster (GitHub #111): sie
// öffnet am ersten offenen Schritt, jeder Schritt ruft dieselbe RPC wie die
// Einstellungen, und jeder Fehlerfall steht am Schritt statt als Text unter
// der Liste.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/time/now.dart';
import 'package:humanitl/core/ui/agents/login_url_opener.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl/features/start/providers/agent_setup.dart';
import 'package:humanitl/features/start/start_screen.dart';

import '../../harness/app_harness.dart';
import '../../harness/fixed_now.dart';

final DateTime _now = DateTime.utc(2026, 11, 27, 12);

const String _id = 'p-web';

ProjectEntry _project({String agent = 'opencode'}) => ProjectEntry(
  projectId: _id,
  path: '/home/dev/web',
  displayName: 'web',
  lastStarted: _now.subtract(const Duration(hours: 2)),
  lastAgent: agent,
  runs: 3,
  hasChoice: true,
);

/// OpenCode in the depot, as [installed] and [loggedIn] say.
AgentDepot _opencode({
  bool installed = false,
  bool loggedIn = false,
  String source = AgentSource.depot,
  List<String> versions = const <String>[],
  AgentMaintenance? run,
}) => FakeAgents.fakeAgent(
  'opencode',
  'OpenCode',
  depotSupported: true,
  source: source,
  version: installed ? '1.2.0' : '',
  versions: versions,
  loggedIn: loggedIn,
  loginDeclared: true,
  maintenance: run,
);

FakeDaemonClient _client(AgentDepot opencode) {
  final FakeDaemonClient client = FakeDaemonClient()
    ..projects = <ProjectEntry>[_project()]
    ..agentsWithLogin = <String>{'opencode'}
    ..agentsStatus = AgentsStatus(
      agents: <AgentDepot>[
        FakeAgents.fakeAgent('claude', 'Claude Code', depotSupported: true),
        opencode,
      ],
    )
    ..savedServers = const <SavedServer>[
      SavedServer(
        name: 'local',
        endpoint: 'http://localhost:11434',
        models: <String>['qwen3'],
        local: true,
      ),
    ]
    ..activeServer = 'local';
  return client;
}

Future<void> _pump(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(
    tester,
    client: client,
    startWindow: true,
    size: const Size(1400, 1000),
    overrides: [
      nowProvider.overrideWith(() => FixedNow(_now)),
      loginUrlOpenerProvider.overrideWithValue((String url) async => true),
    ],
  );
  await tester.pump();
  await tester.pump();
}

/// Lets the setup's calls to the daemon answer.
Future<void> _settle(WidgetTester tester) async {
  for (int i = 0; i < 6; i++) {
    await tester.pump();
  }
}

Future<void> _tap(WidgetTester tester, String key) async {
  final Finder target = find.byKey(Key(key));
  await tester.ensureVisible(target);
  await tester.tap(target);
  await _settle(tester);
}

String _state(WidgetTester tester, SetupStepName step) => tester
    .widget<Text>(find.byKey(Key('agent-setup-step-state-${step.name}')))
    .data!;

/// The step ids as the keys name them.
enum SetupStepName { source, install, inference, update, ready }

void main() {
  testWidgets('a depot agent without install opens the setup at install '
      'instead of starting; install and sign-in call Agents', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(_opencode());
    await _pump(tester, client);

    expect(
      find.byKey(const Key('start-agent-state-$_id')),
      findsOneWidget,
      reason: 'the row says the agent needs setup',
    );
    await _tap(tester, 'start-start-$_id');

    expect(client.startProjectIds, isEmpty, reason: 'nothing started');
    expect(find.byKey(const Key('agent-setup')), findsOneWidget);
    expect(find.byKey(const Key('agent-setup-panel-install')), findsOneWidget);
    expect(_state(tester, SetupStepName.install), 'Needs setup');
    expect(_state(tester, SetupStepName.update), 'Waits for an earlier step');

    await _tap(tester, 'agent-setup-install');
    expect(client.agentsCalls.last, const AgentsOp.install('opencode'));
    expect(_state(tester, SetupStepName.install), 'Running');
    expect(find.byKey(const Key('agent-setup-run-survives')), findsOneWidget);

    client.finishAgentRun('opencode', version: '1.2.0');
    await tester.pump(const Duration(milliseconds: 1600));
    await _settle(tester);
    expect(find.byKey(const Key('agent-setup-login')), findsOneWidget);

    await _tap(tester, 'agent-setup-login');
    expect(client.agentsCalls.last, const AgentsOp.login('opencode'));
    client.finishAgentRun('opencode');
    await tester.pump(const Duration(milliseconds: 1600));
    await _settle(tester);
    expect(_state(tester, SetupStepName.install), 'Done');
  });

  testWidgets('a failed install stays at the step with the finding; the '
      'step keeps needing action', (WidgetTester tester) async {
    final FakeDaemonClient client = _client(_opencode());
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');
    await _tap(tester, 'agent-setup-install');
    client.finishAgentRun(
      'opencode',
      failure: const Diagnostic(
        code: 'AGENT_011',
        severity: Severity.error,
        why: 'the checksum did not match',
      ),
    );
    await tester.pump(const Duration(milliseconds: 1600));
    await _settle(tester);

    expect(find.byKey(const Key('agent-run-opencode')), findsOneWidget);
    expect(find.text('the checksum did not match'), findsWidgets);
    expect(_state(tester, SetupStepName.install), 'Needs setup');
  });

  testWidgets('a refusal of the daemon stands at the step', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(_opencode())
      ..agentsRefusals[AgentsOpKind.install] = const Diagnostic(
        code: 'AGENT_011',
        severity: Severity.error,
        why: 'no session can start now',
      );
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');
    await _tap(tester, 'agent-setup-install');

    expect(find.byKey(const Key('agent-setup-refusal')), findsOneWidget);
    expect(_state(tester, SetupStepName.install), 'Needs setup');
  });

  testWidgets('a running install shows again when the setup opens later', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(
      _opencode(
        run: const AgentMaintenance(
          sandboxId: 'maint-opencode-install',
          task: AgentRunTask.install,
          state: AgentRunState.running,
        ),
      ),
    );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');

    expect(find.byKey(const Key('agent-setup-panel-install')), findsOneWidget);
    expect(_state(tester, SetupStepName.install), 'Running');
    expect(find.byKey(const Key('agent-run-state-opencode')), findsOneWidget);
  });

  testWidgets('LLM_012 at the start leads into the inference step; the '
      'server is tested before it is taken over for this project', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..sandboxStartFailure = const Diagnostic(
            code: 'LLM_012',
            severity: Severity.blocking,
            why: 'no model server is set',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');

    expect(client.startProjectIds, <String?>[_id]);
    expect(
      find.byKey(const Key('agent-setup-panel-inference')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('start-diagnostic-LLM_012')), findsNothing);
    expect(_state(tester, SetupStepName.inference), 'Needs setup');

    // Ohne Test nimmt der Schritt keinen Server.
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-server-use')))
          .enabled,
      isFalse,
    );
    await _tap(tester, 'agent-setup-server-test');
    expect(client.probedServers.last, 'local');
    expect(find.byKey(const Key('agent-setup-probe-ok')), findsOneWidget);

    await _tap(tester, 'agent-setup-server-use');
    final (String?, String?, List<ChoiceValue>) saved =
        client.savedChoices.last;
    expect(saved.$1, _id);
    expect(saved.$3, contains(const ChoiceValue('llm.server', 'local')));
    expect(_state(tester, SetupStepName.inference), 'Done');
  });

  testWidgets('a server for all projects becomes the default server', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..sandboxStartFailure = const Diagnostic(
            code: 'LLM_012',
            severity: Severity.blocking,
            why: 'no model server is set',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');
    await tester.tap(find.text('Mine (all projects)'));
    await _settle(tester);
    await _tap(tester, 'agent-setup-server-test');
    await _tap(tester, 'agent-setup-server-use');

    expect(client.serverRequests.last, 'activate local');
    expect(client.savedChoices, isEmpty);
  });

  testWidgets('a failed test keeps the server from being taken over', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..llmProbeFailure = const Diagnostic(
            code: 'LLM_001',
            severity: Severity.error,
            why: 'nothing answers on localhost:11434',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-inference');
    await _tap(tester, 'agent-setup-server-test');

    expect(find.byKey(const Key('agent-setup-refusal')), findsOneWidget);
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-server-use')))
          .enabled,
      isFalse,
    );
  });

  testWidgets('the own providers are never preselected and need their own '
      'confirmation; then inference = own and the notice is confirmed', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..proposalCloud = const CloudInference(
            agent: 'opencode',
            provider: 'opencode.ai',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-inference');

    expect(find.byKey(const Key('agent-setup-cloud-notice')), findsNothing);
    expect(find.byKey(const Key('agent-setup-server-test')), findsOneWidget);

    await _tap(tester, 'agent-setup-inference-own');
    expect(find.byKey(const Key('agent-setup-cloud-notice')), findsOneWidget);
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-cloud-use')))
          .enabled,
      isFalse,
      reason: 'the confirmation is not ticked in advance',
    );
    await _tap(tester, 'agent-setup-cloud-confirm');
    await _tap(tester, 'agent-setup-cloud-use');

    expect((await client.getConfig()).at('agents.opencode.inference'), 'own');
    expect(client.cloudAcknowledgements, <String>['opencode opencode.ai']);
    expect(
      find.byKey(const Key('agent-setup-inference-own-current')),
      findsOneWidget,
    );
  });

  testWidgets('under LLM_014 the own providers stay visible and locked', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..sandboxStartFailure = const Diagnostic(
            code: 'LLM_014',
            severity: Severity.blocking,
            why: 'llm.cloud is block',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');

    expect(
      find.byKey(const Key('agent-setup-inference-own-locked')),
      findsOneWidget,
    );
    await _tap(tester, 'agent-setup-inference-own');
    expect(find.byKey(const Key('agent-setup-cloud-notice')), findsNothing);
  });

  testWidgets('the source step turns the depot on with Agents set_source', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(
      _opencode(source: AgentSource.host),
    );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-source');
    await _tap(tester, 'agent-setup-source-depot');
    expect(find.byKey(const Key('agent-setup-source-change')), findsOneWidget);
    await _tap(tester, 'agent-setup-source-apply');

    expect(
      client.agentsCalls.last,
      const AgentsOp.setSource('opencode', AgentSource.depot),
    );
    expect(find.byKey(const Key('agent-setup-depot-empty')), findsOneWidget);
    expect(_state(tester, SetupStepName.install), 'Needs setup');
  });

  testWidgets('a company lock keeps the source and says so', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(_opencode())
      ..agentsStatus = AgentsStatus(
        agents: <AgentDepot>[
          FakeAgents.fakeAgent(
            'opencode',
            'OpenCode',
            depotSupported: true,
            source: AgentSource.depot,
            lockedBy: '/etc/humanitl/policy.toml',
          ),
        ],
      );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-source');

    expect(find.byKey(const Key('agent-setup-source-locked')), findsOneWidget);
    await _tap(tester, 'agent-setup-source-host');
    expect(find.byKey(const Key('agent-setup-source-change')), findsNothing);
  });

  testWidgets('from the agent row: the update step shows the version, updates '
      'in a run, and a failed update leaves the old version', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(
      _opencode(
        installed: true,
        loggedIn: true,
        versions: const <String>['1.2.0', '1.1.0'],
      ),
    );
    await _pump(tester, client);
    expect(find.text('Isolated 1.2.0'), findsOneWidget);
    await _tap(tester, 'start-agent-setup-$_id');

    expect(find.byKey(const Key('agent-setup-panel-update')), findsOneWidget);
    expect(
      tester
          .widget<Text>(find.byKey(const Key('agent-setup-update-version')))
          .data,
      '1.2.0',
    );
    await _tap(tester, 'agent-setup-update');
    expect(client.agentsCalls.last, const AgentsOp.update('opencode'));
    expect(_state(tester, SetupStepName.update), 'Running');

    client.finishAgentRun(
      'opencode',
      failure: const Diagnostic(
        code: 'AGENT_011',
        severity: Severity.error,
        why: 'the signature did not verify',
      ),
    );
    await tester.pump(const Duration(milliseconds: 1600));
    await _settle(tester);
    expect(_state(tester, SetupStepName.update), 'Needs setup');
    expect(
      tester
          .widget<Text>(find.byKey(const Key('agent-setup-update-version')))
          .data,
      '1.2.0',
      reason: 'a failed update leaves the installed version',
    );
    expect(client.agentsStatus.of('opencode')!.loggedIn, isTrue);

    await _tap(tester, 'agent-setup-update-back-1.1.0');
    expect(
      client.agentsCalls.last,
      const AgentsOp.update('opencode', version: '1.1.0'),
    );
  });

  testWidgets('a ready project starts as today, and the summary starts it '
      'from the setup', (WidgetTester tester) async {
    final FakeDaemonClient client = _client(
      _opencode(installed: true, loggedIn: true),
    );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-ready');

    expect(_state(tester, SetupStepName.ready), 'Done');
    expect(
      tester
          .widget<Text>(find.byKey(const Key('agent-setup-ready-version')))
          .data,
      '1.2.0',
    );
    await _tap(tester, 'agent-setup-start');
    await tester.pump(const Duration(milliseconds: 50));
    expect(client.startProjectIds, <String?>[_id]);
    expect(find.byType(ShellScreen), findsOneWidget);
    expect(find.byType(StartScreen), findsNothing);
  });

  testWidgets('every step names its command line', (WidgetTester tester) async {
    final FakeDaemonClient client = _client(_opencode());
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    for (final SetupStepName step in SetupStepName.values) {
      await _tap(tester, 'agent-setup-step-${step.name}');
      expect(
        find.descendant(
          of: find.byKey(const Key('agent-setup-commands')),
          matching: find.textContaining('humanitl '),
        ),
        findsWidgets,
        reason: step.name,
      );
    }
    expect(find.text('humanitl agents install opencode'), findsNothing);
    await _tap(tester, 'agent-setup-step-install');
    expect(find.text('humanitl agents install opencode'), findsOneWidget);
    expect(find.text('humanitl agents login opencode'), findsOneWidget);
  });

  testWidgets('an installed depot agent without a login starts as before; '
      'the sign-in is offered, quiet, and the summary is open', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(_opencode(installed: true));
    await _pump(tester, client);
    expect(find.byKey(const Key('start-agent-state-$_id')), findsOneWidget);
    expect(find.text('Isolated 1.2.0'), findsOneWidget);

    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-install');
    expect(_state(tester, SetupStepName.install), 'Done');
    final HButton login = tester.widget<HButton>(
      find.byKey(const Key('agent-setup-login')),
    );
    expect(login.variant, HButtonVariant.secondary);
    await _tap(tester, 'agent-setup-step-ready');
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-start')))
          .enabled,
      isTrue,
    );
    await _tap(tester, 'agent-setup-close');

    await _tap(tester, 'start-start-$_id');
    await tester.pump(const Duration(milliseconds: 50));
    expect(client.startProjectIds, <String?>[_id]);
    expect(find.byType(ShellScreen), findsOneWidget);
  });

  testWidgets('a refusal no step solves stands above the steps, beside the '
      'one that leads into the source step', (WidgetTester tester) async {
    final FakeDaemonClient client = _client(
      _opencode(installed: true, loggedIn: true),
    );
    await _pump(tester, client);
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(StartScreen)),
    );
    unawaited(
      container
          .read(agentSetupProvider.notifier)
          .open(
            AgentSetupTarget.of(
              _project(),
              findings: const <Diagnostic>[
                Diagnostic(
                  code: 'AGENT_001',
                  severity: Severity.blocking,
                  why: 'opencode is not on PATH',
                ),
                Diagnostic(
                  code: 'SANDBOX_045',
                  severity: Severity.blocking,
                  why: 'the kernel level needs /dev/kvm',
                ),
              ],
            ),
          ),
    );
    await _settle(tester);

    expect(find.byKey(const Key('agent-setup-panel-source')), findsOneWidget);
    expect(
      find.byKey(const Key('start-diagnostic-SANDBOX_045')),
      findsOneWidget,
    );
    expect(find.byKey(const Key('start-diagnostic-AGENT_001')), findsNothing);
  });

  testWidgets('a start with only a warning of the setup stays in the list', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..sandboxStartFailure = const Diagnostic(
            code: 'AGENT_009',
            severity: Severity.warning,
            why: 'no login stored',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-start-$_id');

    expect(find.byKey(const Key('agent-setup')), findsNothing);
    expect(find.byKey(const Key('start-diagnostic-AGENT_009')), findsOneWidget);
  });

  testWidgets('a failed confirmation sets inference back and clears the '
      'checkbox', (WidgetTester tester) async {
    final FakeDaemonClient client =
        _client(_opencode(installed: true, loggedIn: true))
          ..proposalCloud = const CloudInference(
            agent: 'opencode',
            provider: 'opencode.ai',
          )
          ..cloudAcknowledgeFailure = const Diagnostic(
            code: 'CONFIG_003',
            severity: Severity.error,
            why: 'the confirmation could not be kept',
          );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    await _tap(tester, 'agent-setup-step-inference');
    await _tap(tester, 'agent-setup-inference-own');
    await _tap(tester, 'agent-setup-cloud-confirm');
    await _tap(tester, 'agent-setup-cloud-use');

    expect(
      (await client.getConfig()).at('agents.opencode.inference'),
      'model-server',
    );
    expect(find.byKey(const Key('agent-setup-refusal')), findsOneWidget);
    expect(
      tester
          .widget<HCheckbox>(find.byKey(const Key('agent-setup-cloud-confirm')))
          .value,
      isFalse,
    );
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-cloud-use')))
          .enabled,
      isFalse,
    );
  });

  testWidgets('never two enabled primary buttons, in any step and state', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = _client(
      _opencode(installed: true, versions: const <String>['1.2.0', '1.1.0']),
    );
    await _pump(tester, client);
    await _tap(tester, 'start-agent-setup-$_id');
    int primaries() => tester
        .widgetList<HButton>(find.byType(HButton))
        .where((HButton b) => b.variant == HButtonVariant.primary && b.enabled)
        .length;
    for (final SetupStepName step in SetupStepName.values) {
      await _tap(tester, 'agent-setup-step-${step.name}');
      expect(primaries(), lessThanOrEqualTo(1), reason: step.name);
    }
    // Inferenz: getestet ist „Server nutzen" offen, „Weiter" tritt zurück.
    await _tap(tester, 'agent-setup-step-inference');
    await _tap(tester, 'agent-setup-server-test');
    expect(
      tester
          .widget<HButton>(find.byKey(const Key('agent-setup-server-use')))
          .enabled,
      isTrue,
    );
    expect(primaries(), 1);
    expect(
      tester.widget<HButton>(find.byKey(const Key('agent-setup-next'))).variant,
      HButtonVariant.secondary,
    );
    // Eigene Provider bestätigt: wieder genau einer.
    await _tap(tester, 'agent-setup-inference-own');
    await _tap(tester, 'agent-setup-cloud-confirm');
    expect(primaries(), 1);
    // Quelle umgestellt, aber nicht übernommen: genau einer.
    await _tap(tester, 'agent-setup-step-source');
    await _tap(tester, 'agent-setup-source-host');
    expect(primaries(), 1);
  });

  testWidgets(
    'a run waiting for confirmation keeps its accept as the only primary',
    (WidgetTester tester) async {
      final FakeDaemonClient client = _client(
        _opencode(
          installed: true,
          run: const AgentMaintenance(
            sandboxId: 'maint-opencode-update',
            task: AgentRunTask.update,
            state: AgentRunState.confirm,
          ),
        ),
      );
      await _pump(tester, client);
      await _tap(tester, 'start-agent-setup-$_id');
      for (final String step in <String>['install', 'update']) {
        await _tap(tester, 'agent-setup-step-$step');
        expect(
          tester
              .widgetList<HButton>(find.byType(HButton))
              .where(
                (HButton b) => b.variant == HButtonVariant.primary && b.enabled,
              )
              .length,
          lessThanOrEqualTo(1),
          reason: step,
        );
      }
    },
  );
}
