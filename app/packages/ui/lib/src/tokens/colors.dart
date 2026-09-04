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

  /// Flächenalpha einer Zustandsfarbe unter einem überfahrenen Control.
  ///
  /// Die Tönung aus [tintAlpha] ist die Ruhefläche; Hover und Druck treten
  /// darüber, damit die drei Zustände nicht dieselbe Füllung tragen. Der Wert
  /// steht hier und nicht im Button, weil der Kontrast-Test jede Fläche
  /// aufzählen muss, auf der eine Zustandsfarbe als Text steht
  /// (`docs/UX.md` 6).
  static const double fillHoverAlpha = 0.14;

  /// Flächenalpha einer Zustandsfarbe unter einem gedrückten Control.
  ///
  /// Die dunkelste Fläche, die ein Control aus einer Zustandsfarbe baut,
  /// solange niemand hält; wer hält, bekommt [fillHoldAlpha].
  static const double fillPressedAlpha = 0.18;

  /// Flächenalpha der ruhenden Fläche einer geteilten Pille.
  ///
  /// Unter der Tönungsgrenze: die Pille ist ein Control und keine Auszeichnung,
  /// und ihre Ruhefläche soll nur zeigen, wo sie anfängt. Sie steht **nicht**
  /// in [HColorDerivation.fillAlphas], und das ist kein Versehen: der
  /// schlechteste Kontrast über einer Fläche wächst monoton mit dem Alpha,
  /// also decken die 0 und die [fillHoldAlpha] der Liste jeden Wert dazwischen
  /// mit ab.
  static const double fillRestAlpha = 0.06;

  /// Flächenalpha der wachsenden Füllung eines Haltens.
  ///
  /// Die dunkelste Fläche überhaupt, die ein Control aus einer Zustandsfarbe
  /// baut, und damit der strenge Fall für die Textableitung. Sie liegt
  /// bewusst über der Tönung einer ruhenden Fläche: das hier ist keine
  /// ruhende Fläche, sondern die Antwort auf einen Finger, der noch unten
  /// liegt. Der Wert steht hier und nicht im Control, weil ihn zwei Stellen
  /// zeichnen — das Halten der Aktionsleiste und die geteilte Pille — und
  /// weil der Kontrast-Test jede Fläche aufzählen muss, auf der eine
  /// Zustandsfarbe als Text steht (`docs/UX.md` 6).
  static const double fillHoldAlpha = 0.20;

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

  /// Die Kontrastuntergrenze für Text: AA für alles, was jemand liest.
  ///
  /// Sie gilt gegen die Fläche, auf der der Text wirklich steht — also auch
  /// gegen eine Tönung und gegen eine Füllung (`docs/UX.md` 6).
  static const double textMinContrast = 4.5;

  /// Die Kontrastuntergrenze für Flächen, Rails und Bögen.
  static const double areaMinContrast = 3.0;

  /// Jedes Flächenalpha, mit dem eine Zustandsfarbe hinter ihrem eigenen Text
  /// liegen kann: gar keines, die Tönung, die Hover-, die Druck- und die
  /// Haltefüllung.
  static const List<double> fillAlphas = <double>[
    0,
    HColors.tintAlpha,
    HColors.fillHoverAlpha,
    HColors.fillPressedAlpha,
    HColors.fillHoldAlpha,
  ];

  /// Jede Fläche, auf der Text in der Farbe [area] stehen kann.
  ///
  /// Das sind die vier Flächen der Leiter und dieselben vier noch einmal je
  /// Alpha aus [fillAlphas], weil ein Badge, ein Ghost-Button und ein
  /// gedrücktes Control ihre eigene Farbe als Fläche unter ihren Text legen.
  /// Ohne diese Liste misst ein Kontrast-Test die Farbe auf einem Untergrund,
  /// auf dem sie nie steht.
  static List<Color> textBackgrounds(Color area, List<Color> surfaces) {
    final List<Color> backgrounds = <Color>[];
    for (final Color surface in surfaces) {
      for (final double alpha in fillAlphas) {
        backgrounds.add(
          alpha == 0
              ? surface
              : flatten(area.withValues(alpha: alpha), surface),
        );
      }
    }
    return backgrounds;
  }

  /// Der schlechteste Kontrast, den [text] über der Farbe [area] auf einer der
  /// [surfaces] erreicht.
  static double worstTextContrast(
    Color text,
    Color area,
    List<Color> surfaces,
  ) => worstContrast(text, textBackgrounds(area, surfaces));

  /// Leitet aus der Flächenfarbe [area] die Variante ab, die Text tragen darf.
  ///
  /// Die Flächenfarbe bleibt, wo sie ist: sie ist auf 3:1 geklemmt, und mehr
  /// braucht eine Rail nicht. Ein Wort in derselben Farbe braucht 4,5:1, und
  /// zwar auf jedem Untergrund aus [textBackgrounds]. Die Ableitung schiebt
  /// deshalb nur die Helligkeit von den Flächen weg — im hellen Theme nach
  /// unten, im dunklen nach oben —, bis die Grenze überall erreicht ist; Ton,
  /// Sättigung und Alpha bleiben, damit das Wort dieselbe Farbe *ist* und
  /// nicht eine zweite (`docs/UX.md` 6 und 9, Punkt 5).
  ///
  /// Erreicht die Farbe die Grenze schon, kommt sie unverändert zurück.
  static Color textVariant(
    Color area, {
    required List<Color> surfaces,
    double minContrast = textMinContrast,
  }) {
    final List<Color> backgrounds = textBackgrounds(area, surfaces);
    final HSLColor hsl = HSLColor.fromColor(area);
    double meanSurface = 0;
    for (final Color surface in surfaces) {
      meanSurface += surface.computeLuminance();
    }
    meanSurface /= surfaces.length;
    // Von den Flächen weg: gegen helle Flächen abdunkeln, gegen dunkle
    // aufhellen. Die Richtung wird gemessen, nicht übergeben, damit eine
    // dritte Leiter sie nicht falsch bekommen kann.
    final double step = meanSurface > area.computeLuminance()
        ? -clampStep
        : clampStep;
    double lightness = hsl.lightness;
    Color candidate = area;
    while (worstContrast(candidate, backgrounds) < minContrast) {
      final double next = lightness + step;
      if (next < 0 || next > 1) {
        break;
      }
      lightness = next;
      candidate = hsl.withLightness(lightness).toColor();
    }
    // Die Schleife bricht am Rand der Leiter ab. Ohne diese Zusicherung käme
    // von dort eine Farbe zurück, die [minContrast] verfehlt, und niemand
    // merkte es: eine Beschriftung wäre zu blass, und der Test, der die
    // Palette prüft, misst genau die Farbe, die die Ableitung geliefert hat.
    assert(
      worstContrast(candidate, backgrounds) >= minContrast,
      '${toHex(area)} reaches only '
      '${worstContrast(candidate, backgrounds).toStringAsFixed(2)}:1 as text, '
      'not $minContrast:1',
    );
    return candidate;
  }

  /// Die Füllung, auf der [text] seine [minContrast] erreicht.
  ///
  /// Ein gefülltes Control trägt sein Wort in einer festen Farbe — der
  /// Primärbutton das [HSurfaceColors.onAccent] seines Themes. Erreicht das
  /// Wort auf der Füllung die Grenze nicht, weicht die **Füllung** zurück,
  /// nicht das Wort: [fill] ist ein Akzent und behält Ton, Sättigung und
  /// Alpha, damit der Button dieselbe Farbe *ist* und nicht eine zweite. Im
  /// hellen Theme misst Weiß auf dem Akzent 3,73:1, also wird der Akzent dort
  /// als Füllung dunkler; im dunklen misst die dunkelste Fläche auf ihm
  /// 7,14:1, und er kommt unverändert zurück (`docs/UX.md` 6).
  static Color readableFill(
    Color fill,
    Color text, {
    double minContrast = textMinContrast,
  }) {
    final HSLColor hsl = HSLColor.fromColor(fill);
    // Von der Textfarbe weg: gegen helle Schrift abdunkeln, gegen dunkle
    // aufhellen.
    final double step = text.computeLuminance() > fill.computeLuminance()
        ? -clampStep
        : clampStep;
    double lightness = hsl.lightness;
    Color candidate = fill;
    while (contrast(candidate, text) < minContrast) {
      final double next = lightness + step;
      if (next < 0 || next > 1) {
        break;
      }
      lightness = next;
      candidate = hsl.withLightness(lightness).toColor();
    }
    assert(
      contrast(candidate, text) >= minContrast,
      '${toHex(text)} reaches only '
      '${contrast(candidate, text).toStringAsFixed(2)}:1 on '
      '${toHex(fill)}, not $minContrast:1',
    );
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

  /// Das Alpha, das über einer schon liegenden Fläche von [beneath] genau
  /// [target] ergibt.
  ///
  /// Zwei Schichten derselben Farbe addieren sich nicht, sie komponieren:
  /// `0,20` über `0,06` sind wirksam `0,248`, und für diese Fläche gilt keine
  /// der Zusicherungen aus [fillAlphas], weil sie dort nicht vorkommt. Wer
  /// über einer Tönung füllt, rechnet deshalb mit dieser Funktion zurück
  /// (`docs/UX.md` 6).
  static double alphaOver(double target, double beneath) {
    assert(target >= beneath, 'a layer cannot lighten what lies beneath it');
    assert(beneath < 1, 'nothing composes over an opaque layer');
    return (target - beneath) / (1 - beneath);
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
