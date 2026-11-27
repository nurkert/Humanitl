// Die Schritte der geführten Einrichtung (GitHub #111): welcher Schritt
// was braucht, allein aus den Antworten des Daemons, und wo die Einrichtung
// aufgeht.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/start/setup/setup_facts.dart';
import 'package:humanitl/features/start/setup/setup_steps.dart';

AgentDepot _depot({
  bool installed = false,
  bool loggedIn = false,
  AgentMaintenance? run,
  String source = AgentSource.depot,
  bool supported = true,
}) => FakeAgents.fakeAgent(
  'opencode',
  'OpenCode',
  depotSupported: supported,
  source: source,
  version: installed ? '1.0.0' : '',
  loggedIn: loggedIn,
  loginDeclared: true,
  maintenance: run,
);

Map<SetupStepId, SetupStepState> _states(SetupFacts facts) =>
    <SetupStepId, SetupStepState>{
      for (final SetupStep step in setupSteps(facts)) step.id: step.state,
    };

const Diagnostic _llm012 = Diagnostic(
  code: 'LLM_012',
  severity: Severity.blocking,
  why: 'no model server',
);

void main() {
  test('a ready host agent with a server is done everywhere and opens at '
      'the summary', () {
    final SetupFacts facts = SetupFacts(
      depot: _depot(source: AgentSource.host),
      hostAvailable: true,
    );
    expect(_states(facts).values, everyElement(SetupStepState.done));
    expect(firstOpenStep(setupSteps(facts)), SetupStepId.ready);
  });

  test('AGENT_001 leads into the source step; install and update wait', () {
    final Map<SetupStepId, SetupStepState> states = _states(
      SetupFacts(
        depot: _depot(source: AgentSource.host),
        findings: const <Diagnostic>[
          Diagnostic(code: 'AGENT_001', severity: Severity.blocking, why: ''),
        ],
      ),
    );
    expect(states[SetupStepId.source], SetupStepState.action);
    expect(states[SetupStepId.install], SetupStepState.blocked);
    expect(states[SetupStepId.update], SetupStepState.blocked);
    expect(states[SetupStepId.ready], SetupStepState.blocked);
  });

  test('a host program the proposal does not find needs the source step', () {
    final SetupFacts facts = SetupFacts(
      depot: _depot(source: AgentSource.host),
      hostAvailable: false,
    );
    expect(_states(facts)[SetupStepId.source], SetupStepState.action);
    expect(firstOpenStep(setupSteps(facts)), SetupStepId.source);
  });

  test('a depot agent without install opens at install; update waits', () {
    final SetupFacts facts = SetupFacts(depot: _depot());
    final Map<SetupStepId, SetupStepState> states = _states(facts);
    expect(states[SetupStepId.source], SetupStepState.done);
    expect(states[SetupStepId.install], SetupStepState.action);
    expect(states[SetupStepId.update], SetupStepState.blocked);
    expect(firstOpenStep(setupSteps(facts)), SetupStepId.install);
    expect(depotNeedsSetup(facts.depot!), isTrue);
  });

  test('installed but not signed in is done: the sign-in is optional, and '
      'its warning AGENT_009 leads nowhere', () {
    final AgentDepot depot = _depot(installed: true);
    expect(
      _states(
        SetupFacts(
          depot: depot,
          findings: const <Diagnostic>[
            Diagnostic(code: 'AGENT_009', severity: Severity.warning, why: ''),
          ],
        ),
      )[SetupStepId.install],
      SetupStepState.done,
    );
    expect(depotNeedsSetup(depot), isFalse);
    expect(
      leadsIntoSetup(const <Diagnostic>[
        Diagnostic(code: 'AGENT_009', severity: Severity.warning, why: ''),
      ]),
      isFalse,
    );
    expect(
      uncoveredFindings(const <Diagnostic>[
        Diagnostic(code: 'AGENT_001', severity: Severity.blocking, why: ''),
        Diagnostic(code: 'SANDBOX_045', severity: Severity.blocking, why: ''),
        Diagnostic(code: 'SANDBOX_033', severity: Severity.warning, why: ''),
      ]).map((Diagnostic d) => d.code),
      <String>['SANDBOX_045'],
    );
  });

  test('a running login shows as running, a failed one needs action', () {
    expect(
      _states(
        SetupFacts(
          depot: _depot(
            installed: true,
            run: const AgentMaintenance(
              task: AgentRunTask.login,
              state: AgentRunState.running,
            ),
          ),
        ),
      )[SetupStepId.install],
      SetupStepState.running,
    );
    expect(
      _states(
        SetupFacts(
          depot: _depot(
            installed: true,
            loggedIn: true,
            run: const AgentMaintenance(
              task: AgentRunTask.install,
              state: AgentRunState.failed,
            ),
          ),
        ),
      )[SetupStepId.install],
      SetupStepState.action,
    );
  });

  test('a failed update needs the update step, a running one runs', () {
    AgentDepot updating(String state) => _depot(
      installed: true,
      loggedIn: true,
      run: AgentMaintenance(task: AgentRunTask.update, state: state),
    );
    expect(
      _states(
        SetupFacts(depot: updating(AgentRunState.failed)),
      )[SetupStepId.update],
      SetupStepState.action,
    );
    expect(
      _states(
        SetupFacts(depot: updating(AgentRunState.running)),
      )[SetupStepId.update],
      SetupStepState.running,
    );
    expect(depotNeedsSetup(updating(AgentRunState.failed)), isFalse);
  });

  test('the missing model server leads into inference, own providers are '
      'done without one', () {
    final SetupFacts missing = SetupFacts(
      depot: _depot(source: AgentSource.host),
      hasServer: false,
    );
    expect(_states(missing)[SetupStepId.inference], SetupStepState.action);
    expect(firstOpenStep(setupSteps(missing)), SetupStepId.inference);
    expect(
      _states(
        SetupFacts(
          depot: _depot(source: AgentSource.host),
          hasServer: false,
          ownSupported: true,
          inference: inferenceOwn,
        ),
      )[SetupStepId.inference],
      SetupStepState.done,
    );
    // Ohne Angebot des Daemons zählt `own` nicht.
    expect(
      _states(
        SetupFacts(
          depot: _depot(source: AgentSource.host),
          hasServer: false,
          inference: inferenceOwn,
        ),
      )[SetupStepId.inference],
      SetupStepState.action,
    );
  });

  test('every inference finding of the start leads into inference', () {
    for (final String code in inferenceCodes) {
      expect(setupStepFor(code), SetupStepId.inference, reason: code);
      expect(
        _states(
          SetupFacts(
            depot: _depot(source: AgentSource.host),
            findings: <Diagnostic>[
              Diagnostic(code: code, severity: Severity.blocking, why: ''),
            ],
          ),
        )[SetupStepId.inference],
        SetupStepState.action,
        reason: code,
      );
    }
    expect(setupStepFor('AGENT_008'), SetupStepId.install);
    expect(setupStepFor('CLI_005'), isNull);
    expect(leadsIntoSetup(const <Diagnostic>[_llm012]), isTrue);
  });

  test('serverSet reads the proposal first, then the saved servers', () {
    const ProposalField chosen = ProposalField(
      id: 'llm_server',
      value: 'local',
      options: <ProposalOption>[
        ProposalOption(value: 'local', label: 'local', available: true),
      ],
    );
    const ProposalField locked = ProposalField(
      id: 'llm_server',
      value: 'cloud',
      options: <ProposalOption>[
        ProposalOption(value: 'cloud', label: 'cloud', available: false),
      ],
    );
    StartProposal with_(ProposalField field) => StartProposal(
      path: '/p',
      displayName: 'p',
      fields: <ProposalField>[field],
    );
    expect(serverSet(with_(chosen), null), isTrue);
    expect(serverSet(with_(locked), null), isFalse);
    expect(serverSet(null, const SavedServers()), isFalse);
    expect(serverSet(null, null), isTrue);
  });
}
