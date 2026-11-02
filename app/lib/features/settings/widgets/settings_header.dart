/// The head row of the settings overlay: title, search field and close
/// button (HUM-404).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'setting_row.dart';

/// Title over the column of categories, the search field on the text edge of
/// the rows, the close button on the edge of their controls.
///
/// As high as a pane header, but none: the overlay does not stand in the
/// grid of the shell (HUM-286), so it is no `HPaneHeader`.
class SettingsHeader extends StatelessWidget {
  /// Creates the head; [textEdge] is where the page begins, [pageMaxWidth]
  /// the widest the page gets.
  const SettingsHeader({
    required this.textEdge,
    required this.pageMaxWidth,
    required this.searchController,
    required this.searchFocus,
    required this.onSubmitSearch,
    required this.onClose,
    super.key,
  });

  /// The left edge of the page, from the left edge of the panel.
  final double textEdge;

  /// The widest the page gets.
  final double pageMaxWidth;

  /// The text of the search.
  final TextEditingController searchController;

  /// The node of the search field.
  final FocusNode searchFocus;

  /// `Enter` in the search field.
  final VoidCallback onSubmitSearch;

  /// The close button.
  final VoidCallback onClose;

  /// The widest the search field gets.
  static const double searchWidth = 360;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SizedBox(
      height: HPaneHeader.rowHeight(context) - HSize.hairline,
      child: Padding(
        padding: EdgeInsetsDirectional.only(end: tokens.spacing.x4),
        child: Row(
          children: <Widget>[
            SizedBox(
              width: textEdge,
              child: Padding(
                padding: const EdgeInsetsDirectional.only(start: HSpace.x3),
                child: Semantics(
                  header: true,
                  child: Text(
                    l10n.settingsTitle,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui16.semibold.tinted(
                      tokens.colors.fg0,
                    ),
                  ),
                ),
              ),
            ),
            // Suchfeld und Schließen über der Seite: Das Feld beginnt auf der
            // Textkante der Zeilen, das Kreuz endet auf der Kante ihrer
            // Controls.
            Expanded(
              child: Align(
                alignment: AlignmentDirectional.centerStart,
                child: ConstrainedBox(
                  constraints: BoxConstraints(maxWidth: pageMaxWidth),
                  child: Padding(
                    padding: const EdgeInsets.symmetric(
                      horizontal: SettingRowGrid.inset,
                    ),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: <Widget>[
                        Flexible(
                          child: Container(
                            margin: EdgeInsetsDirectional.only(
                              end: tokens.spacing.x3,
                            ),
                            constraints: const BoxConstraints(
                              maxWidth: searchWidth,
                            ),
                            child: FocusTraversalOrder(
                              order: const NumericFocusOrder(1),
                              child: _SearchField(
                                controller: searchController,
                                focusNode: searchFocus,
                                onSubmitted: onSubmitSearch,
                              ),
                            ),
                          ),
                        ),
                        FocusTraversalOrder(
                          order: const NumericFocusOrder(4),
                          child: HIconButton(
                            key: const Key('settings-close'),
                            glyph: HGlyph.close,
                            onPressed: onClose,
                            semanticsLabel: l10n.settingsClose,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// The search field with its glyph in front.
class _SearchField extends StatelessWidget {
  const _SearchField({
    required this.controller,
    required this.focusNode,
    required this.onSubmitted,
  });

  final TextEditingController controller;
  final FocusNode focusNode;
  final VoidCallback onSubmitted;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      children: <Widget>[
        HGlyphIcon(HGlyph.search, size: 14, color: tokens.colors.fg2),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: HTextField(
            key: const Key('settings-search'),
            controller: controller,
            focusNode: focusNode,
            mono: false,
            semanticsLabel: l10n.settingsSearchHint,
            hint: l10n.settingsSearchHint,
            onSubmitted: (String _) => onSubmitted(),
          ),
        ),
      ],
    );
  }
}
