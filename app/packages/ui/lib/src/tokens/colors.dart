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
  // --- Dark neutral ladder (achromatic since HUM-355) -----------------------
  //
  // Bis HUM-355 trug die Leiter ein kühles Blaugrau (Ton ~230). Neben dem
  // Gelb der Marke wirkte das kalt und billig; seitdem ist jede Stufe ein
  // reines Grau (R = G = B), und die Leuchtdichte jeder Stufe liegt so nahe
  // an der alten, dass jede Kontrastzusage hält. Der Token-Test misst die
  // Buntheit jeder Stufe in OKLab und lässt keinen Ton zurück.

  /// Application background, the darkest surface.
  static const Color bg0 = Color(0xFF111111);

  /// Panel background.
  static const Color bg1 = Color(0xFF171717);

  /// Raised surface inside a panel (rows, inputs).
  static const Color bg2 = Color(0xFF1E1E1E);

  /// Highest surface (hover, pressed, selected fills).
  static const Color bg3 = Color(0xFF262626);

  /// Hairline separator.
  static const Color line = Color(0xFF2E2E2E);

  /// Emphasised hairline (focused or active borders).
  ///
  /// Auch die Druckfläche neutraler Controls; [fg1] misst auf ihr 4,57:1.
  static const Color lineStrong = Color(0xFF3D3D3D);

  /// Primary text. 12,35:1 auf [bg3], der hellsten dunklen Fläche.
  static const Color fg0 = Color(0xFFE8E8E8);

  /// Secondary text. 6,36:1 auf [bg3].
  static const Color fg1 = Color(0xFFA8A8A8);

  /// Tertiary text, disabled labels.
  ///
  /// Von der 4,5:1 ausgenommen (`docs/UX.md` 6); auf [bg3] misst die Stufe
  /// 3,06:1 und hält damit die 3:1 einer Fläche.
  static const Color fg2 = Color(0xFF707070);

  /// The single accent: focus, primary button, selection, links. Nothing else.
  ///
  /// Seit HUM-260 das Gelb der Marke: das App-Icon ist eine schwarze Faust auf
  /// einer gelben Kachel (#EBC05A bis #D49E34), und die Oberfläche übernimmt
  /// dieses Gelb als Akzent, mit dunklem Vordergrund ([onAccent]). Der Wert
  /// liegt zwischen den beiden Enden des Verlaufs der Kachel, eine Spur
  /// gesättigter als #EBC05A, damit er auf `bg0` bis `bg3` nicht ins Beige
  /// kippt. Er misst auf der hellsten dunklen Fläche (`bg3`) 8,21:1 und trägt
  /// [onAccent] mit 9,81:1.
  static const Color accent = Color(0xFFE8B84B);

  /// Text und Glyphen auf dem Akzent, in beiden Themes.
  ///
  /// Auf Gelb ist Weiß unlesbar (1,85:1 auf [accent]); die Marke stellt
  /// deshalb Schwarz auf Gelb, und die Oberfläche tut es ihr nach. Ein
  /// eigenes Token und nicht `bg0`, weil der Wert in beiden Themes derselbe
  /// ist: das helle Theme schreibt auf seinem Ocker [lAccent] ebenfalls dunkel
  /// (4,65:1); Weiß erreichte dort nur 3,89:1. Seit HUM-355 ein reines Grau
  /// wie die Leiter; die Faust des App-Icons behält ihr eigenes #14161C.
  static const Color onAccent = Color(0xFF161616);

  /// Achtung: Warnungen, die kein Zustand eines Flows sind.
  ///
  /// Das Gelb der Marke, also derselbe Wert wie [accent]: Gelb heißt
  /// Achtung, wie beim Absperrband. Bis HUM-260 trugen Warnungen das Amber
  /// von `held`; seit `held` nicht mehr gelb ist, trägt die Warnung das Gelb,
  /// das jeder als Achtung liest. Weil der Wert
  /// mit dem Akzent zusammenfällt, steht eine Warnung nie als Farbe allein:
  /// immer mit Glyph (`triangleAlert`) oder Text, und ein Control in
  /// Warnfarbe ist trotzdem eines, das man anfassen kann (`docs/UX.md` 3.3,
  /// Regel 12).
  static const Color warning = accent;

  /// Information: ein helles, unbuntes Silber.
  ///
  /// Bis HUM-355 das Blau, das bis HUM-260 der Akzent war (#7C9CF5). Blau
  /// neben dem Gelb der Marke wirkte fremd, und jeder bunte Ton, der noch frei
  /// wäre, liegt zu nah an einer Zustandsfarbe: Grün und Teal sind vergeben,
  /// Violett trägt `passthroughLlm`, Orchidee trägt `held`. Eine Auskunft ist
  /// deshalb leise: ein Silber knapp unter [fg0], das keine Zustandsfarbe
  /// trägt und das Glyph `info` und sein Wort nie allein lässt. Der Abstand zu
  /// [warning], zum Akzent und zu `held` steht in `tokens_test.dart`; der
  /// knappste Fall ist `held` unter Deuteranopie (OKLab 0,11).
  static const Color info = Color(0xFFE0E0E0);

  // --- Light neutral ladder (achromatic since HUM-355) -----------------------

  /// Light application background.
  static const Color lBg0 = Color(0xFFFBFBFB);

  /// Light panel background.
  static const Color lBg1 = Color(0xFFFFFFFF);

  /// Light raised surface.
  static const Color lBg2 = Color(0xFFF5F5F5);

  /// Light highest surface.
  static const Color lBg3 = Color(0xFFECECEC);

  /// Light hairline separator.
  static const Color lLine = Color(0xFFE5E5E5);

  /// Light emphasised hairline.
  static const Color lLineStrong = Color(0xFFCFCFCF);

  /// Light primary text. 15,03:1 auf [lBg3], der dunkelsten hellen Fläche.
  static const Color lFg0 = Color(0xFF181818);

  /// Light secondary text. 6,72:1 auf [lBg3].
  static const Color lFg1 = Color(0xFF515151);

  /// Light tertiary text. 3,25:1 auf [lBg3], wie [fg2] von 4,5:1 ausgenommen.
  static const Color lFg2 = Color(0xFF828282);

  /// Light accent: das Gelb der Marke als Ocker.
  ///
  /// Gelb auf Weiß ist unlesbar — [accent] misst auf `lBg3` 1,56:1 und
  /// verfehlt selbst die Flächengrenze. Der helle Akzent behält deshalb den
  /// Ton der Marke (41°) und geht in der Helligkeit so weit herunter, dass er
  /// zwei Grenzen zugleich hält: 3,29:1 als Fläche auf der dunkelsten hellen
  /// Fläche (`lBg3`) und 4,65:1 für [onAccent] auf ihm. Das Fenster dafür ist
  /// schmal (relative Leuchtdichte zwischen rund 0,211 und 0,243), und dieser
  /// Wert sitzt in seiner Mitte; damit ist die Füllung des Primärbuttons im
  /// hellen Theme der Akzent selbst, wie im dunklen.
  static const Color lAccent = Color(0xFFA57A1C);

  /// Light warning: der Ocker des hellen Akzents, aus demselben Grund wie
  /// [warning].
  static const Color lWarning = lAccent;

  /// Light information: ein dunkles Graphit, das Gegenstück zu [info].
  ///
  /// Bis HUM-355 das Blau #5B7FE6. Dunkler als das abgeleitete `timedOut`
  /// (#717171), damit eine Auskunft nie wie eine abgelaufene Anfrage aussieht
  /// (OKLab 0,18 auseinander), und mit 8,78:1 auf [lBg3] mehr als eine Fläche.
  static const Color lInfo = Color(0xFF404040);

  // --- Dark state colours ----------------------------------------------------

  /// A request waits for a human decision.
  ///
  /// Bis HUM-260 Amber (#E0B24A), bis HUM-355 Himmelscyan (#79C9EC). Seit
  /// Gelb der Akzent ist, darf `held` nicht gelb sein, damit Akzent und
  /// Zustand nie verwechselt werden: die Fläche zum Anfassen ist gelb, die
  /// Fläche zum Ablesen nicht (`docs/UX.md` 3.3, Regel 8 und 9). Das Cyan
  /// passte als Blau nicht zum Gelb der Marke; seit HUM-355 trägt `held` eine
  /// helle Orchidee (OKLCh-Ton 320°, Helligkeit 0,81, Buntheit 0,08).
  ///
  /// Der Ton ist der einzige, der in beiden Themes Abstand hält. Ein
  /// kräftiges Magenta fiele unter Protanopie auf das Violett von
  /// [passthrough], Teal auf [allowedEdited], ein Weiß im hellen Theme unter
  /// Deuteranopie auf das abgeleitete Teal. Die Orchidee hält zu jeder
  /// anderen Zustandsfarbe und zum Akzent mindestens 0,09 in OKLab, normal
  /// und unter Protanopie und Deuteranopie simuliert, im dunklen wie im
  /// abgeleiteten hellen Theme (#B56BC6); die Prüfung steht in
  /// `tokens_test.dart`.
  static const Color held = Color(0xFFD8B1E1);

  /// A request was allowed unchanged.
  static const Color allowed = Color(0xFF4FBF8C);

  /// A request was allowed after editing: green carrying the accent pencil.
  ///
  /// BACKLOG.md describes the state as "green plus an accent pencil dot". The
  /// dot is drawn by [HColors.accent]. Die Fläche entstand bis HUM-260 als
  /// [allowed], 18 Prozent zum damaligen blauen Akzent (#7C9CF5) gezogen; seit
  /// der Akzent gelb ist, steht sie als fester Wert. Zum Gelb gezogen läge sie
  /// näher an `allowed`, und die Rail einer bearbeiteten Anfrage wäre von
  /// einer unberührten kaum noch zu unterscheiden.
  static const Color allowedEdited = Color(0xFF57B99F);

  /// A request was blocked. Red means blocked, never anything else.
  static const Color blocked = Color(0xFFE5646E);

  /// A held request ran out of time.
  ///
  /// Ein reines Grau wie die Leiter (bis HUM-355 #8A90A2): abgelaufen heißt
  /// ohne Farbe.
  static const Color timedOut = Color(0xFF909090);

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

  /// Deckkraft des Rings, den ein gedrücktes Akzent-Control innen an seiner
  /// Kante trägt (HUM-272).
  ///
  /// Der Ring ist [onAccent] in dieser Deckkraft über der Druckfüllung. Der
  /// Entwurf des Owners nannte 50 Prozent; das misst auf dem hellen Druck
  /// nur 2,23:1 und auf dem dunklen 2,82:1, und ein Ring, der einen Zustand
  /// trägt, ist ein Bestandteil einer Bedienung und braucht 3:1 (WCAG
  /// 1.4.11). Bei 75 Prozent misst er 3,35:1 hell und 5,19:1 dunkel.
  static const double pressedRingAlpha = 0.75;

  // --- Method hues (never a state) ------------------------------------------

  /// `GET` and `HEAD`.
  static const Color methodGet = accent;

  /// `POST`.
  static const Color methodPost = passthrough;

  /// `PUT` and `PATCH`: the hue of [held], since HUM-355 the orchid.
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
  /// Primärbutton [HColors.onAccent]. Erreicht das Wort auf der Füllung die
  /// Grenze nicht, weicht die **Füllung** zurück, nicht das Wort: [fill] ist
  /// ein Akzent und behält Ton, Sättigung und Alpha, damit der Button
  /// dieselbe Farbe *ist* und nicht eine zweite. Seit HUM-260 erreicht der
  /// dunkle Vordergrund beide Akzente schon (9,81:1 dunkel, 4,65:1 hell), und
  /// beide kommen unverändert zurück; die Ableitung bleibt als Sicherung für
  /// den Tag, an dem jemand den Akzent verschiebt (`docs/UX.md` 6).
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

  // --- Hover und Druck gefüllter Controls (HUM-272) -------------------------

  /// Der kleinste Abstand in OKLab zwischen Ruhe und Hover eines gefüllten
  /// Controls.
  ///
  /// Rund 0,02 ist die kleinste Differenz, die man nebeneinander noch sieht;
  /// ein Zustandswechsel unter dem Zeiger steht nicht neben seinem Vorgänger,
  /// sondern folgt ihm, und braucht deshalb das Doppelte. Bis HUM-272 lag der
  /// Primärbutton im hellen Theme bei 0,011 und 0,012 und im dunklen beim
  /// Hover bei 0,020: der Ocker sitzt in einem schmalen Helligkeitsfenster
  /// (4,5:1 für [HColors.onAccent], 3:1 als Fläche auf `lBg3`), und ein
  /// Schritt, der nur die Helligkeit verschiebt, findet darin keinen Platz.
  static const double interactionStepMin = 0.04;

  /// Der kleinste Abstand in OKLab, den der Druck als Farbe von Hover und
  /// von Ruhe hält.
  ///
  /// Kleiner als [interactionStepMin], und das ist eine Entscheidung, kein
  /// Nachlass: den Schritt in den Druck trägt der Ring
  /// ([HColors.pressedRingAlpha]), die Farbe trägt nur einen Teil. Der Owner
  /// hat am 2026-10-22 einen Druck nahe am Ocker mit Ring einem Druck in
  /// Orange ohne Ring vorgezogen (HUM-272, Variante V2). Die Untergrenze hält
  /// die Farbe trotzdem über der Sichtbarkeitsschwelle, auch für eine Taste,
  /// die ohne Hover von der Ruhe in den Druck springt.
  static const double interactionJumpMin = 0.025;

  /// Wie weit Hover und Druck die OKLab-Helligkeit verschieben wollen: der
  /// Hover vom Wort weg, der Druck zum Wort hin.
  ///
  /// Ein Wunsch, keine Zusage: [fillHover] und [fillPressed] nehmen die
  /// Helligkeit, die diesem Wunsch am nächsten liegt und Wort und Fläche ihre
  /// Grenzen lässt.
  static const double fillStepLightness = 0.04;

  /// Der Faktor auf die Buntheit beim Hover: kräftiger, nicht blasser.
  ///
  /// Ein blasser Hover (Faktor 0,75, der erste Entwurf von HUM-272) wirkte im
  /// hellen Theme khakifarben, fast wie ein abgeschaltetes Control.
  static const double fillHoverChroma = 1.15;

  /// Der Faktor auf die Buntheit beim Druck: noch einmal satter.
  static const double fillPressedChroma = 1.3;

  /// Wie viel Grad der Druck über die Drehung des Hovers hinaus zum Warmen
  /// dreht: eine Spur, damit er tiefer wirkt und nahe am Ocker bleibt.
  static const double fillPressedHueStep = 4;

  /// Die größte Drehung des Farbtons zum Warmen hin, in Grad.
  ///
  /// Die Ableitung dreht nur so weit, wie der Schritt es braucht: im dunklen
  /// Theme reicht die Helligkeit, und der Hover bleibt gelb (0°); im engen
  /// Fenster des Ockers muss der Ton zum Orange hin (rund 13° beim Hover,
  /// 17° beim Druck).
  static const double fillMaxHueShift = 40;

  /// Die Reserve über [interactionStepMin] und [interactionJumpMin], damit
  /// das Runden auf acht Bit einen Schritt nicht unter die Grenze drückt.
  static const double _stepMargin = 0.002;

  /// Die Schrittweite der Helligkeitssuche.
  static const double _lightnessStep = 0.001;

  /// Wie weit die Helligkeitssuche höchstens von der Ruhe weggeht.
  static const double _lightnessReach = 0.1;

  /// Die Füllung eines überfahrenen Controls, das [text] auf [fill] schreibt.
  ///
  /// Heller, vom Wort weg, und satter. Die Helligkeit allein reicht nicht: im
  /// hellen Theme lässt das Fenster zwischen 4,5:1 für das Wort und 3:1 als
  /// Fläche auf [surfaces] nur rund 0,03 OKLab-Helligkeit. Buntheit und Ton
  /// gehen in keine der beiden Kontrastgrenzen ein und geben den Rest: der
  /// Ton dreht Grad um Grad zum Warmen, bis der Abstand zur Ruhe
  /// [interactionStepMin] erreicht.
  ///
  /// Findet sich kein solcher Schritt, kommt [fill] unverändert zurück. Der
  /// Token-Test meldet das laut, statt dass ein Hover still aussieht wie die
  /// Ruhe.
  static Color fillHover(Color fill, Color text, List<Color> surfaces) =>
      _hover(fill, text, surfaces).$1;

  /// Die Füllung eines gedrückten Controls, das [text] auf [fill] schreibt.
  ///
  /// Zum Wort hin, so weit es die 4,5:1 zulassen, satter als der Hover und
  /// [fillPressedHueStep] wärmer; weiter nur, wenn es braucht, um
  /// [interactionJumpMin] von Hover und Ruhe entfernt zu liegen. Den Rest des
  /// Schritts trägt der Ring. Ohne solchen Schritt kommt [fill] zurück, wie
  /// bei [fillHover].
  static Color fillPressed(Color fill, Color text, List<Color> surfaces) {
    final (Color hover, double hoverHue) = _hover(fill, text, surfaces);
    if (hover == fill) {
      return fill;
    }
    final double away = _awayFromText(fill, text);
    for (
      double hue = hoverHue + fillPressedHueStep;
      hue <= fillMaxHueShift;
      hue++
    ) {
      final Color? candidate = _fillStep(
        fill,
        text,
        surfaces,
        lightness: -fillStepLightness * away,
        chroma: fillPressedChroma,
        hue: -hue,
      );
      if (candidate != null &&
          perceivedDistance(hover, candidate) >=
              interactionJumpMin + _stepMargin &&
          perceivedDistance(fill, candidate) >=
              interactionJumpMin + _stepMargin) {
        return candidate;
      }
    }
    return fill;
  }

  /// Der Ring eines gedrückten Controls: [text] mit
  /// [HColors.pressedRingAlpha] über [pressed], opak gerechnet.
  static Color pressedRing(Color pressed, Color text) =>
      flatten(text.withValues(alpha: HColors.pressedRingAlpha), pressed);

  /// [fillHover] samt der Drehung des Tons in Grad, die er gebraucht hat.
  static (Color, double) _hover(Color fill, Color text, List<Color> surfaces) {
    final double away = _awayFromText(fill, text);
    for (double hue = 0; hue <= fillMaxHueShift; hue++) {
      final Color? candidate = _fillStep(
        fill,
        text,
        surfaces,
        lightness: fillStepLightness * away,
        chroma: fillHoverChroma,
        hue: -hue,
      );
      if (candidate != null &&
          perceivedDistance(fill, candidate) >=
              interactionStepMin + _stepMargin) {
        return (candidate, hue);
      }
    }
    return (fill, 0);
  }

  /// Der Abstand von [a] und [b] in OKLab (Björn Ottosson, 2020).
  static double perceivedDistance(Color a, Color b) {
    final List<double> pa = toOklab(a);
    final List<double> pb = toOklab(b);
    double sum = 0;
    for (int i = 0; i < 3; i++) {
      sum += (pa[i] - pb[i]) * (pa[i] - pb[i]);
    }
    return math.sqrt(sum);
  }

  /// [color] in OKLab als `[L, a, b]`; das Alpha zählt nicht.
  static List<double> toOklab(Color color) {
    final double r = _decode(color.r);
    final double g = _decode(color.g);
    final double b = _decode(color.b);
    final double l = _cbrt(
      0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b,
    );
    final double m = _cbrt(
      0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b,
    );
    final double s = _cbrt(
      0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b,
    );
    return <double>[
      0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
      1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
      0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ];
  }

  /// +1, wenn das Wort dunkler ist als die Füllung (Hover hellt dann auf),
  /// sonst −1. Gemessen und nicht übergeben, wie in [textVariant].
  static double _awayFromText(Color fill, Color text) =>
      text.computeLuminance() < fill.computeLuminance() ? 1 : -1;

  /// [fill] in OKLCh um [chroma] (Faktor) und [hue] (Grad) gedreht, mit der
  /// Helligkeit, die der Ruhe plus [lightness] am nächsten liegt und auf der
  /// [text] 4,5:1 und die Füllung auf jeder der [surfaces] 3:1 hält.
  ///
  /// Gesucht wird nach beiden Seiten des Wunsches, weil ein wärmerer Ton bei
  /// derselben OKLab-Helligkeit eine andere Leuchtdichte hat: das Fenster
  /// liegt für Orange woanders als für Ocker. `null`, wenn keine Helligkeit
  /// im Abstand [_lightnessReach] von der Ruhe passt.
  static Color? _fillStep(
    Color fill,
    Color text,
    List<Color> surfaces, {
    required double lightness,
    required double chroma,
    required double hue,
  }) {
    final List<double> lab = toOklab(fill);
    final double l0 = lab[0];
    final double c = math.sqrt(lab[1] * lab[1] + lab[2] * lab[2]) * chroma;
    final double h = math.atan2(lab[2], lab[1]) + hue * math.pi / 180;
    final double target = l0 + lightness;
    final int steps = (2 * _lightnessReach / _lightnessStep).round();
    for (int k = 0; k <= steps; k++) {
      for (final int side in k == 0 ? const <int>[0] : const <int>[-1, 1]) {
        final double l = target + side * k * _lightnessStep;
        // Außerhalb von 0 bis 1 gibt es keine Farbe; eine schwarze Füllung
        // unter weißer Schrift will beim Hover genau dorthin.
        if ((l - l0).abs() > _lightnessReach + 1e-9 || l < 0 || l > 1) {
          continue;
        }
        final Color? candidate = _fromOklch(l, c, h);
        if (candidate != null &&
            contrast(candidate, text) >= textMinContrast &&
            worstContrast(candidate, surfaces) >= areaMinContrast) {
          return candidate;
        }
      }
    }
    return null;
  }

  /// Eine opake Farbe aus OKLCh; eine Buntheit außerhalb von sRGB wird auf
  /// den Rand des Farbraums gekürzt, Helligkeit und Ton bleiben. `null`, wenn
  /// selbst das Grau dieser Helligkeit nicht in sRGB liegt.
  static Color? _fromOklch(double l, double chroma, double hue) {
    List<double>? rgb = _oklabToSrgb(
      l,
      chroma * math.cos(hue),
      chroma * math.sin(hue),
    );
    if (rgb == null) {
      double lo = 0;
      double hi = chroma;
      for (int i = 0; i < 32; i++) {
        final double mid = (lo + hi) / 2;
        if (_oklabToSrgb(l, mid * math.cos(hue), mid * math.sin(hue)) == null) {
          hi = mid;
        } else {
          lo = mid;
        }
      }
      rgb = _oklabToSrgb(l, lo * math.cos(hue), lo * math.sin(hue));
      if (rgb == null) {
        return null;
      }
    }
    int channel(double v) => (v * 255).round().clamp(0, 255);
    return Color.fromARGB(
      255,
      channel(rgb[0]),
      channel(rgb[1]),
      channel(rgb[2]),
    );
  }

  /// OKLab nach gamma-kodiertem sRGB in 0..1, oder `null` außerhalb von sRGB.
  static List<double>? _oklabToSrgb(double l, double a, double b) {
    final double l1 = l + 0.3963377774 * a + 0.2158037573 * b;
    final double m1 = l - 0.1055613458 * a - 0.0638541728 * b;
    final double s1 = l - 0.0894841775 * a - 1.2914855480 * b;
    final double lc = l1 * l1 * l1;
    final double mc = m1 * m1 * m1;
    final double sc = s1 * s1 * s1;
    final List<double> linear = <double>[
      4.0767416621 * lc - 3.3077115913 * mc + 0.2309699292 * sc,
      -1.2684380046 * lc + 2.6097574011 * mc - 0.3413193965 * sc,
      -0.0041960863 * lc - 0.7034186147 * mc + 1.7076147010 * sc,
    ];
    const double eps = 1e-6;
    if (linear.any((double v) => v < -eps || v > 1 + eps)) {
      return null;
    }
    return <double>[for (final double v in linear) _encode(v.clamp(0, 1))];
  }

  static double _decode(double c) =>
      c <= 0.04045 ? c / 12.92 : math.pow((c + 0.055) / 1.055, 2.4).toDouble();

  static double _encode(double c) => c <= 0.0031308
      ? 12.92 * c
      : 1.055 * math.pow(c, 1 / 2.4).toDouble() - 0.055;

  static double _cbrt(double x) =>
      x < 0 ? -math.pow(-x, 1 / 3).toDouble() : math.pow(x, 1 / 3).toDouble();

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
