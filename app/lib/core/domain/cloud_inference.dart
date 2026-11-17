/// Inference at a model provider outside this machine (HUM-516).
///
/// A plain value type, mirror of `CloudInference`: the daemon decides
/// whether the agent's model endpoint lies outside this machine and its
/// network, and whether the person confirmed that once for this agent and
/// provider (ADR-018). The app only draws it.
library;

import 'diagnostic.dart';

/// An agent whose inference goes to a provider outside this machine.
final class CloudInference {
  /// Creates the notice.
  const CloudInference({
    required this.agent,
    required this.provider,
    this.acknowledged = false,
    this.notice,
  });

  /// The id of the agent's adapter (`agent.adapter`), such as `opencode`.
  final String agent;

  /// The provider's host, such as `api.anthropic.com`.
  final String provider;

  /// True once the person confirmed the notice for this agent and provider;
  /// until then a start asks first.
  final bool acknowledged;

  /// The whole notice as `LLM_010`, for a client without its own text.
  final Diagnostic? notice;

  /// True while a start waits for the one confirmation.
  bool get needsConfirmation => !acknowledged;

  /// The same notice, confirmed.
  CloudInference confirmed() => CloudInference(
    agent: agent,
    provider: provider,
    acknowledged: true,
    notice: notice,
  );

  @override
  bool operator ==(Object other) =>
      other is CloudInference &&
      other.agent == agent &&
      other.provider == provider &&
      other.acknowledged == acknowledged &&
      other.notice == notice;

  @override
  int get hashCode => Object.hash(agent, provider, acknowledged, notice);
}
