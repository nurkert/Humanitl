/// The facts of the guided agent setup, gathered from the daemon's answers
/// (GitHub #111). Reads, never decides: each value is one the daemon gave.
library;

import '../../../core/domain/domain.dart';
import 'setup_steps.dart';

/// Where the program of the agent lies, by source, as the proposal names
/// it: the sources of the option of [agent], or the field `agent_source` of
/// a daemon from #15 on, which speaks of the chosen agent only. Null for a
/// source the proposal does not name.
AgentSourceOption? proposedSource(
  StartProposal? proposal,
  String agent,
  String source,
) {
  final ProposalField? field = proposal?.field('agent');
  ProposalOption? option;
  for (final ProposalOption each
      in field?.options ?? const <ProposalOption>[]) {
    if (each.value == agent) {
      option = each;
    }
  }
  for (final AgentSourceOption each
      in option?.sources ?? const <AgentSourceOption>[]) {
    if (each.source == source) {
      return each;
    }
  }
  if (field?.value != agent) {
    // Ohne Quellen an der Option: Ein Agent, den der Vorschlag nicht gewählt
    // hat, ist auf dem Host da, sobald der Daemon die Option anbietet; sonst
    // nennt deren Grund (`AGENT_001`) das Fehlen.
    return option == null || source != AgentSource.host
        ? null
        : AgentSourceOption(
            source: AgentSource.host,
            available: option.available,
            reasons: option.reasons,
          );
  }
  for (final ProposalOption option
      in proposal?.field('agent_source')?.options ?? const <ProposalOption>[]) {
    if (option.value == source) {
      return AgentSourceOption(
        source: source,
        available: option.available,
        reasons: option.reasons,
      );
    }
  }
  return null;
}

/// The value of `agents.<agent>.inference` in [config]; the model server
/// when it is not set.
String inferenceOf(ConfigValues? config, String agent) =>
    switch (config?.at('agents.$agent.inference')) {
      final String value when value.isNotEmpty => value,
      _ => inferenceModelServer,
    };

/// Whether the daemon offers the agent's own providers: its catalog has
/// `agents.<agent>.inference` only for an adapter that declares them.
bool ownInferenceOffered(SettingsCatalog? catalog, String agent) =>
    catalog?.entry('agents.$agent.inference') != null;

/// The catalog entry of `agents.<agent>.inference`, with its lock.
SettingsCatalogEntry? inferenceEntry(SettingsCatalog? catalog, String agent) =>
    catalog?.entry('agents.$agent.inference');

/// Whether a model server is set for the project: the server the proposal
/// chose for it, else the active saved server. True while neither answer
/// is there, so a missing answer never asks for a server by itself.
bool serverSet(StartProposal? proposal, SavedServers? servers) {
  final ProposalField? field = proposal?.field('llm_server');
  if (field != null) {
    return field.value.isNotEmpty && (field.chosen?.available ?? false);
  }
  if (servers != null) {
    return servers.activeServer != null;
  }
  return true;
}

/// The facts of the steps.
SetupFacts setupFacts({
  required AgentDepot? depot,
  required StartProposal? proposal,
  required SavedServers? servers,
  required List<Diagnostic> findings,
  required ConfigValues? config,
  required SettingsCatalog? catalog,
  required String agent,
}) => SetupFacts(
  depot: depot,
  findings: findings,
  hostAvailable: proposedSource(proposal, agent, AgentSource.host)?.available,
  inference: inferenceOf(config, agent),
  ownSupported: ownInferenceOffered(catalog, agent),
  hasServer: serverSet(proposal, servers),
);
