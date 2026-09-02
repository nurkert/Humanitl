/// Design tokens and the thin wrapper layer around the component library.
///
/// No feature imports the component library directly; everything goes through
/// this package so a later swap touches one place only. See HUM-008.
///
/// The layer has two halves. `HTokens` is data — the colours, type scale,
/// spacing, radii, durations and curves of the Airlock design direction
/// (BACKLOG.md 5), in a dark and a light instance, published to the widget tree
/// by `HTheme` and read back with `HTheme.of(context)`. The `H*` widgets are
/// the vocabulary a screen is allowed to use; they are built on
/// `package:flutter/widgets.dart` today and can be re-pointed at a component
/// library without any feature noticing.
library;

export 'src/gallery/gallery_page.dart';
export 'src/theme/h_theme.dart';
export 'src/tokens/colors.dart';
export 'src/tokens/flow_state.dart';
export 'src/tokens/motion.dart';
export 'src/tokens/spacing.dart';
export 'src/tokens/tokens.dart';
export 'src/tokens/typography.dart';
export 'src/widgets/h_badge.dart';
export 'src/widgets/h_button.dart';
export 'src/widgets/h_glyph.dart';
export 'src/widgets/h_hairline.dart';
export 'src/widgets/h_icon_button.dart';
export 'src/widgets/h_method_badge.dart';
export 'src/widgets/h_modal.dart';
export 'src/widgets/h_panel.dart';
export 'src/widgets/h_pill.dart';
export 'src/widgets/h_row.dart';
export 'src/widgets/h_sheet.dart';
export 'src/widgets/h_state_glyph.dart';
