/// The one question before the first start with an agent whose inference
/// goes to a provider outside this machine (HUM-516).
///
/// The text is factual: what the agent reads goes to the provider as part of
/// its requests; Humanitl isolates the system and holds every other
/// connection, and the inference itself is deliberately allowed. The daemon
/// keeps the confirmation per agent and provider; this modal is how the
/// person gives it. Cancel is preselected, `Escape` and the scrim cancel,
/// and only a deliberate press on the second button confirms and starts
/// (`docs/UX.md` 5.4). Afterwards the same combination starts without it.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// Asks before the first start with [cloud].
class CloudConfirmDialog extends StatelessWidget {
  /// Creates the question. [onCancel] closes it, [onConfirm] confirms and
  /// starts.
  const CloudConfirmDialog({
    required this.cloud,
    required this.onCancel,
    required this.onConfirm,
    super.key,
  });

  /// The agent and the provider its inference goes to.
  final CloudInference cloud;

  /// Closes the question and starts nothing.
  final VoidCallback onCancel;

  /// Confirms for this agent and provider and starts.
  final VoidCallback onConfirm;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SandboxArrive(
      from: ArriveFrom.nowhere,
      child: HModal(
        key: const Key('sandbox-cloud-confirm'),
        title: Text(l10n.cloudInferenceConfirmTitle),
        onDismiss: onCancel,
        scrimSemanticsLabel: l10n.cloudInferenceConfirmCancel,
        width: 580,
        actions: <Widget>[
          HButton(
            key: const Key('sandbox-cloud-confirm-cancel'),
            variant: HButtonVariant.secondary,
            autofocus: true,
            onPressed: onCancel,
            child: Text(l10n.cloudInferenceConfirmCancel),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('sandbox-cloud-confirm-yes'),
            variant: HButtonVariant.primary,
            onPressed: onConfirm,
            child: Text(l10n.cloudInferenceConfirmStart),
          ),
        ],
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.cloudInferenceNotice(cloud.provider),
              key: const Key('sandbox-cloud-confirm-body'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.cloudInferenceConfirmOnce,
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x3),
            _Fact(label: l10n.cloudInferenceConfirmAgent, value: cloud.agent),
            SizedBox(height: tokens.spacing.x2),
            _Fact(
              key: const Key('sandbox-cloud-confirm-provider'),
              label: l10n.cloudInferenceConfirmProvider,
              value: cloud.provider,
            ),
          ],
        ),
      ),
    );
  }
}

/// One fact of the question: a small label over a value in mono.
class _Fact extends StatelessWidget {
  const _Fact({required this.label, required this.value, super.key});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(label, style: tokens.typography.ui11.tinted(tokens.colors.fg2)),
        SizedBox(height: tokens.spacing.x1 / 2),
        Text(value, style: tokens.typography.mono12.tinted(tokens.colors.fg0)),
      ],
    );
  }
}
