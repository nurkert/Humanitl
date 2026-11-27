/// The guided agent setup of the start window (GitHub #111).
///
/// It stands in place of the project list for one project: where the
/// agent's program comes from, install and sign-in for a depot agent, where
/// the inference goes, keeping a depot agent current, and a summary with the
/// start. It opens on its own when a start finds the agent not ready (a
/// depot agent without install or login before the start, `AGENT_001` or a
/// missing model server in the daemon's answer), and on purpose from the
/// agent cell of a project row.
///
/// Every action is a call to the daemon (ADR-018), the same one the
/// settings and the command line make: `Agents` for source, install, login
/// and update, `Projects(save_choice)` for the server of this project,
/// `LlmServers` for the server of all projects, and `SetConfig` for
/// `agents.<id>.inference`. The state of a maintenance run lives in the
/// daemon (`AgentDepot.maintenance`), so a run survives the window, and the
/// setup shows it again when it opens.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/agents_provider.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../setup/setup_facts.dart';
import '../setup/setup_steps.dart';

/// Which project the setup is for.
final class AgentSetupTarget {
  /// Creates the target.
  const AgentSetupTarget({
    required this.path,
    this.projectId = '',
    this.displayName = '',
    this.agent = '',
    this.findings = const <Diagnostic>[],
  });

  /// The project as the register lists it.
  factory AgentSetupTarget.of(
    ProjectEntry entry, {
    List<Diagnostic> findings = const <Diagnostic>[],
  }) => AgentSetupTarget(
    path: entry.path,
    projectId: entry.projectId,
    displayName: entry.displayName,
    agent: entry.lastAgent,
    findings: findings,
  );

  /// The folder, cleaned for display.
  final String path;

  /// The id in the register; empty for a folder of the first-start screen.
  final String projectId;

  /// The folder name.
  final String displayName;

  /// The adapter id of the agent as last known; the proposal of the project
  /// names the one its next start takes.
  final String agent;

  /// What the daemon said against the start that led here.
  final List<Diagnostic> findings;

  /// Whether the register knows the project, so the setup can save a
  /// choice for it and start it.
  bool get known => projectId.isNotEmpty;
}

/// Where a chosen model server is saved (ADR-0025): in the start choice of
/// this project, or as the default server of every project.
enum ServerLevel {
  /// `llm.server` in the project choice.
  project,

  /// The active saved server.
  all,
}

/// What the setup shows.
final class AgentSetup {
  /// Creates the state.
  const AgentSetup({
    required this.target,
    this.step,
    this.proposal,
    this.servers,
    this.loading = true,
    this.busy = false,
    this.refusal,
    this.probe,
    this.probedServer = '',
    this.probing = false,
  });

  /// The project.
  final AgentSetupTarget target;

  /// The step the person chose; null opens the first one not done.
  final SetupStepId? step;

  /// The daemon's start proposal for the project.
  final StartProposal? proposal;

  /// The saved model servers.
  final SavedServers? servers;

  /// True until proposal and servers answered.
  final bool loading;

  /// True while an action waits for the daemon.
  final bool busy;

  /// What the daemon said against the last action.
  final Diagnostic? refusal;

  /// The last test of a saved server.
  final LlmProbe? probe;

  /// The server [probe] belongs to.
  final String probedServer;

  /// True while a server is tested.
  final bool probing;

  /// The agent's adapter id: the one the setup was opened for, else the one
  /// the next start takes, as the proposal names it.
  String get agent {
    if (target.agent.isNotEmpty) {
      return target.agent;
    }
    return proposal?.field('agent')?.value ?? '';
  }

  /// The name of the project folder: the proposal's, else the list's.
  String get projectName {
    final String named = proposal?.displayName ?? '';
    return named.isEmpty ? target.displayName : named;
  }

  /// The findings of the start and of the proposal.
  List<Diagnostic> get findings => <Diagnostic>[
    ...target.findings,
    ...?proposal?.diagnostics,
  ];

  /// A copy with the given parts replaced; [clearRefusal] drops the last
  /// refusal, [clearProbe] the last test.
  AgentSetup copyWith({
    AgentSetupTarget? target,
    SetupStepId? step,
    StartProposal? proposal,
    SavedServers? servers,
    bool? loading,
    bool? busy,
    Diagnostic? refusal,
    bool clearRefusal = false,
    LlmProbe? probe,
    String? probedServer,
    bool clearProbe = false,
    bool? probing,
  }) => AgentSetup(
    target: target ?? this.target,
    step: step ?? this.step,
    proposal: proposal ?? this.proposal,
    servers: servers ?? this.servers,
    loading: loading ?? this.loading,
    busy: busy ?? this.busy,
    refusal: clearRefusal ? null : (refusal ?? this.refusal),
    probe: clearProbe ? null : (probe ?? this.probe),
    probedServer: clearProbe ? '' : (probedServer ?? this.probedServer),
    probing: probing ?? this.probing,
  );
}

/// The setup; null while the start window shows its list.
final NotifierProvider<AgentSetupNotifier, AgentSetup?> agentSetupProvider =
    NotifierProvider.autoDispose<AgentSetupNotifier, AgentSetup?>(
      AgentSetupNotifier.new,
    );

/// The notifier behind [agentSetupProvider].
class AgentSetupNotifier extends Notifier<AgentSetup?> {
  @override
  AgentSetup? build() => null;

  DaemonClient get _client => ref.read(daemonClientProvider);

  /// Opens the setup for [target], at [step] or at the first step that is
  /// not done.
  Future<void> open(AgentSetupTarget target, {SetupStepId? step}) async {
    final AgentSetup opening = AgentSetup(target: target, step: step);
    state = opening;
    await _load(opening);
  }

  /// Leaves the setup; what was saved stays saved.
  void close() => state = null;

  /// Shows [step].
  void select(SetupStepId step) {
    final AgentSetup? now = state;
    if (now != null) {
      state = now.copyWith(step: step, clearRefusal: true);
    }
  }

  /// `Agents(set_source)`: the program from [source].
  Future<void> setSource(String source) => maintain(
    AgentsOp.setSource(state?.agent ?? '', source),
    clears: sourceCodes,
  );

  /// Sends [op] to `Agents` through the state the settings share; the
  /// findings of the start in [clears] go once the daemon took it.
  Future<void> maintain(AgentsOp op, {Set<String> clears = const {}}) async {
    final AgentSetup? now = state;
    if (now == null || now.busy || op.agent.isEmpty) {
      return;
    }
    state = now.copyWith(busy: true, clearRefusal: true);
    final (AgentsStatus? status, Diagnostic? refusal) = await ref
        .read(agentsProvider.notifier)
        .run(op);
    if (!ref.mounted || state == null) {
      return;
    }
    if (status == null) {
      state = state!.copyWith(busy: false, refusal: refusal);
      return;
    }
    await _reload(clears: clears);
  }

  /// Tests the saved server [name] (`ProbeLlm` with that server's key, sent
  /// only to its own origin). Contacts the server: only on the button.
  Future<void> probe(SavedServer server) async {
    final AgentSetup? now = state;
    if (now == null || now.probing) {
      return;
    }
    state = now.copyWith(probing: true, clearProbe: true, clearRefusal: true);
    try {
      final LlmProbe result = await _client.probeLlm(
        server.endpoint,
        server: server.name,
      );
      if (ref.mounted && state != null) {
        state = state!.copyWith(
          probing: false,
          probe: result,
          probedServer: server.name,
        );
      }
    } on DaemonException catch (error) {
      if (ref.mounted && state != null) {
        state = state!.copyWith(probing: false, refusal: error.diagnostic);
      }
    }
  }

  /// Takes the saved server [name] over for this project or for all, and
  /// sets `agents.<id>.inference` back to the model server when it was not.
  Future<void> useServer(String name, ServerLevel level) =>
      _act(clears: inferenceCodes, () async {
        final AgentSetup now = state!;
        if (level == ServerLevel.project && now.target.known) {
          final StartProposal asked = await _client.projectsProposal(
            projectId: now.target.projectId,
            values: <ChoiceValue>[ChoiceValue('llm.server', name)],
          );
          await _client.projectsSaveChoice(
            projectId: now.target.projectId,
            values: asked.choice,
          );
        } else {
          await _client.activateLlmServer(name);
        }
        if (_inference(now) != inferenceModelServer) {
          await ref
              .read(configProvider.notifier)
              .write(_inferenceKey(now), inferenceModelServer);
        }
      });

  /// Saves a new model server (`LlmServers(add)`); its test and its use
  /// follow as for any saved one.
  Future<void> addServer(String name, String endpoint) =>
      _act(() => _client.addLlmServer(name: name, endpoint: endpoint));

  /// The agent's own providers, after the person confirmed the cloud notice
  /// on the screen: `agents.<id>.inference = own` (for every project, the
  /// key has no project level), then the confirmation of the daemon's
  /// notice for agent and provider, as the start would ask it.
  ///
  /// When the proposal or the confirmation fails after the write, the key
  /// goes back to its value before, and the refusal stands at the step: the
  /// agent never keeps its own providers without a confirmed notice.
  Future<void> useOwn() => _act(clears: inferenceCodes, () async {
    final AgentSetup now = state!;
    final String before = _inference(now);
    final ConfigSetting config = ref.read(configProvider.notifier);
    await config.write(_inferenceKey(now), inferenceOwn);
    try {
      final StartProposal asked = await _ask(now);
      final CloudInference? cloud = asked.cloudInference;
      if (cloud != null && cloud.needsConfirmation) {
        await _client.acknowledgeCloudInference(cloud.agent, cloud.provider);
      }
    } on DaemonException {
      await config.write(_inferenceKey(now), before);
      rethrow;
    }
  });

  /// The value of `agents.<id>.inference` as the daemon reads it.
  String _inference(AgentSetup now) =>
      inferenceOf(ref.read(configProvider).value, now.agent);

  static String _inferenceKey(AgentSetup now) =>
      'agents.${now.agent}.inference';

  /// Runs [action]; a refusal stands at the step, otherwise the setup reads
  /// the daemon's state again and drops the findings in [clears].
  Future<void> _act(
    Future<void> Function() action, {
    Set<String> clears = const {},
  }) async {
    final AgentSetup? now = state;
    if (now == null || now.busy) {
      return;
    }
    state = now.copyWith(busy: true, clearRefusal: true);
    try {
      await action();
    } on DaemonException catch (error) {
      if (ref.mounted && state != null) {
        state = state!.copyWith(busy: false, refusal: error.diagnostic);
      }
      return;
    }
    if (ref.mounted && state != null) {
      await _reload(clears: clears);
    }
  }

  Future<StartProposal> _ask(AgentSetup now) => _client.projectsProposal(
    projectId: now.target.known ? now.target.projectId : null,
    path: now.target.known ? null : now.target.path,
  );

  Future<void> _load(AgentSetup opening) async {
    StartProposal? proposal;
    SavedServers? servers;
    Diagnostic? failure;
    try {
      proposal = await _ask(opening);
    } on DaemonException catch (error) {
      failure = error.diagnostic;
    }
    try {
      servers = await _client.listLlmServers();
    } on DaemonException catch (error) {
      failure ??= error.diagnostic;
    }
    if (!ref.mounted || state != opening) {
      return;
    }
    state = opening.copyWith(
      proposal: proposal,
      servers: servers,
      loading: false,
      refusal: failure,
    );
  }

  Future<void> _reload({required Set<String> clears}) async {
    final AgentSetup? now = state;
    if (now == null) {
      return;
    }
    // Die Befunde des abgelehnten Starts gelten nur, bis der Schritt, in
    // den sie führen, etwas geändert hat; danach spricht der Vorschlag.
    final AgentSetup cleared = now.copyWith(
      target: AgentSetupTarget(
        path: now.target.path,
        projectId: now.target.projectId,
        displayName: now.target.displayName,
        agent: now.target.agent,
        findings: <Diagnostic>[
          for (final Diagnostic d in now.target.findings)
            if (!clears.contains(d.code)) d,
        ],
      ),
      busy: false,
    );
    state = cleared;
    await _load(cleared);
  }
}
