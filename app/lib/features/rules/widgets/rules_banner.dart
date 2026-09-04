/// What the list as a whole has to say: a file the engine refused, a reorder
/// the daemon did not take, the report of a reload -- and the strip that
/// offers to take back the last reversible change.
///
/// Both sit above the list, in a slot that is only there when something is in
/// it, and both arrive the way an arrival arrives (`docs/UX.md` 2.2, 4.4,
/// 4.5).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/rules.dart';
import '../severity.dart';
import 'arrive.dart';

/// The diagnostics of the rule set, above the list.
class RulesBannerView extends ConsumerWidget {
  /// Creates the banner.
  const RulesBannerView({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<Diagnostic> found = ref.watch(rulesBannerProvider);
    if (found.isEmpty) {
      return const SizedBox.shrink();
    }
    final Diagnostic first = found.first;
    return ArriveIn(
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          tokens.spacing.x3,
          tokens.spacing.x2,
          tokens.spacing.x3,
          0,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            HDiagnosticCard(
              code: first.code,
              severityLabel: ruleSeverityLabel(l10n, first.severity),
              color: ruleSeverityColor(tokens, first.severity),
              title: first.title.isEmpty ? first.code : first.title,
              // The daemon's own sentence, with the field and the line it
              // names. The app writes the title, never the reason
              // (`docs/UX.md` 4.4).
              why: first.why,
              docsUrl: first.docsUrl,
              width: double.infinity,
            ),
            SizedBox(height: tokens.spacing.x2),
            Row(
              children: <Widget>[
                HButton(
                  key: const Key('rules-banner-reload'),
                  onPressed: ref.read(rulesProvider.notifier).reload,
                  child: Text(l10n.rulesReload),
                ),
                SizedBox(width: tokens.spacing.x2),
                HButton(
                  variant: HButtonVariant.ghost,
                  onPressed: ref.read(rulesBannerProvider.notifier).clear,
                  child: Text(l10n.rulesBannerDismiss),
                ),
                if (found.length > 1) ...<Widget>[
                  SizedBox(width: tokens.spacing.x2),
                  Text(
                    l10n.rulesBannerMore(found.length - 1),
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ],
              ],
            ),
          ],
        ),
      ),
    );
  }
}

/// "Rule removed · Undo", for [HMotion.undoWindow].
///
/// Undo takes back the rule, and only the rule. Requests that already went
/// out while it applied are gone; the strip says what it can take back and
/// nothing more (`docs/UX.md` 4.5).
class RuleUndoStrip extends ConsumerWidget {
  /// Creates the strip.
  const RuleUndoStrip({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final RuleUndo? undo = ref.watch(ruleUndoProvider);
    if (undo == null) {
      return const SizedBox.shrink();
    }
    return ArriveIn(
      child: Container(
        constraints: BoxConstraints(minHeight: tokens.sizes.row),
        padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
        color: tokens.tint(tokens.colors.accent),
        child: Row(
          children: <Widget>[
            Expanded(
              child: Text(switch (undo.kind) {
                RuleUndoKind.removed => l10n.rulesUndoRemoved,
                RuleUndoKind.madePermanent => l10n.rulesUndoPermanent,
              }, style: tokens.typography.ui13.tinted(tokens.colors.fg0)),
            ),
            HButton(
              key: const Key('rules-undo'),
              onPressed: () async {
                final Diagnostic? failed = await ref
                    .read(ruleUndoProvider.notifier)
                    .apply();
                if (failed != null) {
                  ref.read(rulesBannerProvider.notifier).showOne(failed);
                }
              },
              child: Text(l10n.rulesUndo),
            ),
          ],
        ),
      ),
    );
  }
}
