/// The Rules section: the order of the rules, and the form that changes one.
///
/// The order on the screen is the order the daemon evaluates in, and that is
/// the whole point of the screen: the first rule that matches wins, so the
/// chain is read from top to bottom like a text and the last link says what
/// happens when nothing matched. Everything the screen knows comes from
/// `Rules`; it never evaluates a rule itself (ADR-018, `docs/UX.md` 1.1).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'providers/editor.dart';
import 'providers/rules.dart';
import 'severity.dart';
import 'widgets/arrive.dart';
import 'widgets/rule_editor.dart';
import 'widgets/rules_banner.dart';
import 'widgets/rules_list.dart';

/// Below this window width the editor is a sheet over the list instead of a
/// pane beside it: two panes under 900 px leave the sentence of a rule too
/// little room to be a sentence.
const double rulesSheetBelow = 900;

/// How much of the width the list takes when both panes are shown.
const double rulesListFraction = 0.4;

/// How much of the width the sheet takes when the editor cannot stand beside
/// the list. Not all of it: the chain stays visible behind its own editor.
const double rulesSheetFraction = 0.7;

/// How many rows the skeleton of the first load draws.
const int rulesSkeletonRows = 6;

/// The Rules section.
class RulesScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const RulesScreen({super.key});

  @override
  ConsumerState<RulesScreen> createState() => _RulesScreenState();
}

class _RulesScreenState extends ConsumerState<RulesScreen> {
  bool _visible = false;

  /// True while the shell actually paints this section.
  ///
  /// The shell keeps every section built inside an `IndexedStack`, which wraps
  /// each child in a `Visibility`. Reading it here keeps the check inside this
  /// feature -- no feature imports another one (ARCHITECTURE 5). Without such
  /// an ancestor, in a test that mounts the screen alone, the section counts
  /// as visible.
  bool _isVisible(BuildContext context) => Visibility.of(context);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool visible = _isVisible(context);
    if (visible && !_visible) {
      // A rule may have been created next door while this section was hidden;
      // every `Rules` answer carries the whole set, so one call catches up
      // with all of it.
      WidgetsBinding.instance.addPostFrameCallback(
        (Duration _) => ref.read(rulesProvider.notifier).refresh(),
      );
    }
    _visible = visible;

    final AsyncValue<RuleSet> set = ref.watch(rulesProvider);
    return ColoredBox(
      color: tokens.colors.bg0,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const _Header(),
          const HHairline(),
          const RulesBannerView(),
          const RuleUndoStrip(),
          Expanded(
            child: switch (set) {
              AsyncError(:final Object error) => _CannotRead(error: error),
              _ => HWait(
                loading: set.isLoading && !set.hasValue,
                skeleton: Padding(
                  padding: EdgeInsets.all(tokens.spacing.x3),
                  child: HSkeleton(
                    rows: rulesSkeletonRows,
                    rowHeight: tokens.sizes.row,
                  ),
                ),
                child: const _Body(),
              ),
            },
          ),
        ],
      ),
    );
  }
}

/// List and editor, side by side or one over the other.
class _Body extends ConsumerWidget {
  const _Body();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final bool open = ref.watch(
      ruleEditorProvider.select((RuleEditorState state) => state.isOpen),
    );
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        if (constraints.maxWidth >= rulesSheetBelow) {
          return Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              SizedBox(
                width: constraints.maxWidth * rulesListFraction,
                child: const RulesList(),
              ),
              const HHairline(vertical: true),
              const Expanded(child: RuleEditor()),
            ],
          );
        }
        return Stack(
          children: <Widget>[
            const Positioned.fill(child: RulesList()),
            if (open)
              Positioned(
                top: 0,
                right: 0,
                bottom: 0,
                // The sheet comes from the edge it hangs on, and the list
                // behind it stays where it is (`docs/UX.md` 2.2).
                child: ArriveIn(
                  fromRight: true,
                  child: HSheet(
                    title: Text(l10n.rulesEditorTitle),
                    closeSemanticsLabel: l10n.rulesClose,
                    onClose: ref.read(ruleEditorProvider.notifier).close,
                    width: constraints.maxWidth * rulesSheetFraction,
                    child: const RuleEditor(),
                  ),
                ),
              ),
          ],
        );
      },
    );
  }
}

/// Tabs, filter and the two things somebody can start here.
class _Header extends ConsumerWidget {
  const _Header();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final RuleSet set = ref.watch(rulesProvider).value ?? RuleSet.empty;
    final RuleTab tab = ref.watch(ruleTabSelectionProvider);
    final bool editorOpen = ref.watch(
      ruleEditorProvider.select((RuleEditorState state) => state.isOpen),
    );
    int saved = 0;
    int temporary = 0;
    for (final Rule rule in set.rules) {
      if (tabOf(rule) == RuleTab.saved) {
        saved++;
      } else {
        temporary++;
      }
    }
    // The one filled control of the screen, and only while there is nothing
    // to read: as soon as the list has rules, the fill belongs to `Save` in
    // the editor (`docs/UX.md` 3.1).
    final bool emptyScreen = set.rules.isEmpty && !editorOpen;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Row(
        children: <Widget>[
          _Tabs(tab: tab, saved: saved, temporary: temporary),
          SizedBox(width: tokens.spacing.x3),
          const Expanded(child: _Filter()),
          SizedBox(width: tokens.spacing.x3),
          HButton(
            key: const Key('rules-reload'),
            variant: HButtonVariant.ghost,
            onPressed: ref.read(rulesProvider.notifier).reload,
            child: Text(l10n.rulesReload),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('rules-new'),
            variant: emptyScreen
                ? HButtonVariant.primary
                : HButtonVariant.ghost,
            leading: HGlyphIcon(
              HGlyph.plus,
              size: 12,
              color: emptyScreen ? tokens.colors.onAccent : tokens.colors.fg1,
            ),
            onPressed: ref.read(ruleEditorProvider.notifier).openNew,
            child: Text(l10n.rulesNewRule),
          ),
        ],
      ),
    );
  }
}

class _Tabs extends ConsumerWidget {
  const _Tabs({
    required this.tab,
    required this.saved,
    required this.temporary,
  });

  final RuleTab tab;
  final int saved;
  final int temporary;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    return HSegmented<RuleTab>(
      selected: tab,
      onSelect: ref.read(ruleTabSelectionProvider.notifier).select,
      options: <HSegmentOption<RuleTab>>[
        HSegmentOption<RuleTab>(
          value: RuleTab.saved,
          label: l10n.rulesTabSaved(saved),
        ),
        HSegmentOption<RuleTab>(
          value: RuleTab.temporary,
          label: l10n.rulesTabTemporary(temporary),
        ),
      ],
    );
  }
}

class _Filter extends ConsumerStatefulWidget {
  const _Filter();

  @override
  ConsumerState<_Filter> createState() => _FilterState();
}

class _FilterState extends ConsumerState<_Filter> {
  final TextEditingController _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    // The field owns the text; the provider owns the filter. They are synced
    // in one direction only, so nothing rewrites what somebody is typing.
    ref.listen<String>(ruleQueryProvider, (String? previous, String next) {
      if (next != _controller.text) {
        _controller.text = next;
      }
    });
    return HTextField(
      key: const Key('rules-filter'),
      controller: _controller,
      mono: false,
      semanticsLabel: l10n.rulesSearchLabel,
      hint: l10n.rulesSearchHint,
      onChanged: ref.read(ruleQueryProvider.notifier).set,
    );
  }
}

/// What shows when the rule set cannot be read at all.
///
/// Full width only because there is nothing else to show: with a list on
/// screen the same failure would be a banner over it (`docs/UX.md` 4.4).
class _CannotRead extends ConsumerWidget {
  const _CannotRead({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic diagnostic = error is DaemonException
        ? (error as DaemonException).diagnostic
        : Diagnostic(
            code: DiagnosticCodes.rulesRequestInvalid,
            severity: Severity.error,
            why: '$error',
          );
    return Center(
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x6),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            HDiagnosticCard(
              code: diagnostic.code,
              severityLabel: ruleSeverityLabel(l10n, diagnostic.severity),
              color: ruleSeverityColor(tokens, diagnostic.severity),
              title: l10n.rulesCannotReadTitle,
              why: diagnostic.why,
              docsUrl: diagnostic.docsUrl,
            ),
            SizedBox(height: tokens.spacing.x3),
            HButton(
              key: const Key('rules-retry'),
              variant: HButtonVariant.primary,
              size: HButtonSize.md,
              onPressed: ref.read(rulesProvider.notifier).refresh,
              child: Text(l10n.rulesReload),
            ),
          ],
        ),
      ),
    );
  }
}
