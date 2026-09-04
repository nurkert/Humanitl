/// The chain: the rules of the current tab, read from top to bottom, ending
/// in what happens when none of them matches.
///
/// This is the one place the screen is allowed to be beautiful (`docs/UX.md`
/// 1.1): the order *is* the meaning, so it is shown as an order -- numbered,
/// draggable, unbroken -- instead of being explained in a sentence nobody
/// reads twice. What comes after the last rule is part of the chain and
/// stands there: the contract asks when nothing matched (ADR-007).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/announce.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/editor.dart';
import '../providers/rules.dart';
import 'draw_in.dart';
import 'rule_row.dart';

/// The list pane.
class RulesList extends ConsumerStatefulWidget {
  /// Creates the list.
  const RulesList({super.key});

  @override
  ConsumerState<RulesList> createState() => _RulesListState();
}

class _RulesListState extends ConsumerState<RulesList> {
  /// Which rules this pane has already shown, per tab.
  ///
  /// A rule that is not in the set of its tab draws itself in once. A tab
  /// that has never been shown records its rules without animating any of
  /// them: a list that arrives is not a rule that was created, and switching
  /// tabs is navigation, which takes one frame and no motion (`docs/UX.md`
  /// 2.2 and 2.11).
  final Map<RuleTab, Set<RuleId>> _known = <RuleTab, Set<RuleId>>{};

  late final Map<ShortcutActivator, Intent> _shortcuts = ruleListShortcuts();

  /// Der Filter, unter dem zuletzt eine Bewegung abgelehnt wurde, oder null.
  ///
  /// Der Grund steht über der Liste, solange derselbe Filter steht; ein neuer
  /// Filter ist eine neue Lage, und die Ablehnung von vorhin gehört nicht mehr
  /// dazu. Kein Timer: nichts verschwindet unter dem lesenden Auge
  /// (`docs/UX.md` 2.8).
  String? _refusedUnder;

  /// Sagt, warum die Taste die Regel gerade nicht bewegt hat.
  ///
  /// Leise: der Grund steht daneben und wird höflich angesagt, es blitzt
  /// nichts und es rüttelt nichts (`docs/UX.md` 5.3 und 6).
  void _refuseMove(String query) {
    final String reason = context.l10n.rulesMoveRefusedFiltered;
    if (_refusedUnder != query) {
      setState(() => _refusedUnder = query);
    }
    announcePolitely(context, reason);
  }

  void _open(Rule rule) => ref.read(ruleEditorProvider.notifier).edit(rule);

  Future<void> _delete(Rule rule) async {
    final Diagnostic? failed = await ref
        .read(rulesProvider.notifier)
        .remove(rule);
    if (failed != null) {
      ref.read(rulesBannerProvider.notifier).showOne(failed);
    }
  }

  Future<void> _reorder(int from, int to) async {
    final RuleSet? set = ref.read(rulesProvider).value;
    if (set == null) {
      return;
    }
    final RuleTab tab = ref.read(ruleTabSelectionProvider);
    final Diagnostic? failed = await ref
        .read(rulesProvider.notifier)
        .reorder(chainOrderAfterMove(set, tab, from: from, to: to));
    if (failed != null) {
      ref.read(rulesBannerProvider.notifier).showOne(failed);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final RuleChain chain = ref.watch(visibleRulesProvider);
    final String query = ref.watch(ruleQueryProvider);
    final RuleTab tab = ref.watch(ruleTabSelectionProvider);
    final RuleId? open = ref.watch(
      ruleEditorProvider.select((RuleEditorState state) => state.editing),
    );
    // A filtered list cannot be dragged: the row above the gap is not the
    // rule the order would put there, and an order that does something else
    // than it shows is worse than no dragging (CONVENTIONS 4.13).
    final bool canReorder = query.trim().isEmpty;

    final RuleSet? set = ref.watch(rulesProvider).value;
    if (set == null) {
      // Nothing is known yet, so nothing is new; the screen above shows the
      // skeleton of what is coming.
      return const SizedBox.shrink();
    }
    // What this tab holds, before the filter: a rule that a filter hides and
    // shows again was not created in between, and it must not draw itself in
    // as if it had been (`docs/UX.md` 2.1).
    final Set<RuleId> ids = <RuleId>{
      for (final Rule rule in set.rules)
        if (tabOf(rule) == tab && rule.id != null) rule.id!,
    };
    final Set<RuleId>? seen = _known[tab];
    final Set<RuleId> fresh = seen == null
        ? const <RuleId>{}
        : ids.difference(seen);
    _known[tab] = ids;

    if (chain.isEmpty) {
      return _EmptyChain(tab: tab);
    }
    if (chain.matched == 0) {
      return _FilterFoundNothing(query: query, total: chain.total);
    }

    return Shortcuts(
      shortcuts: _shortcuts,
      child: CustomScrollView(
        slivers: <Widget>[
          if (!canReorder && _refusedUnder == query)
            SliverToBoxAdapter(
              child: _ChainNote(text: l10n.rulesMoveRefusedFiltered),
            ),
          if (tab == RuleTab.saved && chain.otherTab > 0)
            SliverToBoxAdapter(
              child: _ChainNote(
                text: l10n.rulesChainSessionFirst(chain.otherTab),
              ),
            ),
          SliverReorderableList(
            itemCount: chain.rules.length,
            onReorderItem: _reorder,
            proxyDecorator: _carried,
            itemBuilder: (BuildContext context, int index) {
              final Rule rule = chain.rules[index];
              final RuleId? id = rule.id;
              return DrawIn(
                key: ValueKey<String>(id?.value ?? 'draft-$index'),
                animate: id != null && fresh.contains(id),
                child: RuleRow(
                  rule: rule,
                  index: index,
                  // A position counts inside its group, and so does the
                  // count beside it: the rules of the person are one group,
                  // the bundled ones another (CONVENTIONS 4.5).
                  total: chain.ownTotal,
                  selected: id != null && id == open,
                  onOpen: () => _open(rule),
                  onDelete: () => _delete(rule),
                  onMove: canReorder
                      ? (int delta) => _reorder(index, index + delta)
                      : null,
                  onMoveRefused: () => _refuseMove(query),
                  dragHandle: canReorder
                      ? _DragHandle(
                          key: ValueKey<String>('rule-grip-$index'),
                          index: index,
                          position: index + 1,
                        )
                      : null,
                ),
              );
            },
          ),
          if (chain.bundled.isNotEmpty) ...<Widget>[
            const SliverToBoxAdapter(child: _BundledHeader()),
            SliverList.builder(
              itemCount: chain.bundled.length,
              itemBuilder: (BuildContext context, int index) {
                final Rule rule = chain.bundled[index];
                return RuleRow(
                  key: ValueKey<String>(rule.id?.value ?? 'bundled-$index'),
                  rule: rule,
                  index: index,
                  total: chain.bundledTotal,
                  selected: rule.id != null && rule.id == open,
                  onOpen: () => _open(rule),
                  // Der Grund steht bei einer mitgelieferten Regel schon
                  // dauerhaft über ihrem Block; die Taste sagt ihn noch
                  // einmal, statt zu schweigen (`docs/UX.md` 5.3).
                  onMoveRefused: () =>
                      announcePolitely(context, l10n.rulesBundledWhy),
                );
              },
            ),
          ],
          SliverToBoxAdapter(
            child: tab == RuleTab.saved
                ? const _ChainEnd()
                : _ChainNote(text: l10n.rulesChainThenSaved(chain.otherTab)),
          ),
          SliverToBoxAdapter(child: SizedBox(height: tokens.spacing.x4)),
        ],
      ),
    );
  }

  /// The row under the pointer while it is dragged.
  ///
  /// No shadow, no scale, no lift: direct manipulation follows the pointer one
  /// to one, and only the gap that was freed animates (`docs/UX.md` 2.9). The
  /// carried row gets the fill of a selected row so that it can be told from
  /// the ones it passes.
  Widget _carried(Widget child, int index, Animation<double> animation) {
    final HTokens tokens = HTheme.of(context);
    return ColoredBox(color: tokens.colors.bg3, child: child);
  }
}

/// The grip. It is a hit target of its own and starts the drag; the keyboard
/// path is `Alt` plus an arrow key on the row itself (`docs/UX.md` 5.1).
class _DragHandle extends StatelessWidget {
  const _DragHandle({required this.index, required this.position, super.key});

  final int index;
  final int position;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return ReorderableDragStartListener(
      index: index,
      child: MouseRegion(
        cursor: SystemMouseCursors.grab,
        // The key stands on the control that performs it, so that somebody
        // who only ever uses the pointer can find it (`docs/UX.md` 5.1).
        child: HoverLabel(
          label: l10n.rulesDragHint,
          child: SizedBox.square(
            dimension: tokens.sizes.hitMin,
            child: Center(
              child: HGlyphIcon(
                HGlyph.grip,
                size: 14,
                color: tokens.colors.fg1,
                semanticsLabel: l10n.rulesDragHandle(position),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// A quiet line that says where the chain goes on.
class _ChainNote extends StatelessWidget {
  const _ChainNote({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        tokens.spacing.x2,
        tokens.spacing.x3,
        tokens.spacing.x2,
      ),
      child: Text(
        text,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
    );
  }
}

/// The separator above the bundled block, with the reason it cannot be
/// touched and the way around it.
class _BundledHeader extends StatelessWidget {
  const _BundledHeader();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          const HHairline(),
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3,
              tokens.spacing.x2,
              tokens.spacing.x3,
              tokens.spacing.x1,
            ),
            child: Row(
              children: <Widget>[
                HGlyphIcon(HGlyph.lock, size: 12, color: tokens.colors.fg1),
                SizedBox(width: tokens.spacing.x2),
                Text(
                  l10n.rulesBundledTitle,
                  style: tokens.typography.ui12.medium.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3,
              0,
              tokens.spacing.x3,
              tokens.spacing.x2,
            ),
            child: Text(
              l10n.rulesBundledWhy,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        ],
      ),
    );
  }
}

/// The last link of the chain: what happens when no rule matched.
///
/// Not a rule, and drawn so that nobody mistakes it for one -- no number, no
/// handle, a hairline rail. It stands here because "first match wins" is only
/// half an answer without it (ADR-007).
class _ChainEnd extends StatelessWidget {
  const _ChainEnd();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          const HHairline(),
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3 + tokens.sizes.hitMin,
              tokens.spacing.x2,
              tokens.spacing.x3,
              tokens.spacing.x1,
            ),
            child: Row(
              children: <Widget>[
                HGlyphIcon(
                  HGlyph.hourglass,
                  size: 14,
                  color: tokens.colors.fg1,
                ),
                SizedBox(width: tokens.spacing.x2),
                Text(
                  l10n.rulesChainDefault,
                  style: tokens.typography.ui13.medium.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ],
            ),
          ),
          Padding(
            padding: EdgeInsets.fromLTRB(
              tokens.spacing.x3 + tokens.sizes.hitMin,
              0,
              tokens.spacing.x3,
              tokens.spacing.x2,
            ),
            child: Text(
              l10n.rulesChainDefaultWhy,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
        ],
      ),
    );
  }
}

/// What a tab without rules says: the next event, never the absence
/// (`docs/UX.md` 4.1).
///
/// It carries no button of its own. The one way to create a rule stands in
/// the header and is the filled control of the screen for exactly as long as
/// the list is empty; a second one here would be the same offer twice
/// (`docs/UX.md` 3.1).
class _EmptyChain extends StatelessWidget {
  const _EmptyChain({required this.tab});

  final RuleTab tab;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              tab == RuleTab.saved
                  ? l10n.rulesEmptySavedTitle
                  : l10n.rulesEmptyTemporaryTitle,
              textAlign: TextAlign.center,
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              tab == RuleTab.saved
                  ? l10n.rulesEmptySavedHint
                  : l10n.rulesEmptyTemporaryHint,
              textAlign: TextAlign.center,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ],
        ),
      ),
    );
  }
}

/// What a filter that matches nothing says: the filter, the count and the way
/// back, and the way back is a control (`docs/UX.md` 4.1).
class _FilterFoundNothing extends ConsumerWidget {
  const _FilterFoundNothing({required this.query, required this.total});

  final String query;
  final int total;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Text(
              l10n.rulesFilterEmpty(query, total),
              textAlign: TextAlign.center,
              style: tokens.typography.ui13.tinted(tokens.colors.fg0),
            ),
            SizedBox(height: tokens.spacing.x3),
            HButton(
              key: const Key('rules-filter-reset'),
              variant: HButtonVariant.ghost,
              onPressed: ref.read(ruleQueryProvider.notifier).clear,
              child: Text(l10n.rulesFilterReset),
            ),
          ],
        ),
      ),
    );
  }
}
