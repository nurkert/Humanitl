import 'dart:math' as math;

import 'package:flutter/widgets.dart';

/// The raw colour constants of the Airlock design direction.
///
/// Every value that BACKLOG.md section 5 spells out appears here literally and
/// exactly once. The dark ladder is the source of truth; the light ladder
/// inverts it. State colours exist as dark constants only — their light
/// counterparts are derived by [HColorDerivation.lightState] so that nobody
/// guesses a hex by hand (see the pitfalls of HUM-008).
abstract final class HColors {
  // --- Dark neutral ladder (hue ~230) ---------------------------------------

  /// Application background, the darkest surface.
  static const Color bg0 = Color(0xFF0F1115);

  /// Panel background.
  static const Color bg1 = Color(0xFF151821);

  /// Raised surface inside a panel (rows, inputs).
  static const Color bg2 = Color(0xFF1B1F2A);

  /// Highest surface (hover, pressed, selected fills).
  static const Color bg3 = Color(0xFF232838);

  /// Hairline separator.
  static const Color line = Color(0xFF2A3040);

  /// Emphasised hairline (focused or active borders).
  static const Color lineStrong = Color(0xFF384056);

  /// Primary text.
  static const Color fg0 = Color(0xFFE6E8EE);

  /// Secondary text.
  static const Color fg1 = Color(0xFFA3A9B8);

  /// Tertiary text, disabled labels.
  static const Color fg2 = Color(0xFF6B7186);

  /// The single accent: focus, primary button, selection, links. Nothing else.
  static const Color accent = Color(0xFF7C9CF5);

  // --- Light neutral ladder --------------------------------------------------

  /// Light application background.
  static const Color lBg0 = Color(0xFFFAFBFD);

  /// Light panel background.
  static const Color lBg1 = Color(0xFFFFFFFF);

  /// Light raised surface.
  static const Color lBg2 = Color(0xFFF3F5F9);

  /// Light highest surface.
  static const Color lBg3 = Color(0xFFE9ECF3);

  /// Light hairline separator.
  static const Color lLine = Color(0xFFE1E5EE);

  /// Light emphasised hairline.
  static const Color lLineStrong = Color(0xFFC9CFDC);

  /// Light primary text.
  static const Color lFg0 = Color(0xFF16181F);

  /// Light secondary text.
  static const Color lFg1 = Color(0xFF4B5162);

  /// Light tertiary text.
  static const Color lFg2 = Color(0xFF7C8294);

  /// Light accent.
  static const Color lAccent = Color(0xFF5B7FE6);

  // --- Dark state colours ----------------------------------------------------

  /// A request waits for a human decision.
  static const Color held = Color(0xFFE0B24A);

  /// A request was allowed unchanged.
  static const Color allowed = Color(0xFF4FBF8C);

  /// A request was allowed after editing: green carrying the accent pencil.
  ///
  /// BACKLOG.md describes the state as "green plus an accent pencil dot". The
  /// dot is drawn by [HColors.accent]; the surface colour is [allowed] pulled
  /// [allowedEditedBlend] of the way towards the accent so that the rail of an
  /// edited flow is distinguishable from an untouched one.
  static const Color allowedEdited = Color(0xFF57B99F);

  /// How far [allowedEdited] sits between [allowed] and [accent].
  static const double allowedEditedBlend = 0.18;

  /// A request was blocked. Red means blocked, never anything else.
  static const Color blocked = Color(0xFFE5646E);

  /// A held request ran out of time.
  static const Color timedOut = Color(0xFF8A90A2);

  /// A rule decided without asking: [allowed] at 60 percent.
  static const Color autoRule = Color(0x994FBF8C);

  /// The alpha applied to [allowed] to obtain [autoRule].
  static const double autoRuleOpacity = 0.6;

  /// Traffic that passes through to the configured LLM endpoint.
  static const Color passthrough = Color(0xFFB48AF0);

  /// Error, or a secret was found. Orange, so that red stays "blocked".
  static const Color secret = Color(0xFFF0784F);

  /// Maximum area alpha for a state colour used as a tint.
  static const double tintAlpha = 0.10;

  // --- Method hues (never a state) ------------------------------------------

  /// `GET` and `HEAD`.
  static const Color methodGet = accent;

  /// `POST`.
  static const Color methodPost = passthrough;

  /// `PUT` and `PATCH`.
  static const Color methodPutPatch = held;

  /// `DELETE`: [blocked] at 70 percent, so it never reads as a block.
  static const Color methodDelete = Color(0xB3E5646E);
}

/// Colour maths shared by the token layer and its tests.
///
/// Kept public on purpose: the light theme is *derived*, and a test that cannot
/// re-run the derivation cannot protect it.
abstract final class HColorDerivation {
  /// Lightness step used when clamping a derived colour towards legibility.
  static const double clampStep = 0.01;

  /// Every dark surface a state colour is painted on.
  ///
  /// `bg2` and `bg3` are included because rows, buttons and modals put state
  /// colours on them, not only on the panel background.
  static const List<Color> darkSurfaces = <Color>[
    HColors.bg0,
    HColors.bg1,
    HColors.bg2,
    HColors.bg3,
  ];

  /// Every light surface a state colour is painted on.
  ///
  /// `lBg2` and `lBg3` are the darkest of the four and therefore the strict
  /// case for a dark foreground; a clamp against `lBg0`/`lBg1` alone would let
  /// a colour through that fails on a selected row.
  static const List<Color> lightSurfaces = <Color>[
    HColors.lBg0,
    HColors.lBg1,
    HColors.lBg2,
    HColors.lBg3,
  ];

  /// The WCAG 2.1 contrast ratio between [a] and [b], from 1.0 to 21.0.
  ///
  /// Both colours must be opaque; use [flatten] first when they are not.
  static double contrast(Color a, Color b) {
    final double la = a.computeLuminance();
    final double lb = b.computeLuminance();
    final double hi = math.max(la, lb);
    final double lo = math.min(la, lb);
    return (hi + 0.05) / (lo + 0.05);
  }

  /// Composites [foreground] over the opaque [background].
  static Color flatten(Color foreground, Color background) =>
      Color.alphaBlend(foreground, background);

  /// Returns [color] with its HSL lightness reduced by [amount], alpha kept.
  static Color darken(Color color, double amount) {
    final HSLColor hsl = HSLColor.fromColor(color);
    return hsl
        .withLightness((hsl.lightness - amount).clamp(0.0, 1.0))
        .toColor();
  }

  /// Derives the light-theme variant of a dark state colour.
  ///
  /// The rule is "twelve percent darker" from BACKLOG.md 5, followed by a
  /// legibility clamp: the lightness keeps dropping in [clampStep] steps until
  /// the colour — composited over every surface in [surfaces], by default all
  /// four of [lightSurfaces] — reaches [minContrast]. Without the clamp `held`
  /// and `autoRule` would land below 3:1 on white, which the acceptance test
  /// of HUM-008 forbids; clamping against the two lightest surfaces only would
  /// still leave them short on the raised surfaces a selected row uses.
  static Color lightState(
    Color dark, {
    double lightnessDelta = 0.12,
    double minContrast = 3.0,
    List<Color> surfaces = lightSurfaces,
  }) {
    final HSLColor hsl = HSLColor.fromColor(dark);
    double lightness = (hsl.lightness - lightnessDelta).clamp(0.0, 1.0);
    Color candidate = hsl.withLightness(lightness).toColor();
    while (lightness > 0.0 &&
        !_reaches(candidate, surfaces: surfaces, minContrast: minContrast)) {
      lightness = (lightness - clampStep).clamp(0.0, 1.0);
      candidate = hsl.withLightness(lightness).toColor();
    }
    return candidate;
  }

  /// The lowest contrast [color] reaches over any of [surfaces].
  static double worstContrast(Color color, List<Color> surfaces) {
    double worst = double.infinity;
    for (final Color surface in surfaces) {
      worst = math.min(worst, contrast(flatten(color, surface), surface));
    }
    return worst;
  }

  /// [color] as `#RRGGBB`, or `#AARRGGBB` when it is translucent.
  static String toHex(Color color) {
    final String argb = color
        .toARGB32()
        .toRadixString(16)
        .toUpperCase()
        .padLeft(8, '0');
    return argb.startsWith('FF') ? '#${argb.substring(2)}' : '#$argb';
  }

  /// [color] as a background tint, at most [HColors.tintAlpha] of area alpha.
  static Color tint(Color color, [double alpha = HColors.tintAlpha]) =>
      color.withValues(alpha: math.min(alpha, HColors.tintAlpha));

  /// [color] faded to [opacity], for disabled and de-emphasised states.
  static Color fade(Color color, double opacity) =>
      color.withValues(alpha: color.a * opacity);

  static bool _reaches(
    Color color, {
    required List<Color> surfaces,
    required double minContrast,
  }) => worstContrast(color, surfaces) >= minContrast;
}
