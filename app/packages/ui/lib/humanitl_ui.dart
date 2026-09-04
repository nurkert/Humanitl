/// Design tokens and the widget vocabulary of the application.
///
/// No feature builds its own look; everything goes through this package, so a
/// change lands in one place. Hinter dieser Naht steht seit der Revision von
/// ADR-0009 am 2026-09-04 `shadcn_flutter`, exakt auf 0.0.54 gepinnt. **Nur
/// dieses Paket darf die Bibliothek importieren**; ein Import in einem Feature
/// oder in `app/lib/core` ist ein Architekturverstoß und wird von
/// `tools/check-deps.sh` beanstandet. Ein Bildschirm sieht weiterhin
/// `HButton`, `HRow`, `HModal` und die Token — was sich geändert hat, ist
/// ausschließlich, worauf diese Widgets stehen. See HUM-008.
///
/// The layer has two halves. `HTokens` is data — the colours, type scale,
/// spacing, radii, durations and curves of the Airlock design direction
/// (BACKLOG.md 5), in a dark and a light instance, published to the widget tree
/// by `HTheme` and read back with `HTheme.of(context)`. The `H*` widgets are
/// the vocabulary a screen is allowed to use.
///
/// Die Naht ist auch hier zu: `src/theme/shadcn_theme.dart` und
/// `src/widgets/h_control.dart` werden **nicht** exportiert. Beide führen
/// Typen der Bibliothek in ihrer Signatur, und was hier hinausgeht, könnte ein
/// Feature benutzen, ohne je `package:shadcn_flutter` zu schreiben — genau der
/// Import, nach dem `tools/check-deps.sh` sucht. Die Tests dieses Pakets
/// greifen auf beide über ihren Pfad zu.
///
/// **Die Richtung zwischen den beiden Themen ist eine Entscheidung.** Nicht
/// `HTokens` wird aus dem `ColorScheme` der Bibliothek abgeleitet, sondern
/// umgekehrt: `HTheme` baut aus den Token deren `ThemeData` und veröffentlicht
/// es mitsamt den Komponententhemen für Button, Eingabefeld, Kästchen,
/// Haarlinie, Karte, Badge und Fokusring (`HShadcnTheme`). Damit malt jede
/// Komponente der Bibliothek in unserer Palette, und es gibt keine zweite.
/// Die Begründung steht in `src/theme/shadcn_theme.dart`.
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
