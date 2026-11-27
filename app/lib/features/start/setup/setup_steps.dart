/// The steps of the guided agent setup and their state (GitHub #111).
///
/// Pure functions over what the daemon answered: the depot entry of the
/// agent (`Agents`), the start proposal of the project (`Projects`), the
/// saved model servers (`LlmServers`), the value of
/// `agents.<id>.inference` (`GetConfig`) and the findings of a refused
/// start. Nothing here asks the host or decides what the daemon decides
/// (ADR-018); it only sorts the daemon's facts into five steps, so the
/// window can open at the first one that needs a hand.
library;

import '../../../core/domain/domain.dart';

/// The five steps, in their order on the screen.
enum SetupStepId {
  /// Host install or agent depot.
  source,

  /// Install and sign in, for a depot agent.
  install,

  /// Where the inference goes.
  inference,

  /// Keep a depot agent up to date.
  update,

  /// The summary with the start.
  ready,
}

/// What a step needs.
enum SetupStepState {
  /// Nothing to do.
  done,

  /// The person has to act here.
  action,

  /// A maintenance run of this step goes on in the daemon.
  running,

  /// An earlier step has to be done first.
  blocked,
}

/// The value of `agents.<id>.inference` that keeps the inference on the
/// model server Humanitl sets; the default.
const String inferenceModelServer = 'model-server';

/// The value of `agents.<id>.inference` that lets the agent use its own
/// providers, behind the cloud notice.
const String inferenceOwn = 'own';

/// Findings of a start that lead into the source step.
const Set<String> sourceCodes = <String>{
  'AGENT_001',
  'AGENT_002',
  'AGENT_004',
  'AGENT_015',
  'AGENT_017',
};

/// Findings of a start that lead into the step install and sign in.
const Set<String> installCodes = <String>{
  'AGENT_008',
  'AGENT_009',
  'AGENT_011',
  'AGENT_013',
  'AGENT_014',
};

/// Findings of a start that lead into the inference step.
const Set<String> inferenceCodes = <String>{
  'LLM_011',
  'LLM_012',
  'LLM_013',
  'LLM_014',
};

/// The step a finding of the start leads into, or null for a finding the
/// setup does not cover.
SetupStepId? setupStepFor(String code) => switch (code) {
  _ when sourceCodes.contains(code) => SetupStepId.source,
  _ when installCodes.contains(code) => SetupStepId.install,
  _ when inferenceCodes.contains(code) => SetupStepId.inference,
  _ => null,
};

/// Whether [finding] holds a start up: an error or a blocking finding. A
/// warning, such as `AGENT_009` for a depot agent without a login, lets the
/// start go and never leads into the setup by itself.
bool refuses(Diagnostic finding) =>
    finding.severity == Severity.error || finding.severity == Severity.blocking;

/// Whether any of [findings] holds the start up and leads into a step of
/// the setup.
bool leadsIntoSetup(Iterable<Diagnostic> findings) =>
    findings.any((Diagnostic d) => refuses(d) && setupStepFor(d.code) != null);

/// The findings among [findings] that hold the start up and that no step of
/// the setup solves; the setup shows them above its steps, so nothing the
/// daemon said gets lost.
List<Diagnostic> uncoveredFindings(Iterable<Diagnostic> findings) =>
    <Diagnostic>[
      for (final Diagnostic d in findings)
        if (refuses(d) && setupStepFor(d.code) == null) d,
    ];

/// The facts the steps are sorted from, all of them the daemon's.
final class SetupFacts {
  /// Creates the facts.
  const SetupFacts({
    this.depot,
    this.findings = const <Diagnostic>[],
    this.hostAvailable,
    this.inference = inferenceModelServer,
    this.ownSupported = false,
    this.hasServer = true,
  });

  /// The depot entry of the agent; null while `Agents` did not answer.
  final AgentDepot? depot;

  /// The findings of the refused start and of the proposal.
  final List<Diagnostic> findings;

  /// Whether the program lies on the host, as the proposal says; null when
  /// it says nothing.
  final bool? hostAvailable;

  /// The value of `agents.<id>.inference`.
  final String inference;

  /// Whether the daemon offers `agents.<id>.inference` for this agent.
  final bool ownSupported;

  /// Whether a model server is set for the project.
  final bool hasServer;

  /// Whether the agent's program comes from the depot.
  bool get fromDepot => depot?.source == AgentSource.depot;

  bool _found(Set<String> codes) =>
      findings.any((Diagnostic d) => refuses(d) && codes.contains(d.code)) ||
      (depot?.diagnostics.any(
            (Diagnostic d) => refuses(d) && codes.contains(d.code),
          ) ??
          false);
}

/// One step with its state.
final class SetupStep {
  /// Creates the step.
  const SetupStep(this.id, this.state);

  /// Which step.
  final SetupStepId id;

  /// What it needs.
  final SetupStepState state;

  /// Whether nothing is to do here.
  bool get done => state == SetupStepState.done;
}

/// The five steps for [facts], in order.
List<SetupStep> setupSteps(SetupFacts facts) {
  final SetupStepState source = _source(facts);
  final SetupStepState install = _install(facts, source);
  final SetupStepState inference = _inference(facts);
  final SetupStepState update = _update(facts, install);
  final bool ready = <SetupStepState>[
    source,
    install,
    inference,
    update,
  ].every((SetupStepState s) => s == SetupStepState.done);
  return <SetupStep>[
    SetupStep(SetupStepId.source, source),
    SetupStep(SetupStepId.install, install),
    SetupStep(SetupStepId.inference, inference),
    SetupStep(SetupStepId.update, update),
    SetupStep(
      SetupStepId.ready,
      ready ? SetupStepState.done : SetupStepState.blocked,
    ),
  ];
}

/// The step the setup opens at: the first one that is not done, else the
/// summary.
SetupStepId firstOpenStep(List<SetupStep> steps) {
  for (final SetupStep step in steps) {
    if (!step.done) {
      return step.id;
    }
  }
  return SetupStepId.ready;
}

/// Whether a start of a project with [depot] should open the setup instead:
/// a depot agent that is not installed, whose last install or sign-in
/// failed, or whose depot the daemon calls unusable. A missing login does
/// not: the agent starts and may need none (a local server, free models),
/// and the daemon only warns (`AGENT_009`). A host agent never does here; the daemon says so
/// at the start (`AGENT_001`).
bool depotNeedsSetup(AgentDepot depot) {
  if (depot.source != AgentSource.depot) {
    return false;
  }
  return _installState(depot) != SetupStepState.done;
}

SetupStepState _source(SetupFacts facts) {
  final AgentDepot? depot = facts.depot;
  if (facts._found(sourceCodes)) {
    return SetupStepState.action;
  }
  if (depot != null && facts.fromDepot && !depot.depotSupported) {
    return SetupStepState.action;
  }
  if (!facts.fromDepot && facts.hostAvailable == false) {
    return SetupStepState.action;
  }
  return SetupStepState.done;
}

SetupStepState _install(SetupFacts facts, SetupStepState source) {
  final AgentDepot? depot = facts.depot;
  if (depot == null || !facts.fromDepot) {
    // Auf dem Host installiert und meldet sich der Agent selbst an; der
    // Schritt hat dann nichts zu tun, außer die Quelle fehlt noch.
    return source == SetupStepState.done
        ? SetupStepState.done
        : SetupStepState.blocked;
  }
  if (!depot.depotSupported) {
    return SetupStepState.blocked;
  }
  if (facts._found(installCodes)) {
    final SetupStepState own = _installState(depot);
    return own == SetupStepState.done ? SetupStepState.action : own;
  }
  return _installState(depot);
}

/// The state of install and sign-in from the depot entry alone. The sign-in
/// is offered on the step but never holds it open.
SetupStepState _installState(AgentDepot depot) {
  final AgentMaintenance? run = depot.maintenance;
  final bool ownRun =
      run != null &&
      (run.task == AgentRunTask.install || run.task == AgentRunTask.login);
  if (ownRun && run.running) {
    return SetupStepState.running;
  }
  if (!depot.installed) {
    return SetupStepState.action;
  }
  if (depot.diagnostics.any(
    (Diagnostic d) => refuses(d) && installCodes.contains(d.code),
  )) {
    return SetupStepState.action;
  }
  if (ownRun && run.state == AgentRunState.failed) {
    return SetupStepState.action;
  }
  return SetupStepState.done;
}

SetupStepState _inference(SetupFacts facts) {
  if (facts.findings.any(
    (Diagnostic d) => refuses(d) && inferenceCodes.contains(d.code),
  )) {
    return SetupStepState.action;
  }
  if (facts.ownSupported && facts.inference == inferenceOwn) {
    return SetupStepState.done;
  }
  return facts.hasServer ? SetupStepState.done : SetupStepState.action;
}

SetupStepState _update(SetupFacts facts, SetupStepState install) {
  final AgentDepot? depot = facts.depot;
  if (depot == null || !facts.fromDepot) {
    return install == SetupStepState.blocked
        ? SetupStepState.blocked
        : SetupStepState.done;
  }
  if (!depot.installed) {
    return SetupStepState.blocked;
  }
  final AgentMaintenance? run = depot.maintenance;
  if (run != null && run.task == AgentRunTask.update) {
    if (run.running) {
      return SetupStepState.running;
    }
    if (run.state == AgentRunState.failed) {
      return SetupStepState.action;
    }
  }
  return SetupStepState.done;
}
