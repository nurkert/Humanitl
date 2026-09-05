/// The filter bar above the table: one expression field, six quick filters
/// and, when the daemon refuses the expression, its answer underneath.
///
/// The field is the one place on this screen that carries the accent
/// (`docs/UX.md` 3.1): the history has no filled control, because the screen
/// takes no decision.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_view.dart';
import 'providers/history_page.dart';
import 'providers/history_query.dart';

/// The filter bar.
class HistoryFilterBar extends ConsumerStatefulWidget {
  /// Creates the bar. [focusNode] is held by the screen so that `/` can put
  /// the caret in the field.
  const HistoryFilterBar({required this.focusNode, this.trailing, super.key});

  /// The focus of the expression field.
  final FocusNode focusNode;

  /// A control at the right end of the chip row; the export menu.
  final Widget? trailing;

  @override
  ConsumerState<HistoryFilterBar> createState() => _HistoryFilterBarState();
}

class _HistoryFilterBarState extends ConsumerState<HistoryFilterBar> {
  final TextEditingController _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    _controller.text = ref.read(historyQueryProvider).filter;
    widget.focusNode.addListener(_focusChanged);
  }

  @override
  void dispose() {
    widget.focusNode.removeListener(_focusChanged);
    _controller.dispose();
    super.dispose();
  }

  void _focusChanged() => setState(() {});

  void _submit(String value) =>
      ref.read(historyQueryProvider.notifier).submit(value);

  void _reset() {
    _controller.clear();
    ref.read(historyQueryProvider.notifier).reset();
  }

  /// Keeps the field in step with the query when a chip changed it.
  void _syncField(HistoryQuery query) {
    if (_controller.text != query.filter) {
      _controller.value = TextEditingValue(
        text: query.filter,
        selection: TextSelection.collapsed(offset: query.filter.length),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HistoryQuery query = ref.watch(historyQueryProvider);
    final Diagnostic? failure = ref.watch(
      historyPageProvider.select((HistoryPageState page) => page.failure),
    );
    final bool filterRefused =
        failure != null && failure.code == historyFilterInvalidCode;
    _syncField(query);
    final bool focused = widget.focusNode.hasFocus;
    return Padding(
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        tokens.spacing.x2,
        tokens.spacing.x3,
        tokens.spacing.x2,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Semantics(
            textField: true,
            label: l10n.historyFilterLabel,
            child: DecoratedBox(
              decoration: BoxDecoration(
                color: tokens.colors.bg2,
                borderRadius: HRadius.controlRadius,
                border: Border.all(
                  color: filterRefused
                      ? tokens.state.error
                      : focused
                      ? tokens.colors.accent
                      : tokens.colors.line,
                  width: focused ? 2 : HSize.hairline,
                ),
              ),
              child: Padding(
                padding: EdgeInsets.symmetric(
                  horizontal: tokens.spacing.x2,
                  vertical: tokens.spacing.x2,
                ),
                child: Stack(
                  children: <Widget>[
                    if (_controller.text.isEmpty)
                      IgnorePointer(
                        child: Text(
                          key: const Key('history-filter-hint'),
                          l10n.historyFilterHint,
                          // The placeholder teaches the grammar, so it is
                          // read, not decoration (`docs/UX.md` 6).
                          style: tokens.typography.mono13.tinted(
                            tokens.colors.fg1,
                          ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    EditableText(
                      key: const Key('history-filter-input'),
                      controller: _controller,
                      focusNode: widget.focusNode,
                      style: tokens.typography.mono13.tinted(tokens.colors.fg0),
                      cursorColor: tokens.colors.accent,
                      backgroundCursorColor: tokens.colors.bg3,
                      selectionColor: HColorDerivation.fade(
                        tokens.colors.accent,
                        0.35,
                      ),
                      onChanged: (String _) => setState(() {}),
                      onSubmitted: _submit,
                    ),
                  ],
                ),
              ),
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Row(
            children: <Widget>[
              Expanded(
                child: Wrap(
                  spacing: tokens.spacing.x2,
                  runSpacing: tokens.spacing.x1,
                  children: <Widget>[
                    for (final HistoryChip chip in HistoryChip.values)
                      _ChipButton(
                        chip: chip,
                        active: query.has(chip),
                        label: _chipLabel(l10n, chip, query),
                        onPressed: () => ref
                            .read(historyQueryProvider.notifier)
                            .toggle(chip),
                      ),
                  ],
                ),
              ),
              if (!query.isUnfiltered) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HButton(
                  variant: HButtonVariant.ghost,
                  size: HButtonSize.sm,
                  onPressed: _reset,
                  child: Text(l10n.historyFilterReset),
                ),
              ],
              if (widget.trailing != null) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                widget.trailing!,
              ],
            ],
          ),
          if (failure != null && filterRefused) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            HDiagnosticCard(
              code: failure.code,
              severityLabel: historySeverityLabel(l10n, failure.severity),
              color: historySeverityColor(tokens, failure.severity),
              title: l10n.historyFilterInvalidTitle,
              why: failure.why,
              docsUrl: failure.docsUrl,
              width: double.infinity,
            ),
          ],
        ],
      ),
    );
  }

  String _chipLabel(
    AppLocalizations l10n,
    HistoryChip chip,
    HistoryQuery query,
  ) => switch (chip) {
    HistoryChip.held => l10n.historyChipHeld,
    HistoryChip.blocked => l10n.historyChipBlocked,
    HistoryChip.findings => l10n.historyChipFindings,
    HistoryChip.edited => l10n.historyChipEdited,
    HistoryChip.meta => l10n.historyChipMeta,
    HistoryChip.passthrough =>
      query.includePassthrough
          ? l10n.historyChipPassthroughShown
          : l10n.historyChipPassthroughHidden,
  };
}

/// One quick filter.
///
/// An active chip is the tinted `secondary` variant, an inactive one is
/// `ghost`: neither is filled, so the accent stays with the field
/// (`docs/UX.md` 3.1). Both are buttons and therefore focusable, which is
/// what keyboard parity asks for (`docs/UX.md` 5.1).
class _ChipButton extends StatelessWidget {
  const _ChipButton({
    required this.chip,
    required this.active,
    required this.label,
    required this.onPressed,
  });

  final HistoryChip chip;
  final bool active;
  final String label;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      toggled: active,
      child: HButton(
        key: Key('history-chip-${chip.name}'),
        variant: active ? HButtonVariant.secondary : HButtonVariant.ghost,
        size: HButtonSize.sm,
        onPressed: onPressed,
        child: Text(label),
      ),
    );
  }
}

/// The label of [severity] in the person's language.
///
/// The same table as the action bar's; it is repeated rather than imported so
/// that this screen does not depend on a file of another feature. It belongs
/// next to `HDiagnosticCard` in `core/ui` (handoff).
String historySeverityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };

/// The hue of [severity]. Never the blocked red: red means blocked.
///
/// The text-capable readings, not the area ones: `HDiagnosticCard` hands this
/// colour to `HBadge`, which draws the code and the severity as *text* on a
/// tint of it, and the area palette is clamped to 3:1 (`docs/UX.md` 6).
Color historySeverityColor(HTokens tokens, Severity severity) =>
    switch (severity) {
      Severity.info => tokens.colors.accentText,
      Severity.warning => tokens.stateTextColor(HFlowState.held),
      Severity.error ||
      Severity.blocking => tokens.stateTextColor(HFlowState.error),
    };
