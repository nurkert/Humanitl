import 'package:flutter/widgets.dart';

/// The type scale of the Airlock design direction.
///
/// Two families, six UI sizes, four mono sizes, three weights. Line heights are
/// absolute in BACKLOG.md (11/16, 12/16, 13/20, 14/22, 16/24, 20/28) and are
/// expressed here as the multiplier Flutter expects.
///
/// The font binaries are **not** vendored. The families are referenced by name
/// with a fallback stack, so the app looks right where Inter and JetBrains Mono
/// are installed and stays readable where they are not.
abstract final class HType {
  /// UI family name.
  static const String uiFamily = 'Inter';

  /// Fallbacks used when [uiFamily] is not installed.
  static const List<String> uiFallback = <String>[
    'Inter Variable',
    'Noto Sans',
    'DejaVu Sans',
    'Liberation Sans',
    'Roboto',
    'sans-serif',
  ];

  /// Monospace family name.
  static const String monoFamily = 'JetBrains Mono';

  /// Fallbacks used when [monoFamily] is not installed.
  static const List<String> monoFallback = <String>[
    'JetBrains Mono NL',
    'Fira Code',
    'DejaVu Sans Mono',
    'Liberation Mono',
    'Noto Sans Mono',
    'monospace',
  ];

  /// Regular weight (400). The scale has no 700.
  static const FontWeight regular = FontWeight.w400;

  /// Medium weight (500).
  static const FontWeight medium = FontWeight.w500;

  /// Semibold weight (600), the heaviest weight in the system.
  static const FontWeight semibold = FontWeight.w600;

  /// Features of the UI family: the single-storey `cv11` and tabular figures.
  static const List<FontFeature> uiFeatures = <FontFeature>[
    FontFeature('cv11'),
    FontFeature.tabularFigures(),
  ];

  /// Features of the monospace family: ligatures off, tabular figures.
  static const List<FontFeature> monoFeatures = <FontFeature>[
    FontFeature.disable('liga'),
    FontFeature.tabularFigures(),
  ];

  static const TextStyle _ui = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
  );

  static const TextStyle _mono = TextStyle(
    fontFamily: monoFamily,
    fontFamilyFallback: monoFallback,
    fontWeight: regular,
    fontFeatures: monoFeatures,
    leadingDistribution: TextLeadingDistribution.even,
  );

  /// 11 px on a 16 px line. Badges, rail counters.
  static const TextStyle ui11 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 11,
    height: 16 / 11,
  );

  /// 12 px on a 16 px line. Secondary metadata.
  static const TextStyle ui12 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 12,
    height: 16 / 12,
  );

  /// 13 px on a 20 px line. The default density of the application.
  static const TextStyle ui13 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 13,
    height: 20 / 13,
  );

  /// 14 px on a 22 px line. Comfortable body copy.
  static const TextStyle ui14 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 14,
    height: 22 / 14,
  );

  /// 16 px on a 24 px line. Section titles.
  static const TextStyle ui16 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 16,
    height: 24 / 16,
  );

  /// 20 px on a 28 px line. The single display size.
  static const TextStyle ui20 = TextStyle(
    fontFamily: uiFamily,
    fontFamilyFallback: uiFallback,
    fontWeight: regular,
    fontFeatures: uiFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 20,
    height: 28 / 20,
  );

  /// Monospace 11 px on a 16 px line.
  static const TextStyle mono11 = TextStyle(
    fontFamily: monoFamily,
    fontFamilyFallback: monoFallback,
    fontWeight: regular,
    fontFeatures: monoFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 11,
    height: 16 / 11,
  );

  /// Monospace 12 px on a 16 px line. Paths in the queue row.
  static const TextStyle mono12 = TextStyle(
    fontFamily: monoFamily,
    fontFamilyFallback: monoFallback,
    fontWeight: regular,
    fontFeatures: monoFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 12,
    height: 16 / 12,
  );

  /// Monospace 13 px on a 20 px line. Headers and bodies.
  static const TextStyle mono13 = TextStyle(
    fontFamily: monoFamily,
    fontFamilyFallback: monoFallback,
    fontWeight: regular,
    fontFeatures: monoFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 13,
    height: 20 / 13,
  );

  /// Monospace 14 px on a 22 px line. The editor.
  static const TextStyle mono14 = TextStyle(
    fontFamily: monoFamily,
    fontFamilyFallback: monoFallback,
    fontWeight: regular,
    fontFeatures: monoFeatures,
    leadingDistribution: TextLeadingDistribution.even,
    fontSize: 14,
    height: 22 / 14,
  );

  /// The UI base style without a size, for callers that set their own.
  static const TextStyle uiBase = _ui;

  /// The monospace base style without a size.
  static const TextStyle monoBase = _mono;
}

/// The three weights of the scale as suffixes on a [TextStyle].
///
/// `HType.ui13.medium` reads better than a `copyWith` at every call site and
/// makes "no 700" enforceable by having no getter for it.
extension HTextWeight on TextStyle {
  /// The same style at weight 500.
  TextStyle get medium => copyWith(fontWeight: HType.medium);

  /// The same style at weight 600.
  TextStyle get semibold => copyWith(fontWeight: HType.semibold);

  /// The same style in [color].
  TextStyle tinted(Color color) => copyWith(color: color);
}

/// The type scale as instance data, reachable from `HTokens.typography`.
@immutable
class HTypography {
  /// Creates a type scale. Use [standard] unless a test needs a variation.
  const HTypography({
    this.ui11 = HType.ui11,
    this.ui12 = HType.ui12,
    this.ui13 = HType.ui13,
    this.ui14 = HType.ui14,
    this.ui16 = HType.ui16,
    this.ui20 = HType.ui20,
    this.mono11 = HType.mono11,
    this.mono12 = HType.mono12,
    this.mono13 = HType.mono13,
    this.mono14 = HType.mono14,
  });

  /// The scale of the design direction.
  static const HTypography standard = HTypography();

  /// 11/16 UI.
  final TextStyle ui11;

  /// 12/16 UI.
  final TextStyle ui12;

  /// 13/20 UI, the default.
  final TextStyle ui13;

  /// 14/22 UI.
  final TextStyle ui14;

  /// 16/24 UI.
  final TextStyle ui16;

  /// 20/28 UI.
  final TextStyle ui20;

  /// 11/16 monospace.
  final TextStyle mono11;

  /// 12/16 monospace.
  final TextStyle mono12;

  /// 13/20 monospace.
  final TextStyle mono13;

  /// 14/22 monospace.
  final TextStyle mono14;
}
