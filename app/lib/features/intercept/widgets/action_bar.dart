/// The action bar under the card: Block on the left, Allow on the right, and
/// between them the sentence that says why the request is waiting.
///
/// Allow and Block are never adjacent (BACKLOG.md 5). A refused decision
/// becomes a diagnostic card right below the bar; nothing here opens a modal.
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/flows.dart';

/// The gap that keeps the two decisions apart.
const double decisionGap = 24;

/// The action bar.
class ActionBar extends ConsumerWidget {
  /// Creates the bar for [flow]; a null flow disables both buttons.
  const ActionBar({required this.flow, super.key});

  /// The selected flow, or null while nothing is selected.
  final Flow? flow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow? flow = this.flow;
    final DecisionProgress progress = ref.watch(interceptDecisionProvider);
    final bool enabled = flow != null && flow.isHeld && !progress.isSending;
    void decide(Decision decision) {
      if (flow == null) {
        return;
      }
      unawaited(
        ref.read(interceptDecisionProvider.notifier).send(flow.id, decision),
      );
    }

    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(top: BorderSide(color: tokens.colors.line)),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Row(
              children: <Widget>[
                HButton(
                  key: const Key('intercept-block'),
                  variant: HButtonVariant.danger,
                  size: HButtonSize.md,
                  onPressed: enabled
                      ? () => decide(const Decision.block())
                      : null,
                  child: Text(l10n.interceptBlockButton),
                ),
                const SizedBox(width: decisionGap),
                Expanded(
                  child: Text(
                    l10n.interceptHoldReason,
                    textAlign: TextAlign.center,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ),
                const SizedBox(width: decisionGap),
                HButton(
                  key: const Key('intercept-allow'),
                  variant: HButtonVariant.primary,
                  size: HButtonSize.md,
                  onPressed: enabled
                      ? () => decide(const Decision.allow())
                      : null,
                  child: Text(l10n.interceptAllowButton),
                ),
              ],
            ),
            if (progress is DecisionFailed) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              Align(
                alignment: Alignment.centerLeft,
                child: HDiagnosticCard(
                  key: const Key('intercept-decision-error'),
                  code: progress.diagnostic.code,
                  severityLabel: severityLabel(
                    l10n,
                    progress.diagnostic.severity,
                  ),
                  color: severityColor(tokens, progress.diagnostic.severity),
                  title: l10n.interceptDecisionFailedTitle,
                  why: l10n.interceptDecisionFailedWhy,
                  detail: progress.diagnostic.why,
                  docsUrl: progress.diagnostic.docsUrl,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// The label of [severity] in the person's language.
  static String severityLabel(AppLocalizations l10n, Severity severity) =>
      switch (severity) {
        Severity.info => l10n.diagSeverityInfo,
        Severity.warning => l10n.diagSeverityWarning,
        Severity.error => l10n.diagSeverityError,
        Severity.blocking => l10n.diagSeverityBlocking,
      };

  /// The hue of [severity]. Never the blocked red: red means blocked.
  static Color severityColor(HTokens tokens, Severity severity) =>
      switch (severity) {
        Severity.info => tokens.colors.accent,
        Severity.warning => tokens.state.held,
        Severity.error || Severity.blocking => tokens.state.error,
      };
}
