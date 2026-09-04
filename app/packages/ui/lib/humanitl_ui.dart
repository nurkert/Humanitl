/// Design tokens and the widget vocabulary of the application.
///
/// No feature builds its own look; everything goes through this package, so a
/// change lands in one place. ADR-0009 decided on 2026-09-04 that no foreign
/// component library sits behind this seam: the widgets are built on
/// `package:flutter/widgets.dart`. The seam stays anyway, so the decision
/// remains reversible. See HUM-008.
///
/// The layer has two halves. `HTokens` is data — the colours, type scale,
/// spacing, radii, durations and curves of the Airlock design direction
/// (BACKLOG.md 5), in a dark and a light instance, published to the widget tree
/// by `HTheme` and read back with `HTheme.of(context)`. The `H*` widgets are
/// the vocabulary a screen is allowed to use; they are built on
/// `package:flutter/widgets.dart` and could be re-pointed at a component
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
export 'src/widgets/h_animated_fill.dart';
export 'src/widgets/h_badge.dart';
export 'src/widgets/h_button.dart';
export 'src/widgets/h_checkbox.dart';
export 'src/widgets/h_focus_ring.dart';
export 'src/widgets/h_glyph.dart';
export 'src/widgets/h_hairline.dart';
export 'src/widgets/h_icon_button.dart';
export 'src/widgets/h_method_badge.dart';
export 'src/widgets/h_modal.dart';
export 'src/widgets/h_panel.dart';
export 'src/widgets/h_pill.dart';
export 'src/widgets/h_row.dart';
export 'src/widgets/h_segmented.dart';
export 'src/widgets/h_sheet.dart';
export 'src/widgets/h_skeleton.dart';
export 'src/widgets/h_state_glyph.dart';
export 'src/widgets/h_text_field.dart';
