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
import '../../../core/ui/untranslated_wording.dart';
import '../../../l10n/l10n.dart';
import '../providers/rules.dart';
import '../severity.dart';
import 'arrive.dart';

/// The diagnostics of the rule set, above the list.
///
/// Every finding gets its card: two rules of `rules.yaml` that cannot be read
/// are two reasons, and the second must not wait until the first is fixed
/// (HUM-561). Past [collapsedCount] cards the rest folds away behind a button
/// that names how many there are, so a long list does not push the rules off
/// the screen.
class RulesBannerView extends ConsumerStatefulWidget {
  /// Creates the banner.
  const RulesBannerView({super.key});

  /// How many cards stand open before the rest folds away.
  static const int collapsedCount = 2;

  @override
  ConsumerState<RulesBannerView> createState() => _RulesBannerViewState();
}

class _RulesBannerViewState extends ConsumerState<RulesBannerView> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<Diagnostic> found = ref.watch(rulesBannerProvider);
    if (found.isEmpty) {
      return const SizedBox.shrink();
    }
    final int hidden = _expanded
        ? 0
        : (found.length - RulesBannerView.collapsedCount).clamp(
            0,
            found.length,
          );
    final List<Diagnostic> shown = found.sublist(0, found.length - hidden);
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
            for (final (int index, Diagnostic diagnostic)
                in shown.indexed) ...<Widget>[
              if (index > 0) SizedBox(height: tokens.spacing.x2),
              HDiagnosticCard(
                code: diagnostic.code,
                severityLabel: ruleSeverityLabel(l10n, diagnostic.severity),
                color: ruleSeverityColor(tokens, diagnostic.severity),
                title: diagnostic.title.isEmpty
                    ? diagnostic.code
                    : diagnostic.title,
                // The daemon's own sentence, with the field and the line it
                // names. The app writes the title, never the reason
                // (`docs/UX.md` 4.4).
                why: DiagnosticL10n.resolve(diagnostic, l10n).cause,
                original: UntranslatedWording.forDiagnostic(diagnostic, l10n),
                docsUrl: diagnostic.docsUrl,
                width: double.infinity,
              ),
            ],
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
                if (found.length > RulesBannerView.collapsedCount) ...<Widget>[
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('rules-banner-more'),
                    variant: HButtonVariant.ghost,
                    onPressed: () => setState(() => _expanded = !_expanded),
                    child: Text(
                      _expanded
                          ? l10n.rulesBannerFewer
                          : l10n.rulesBannerMore(hidden),
                    ),
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
