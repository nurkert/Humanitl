import 'package:flutter/widgets.dart';

import 'colors.dart';
import 'flow_state.dart';
import 'motion.dart';
import 'spacing.dart';
import 'typography.dart';

/// Surface, line and text colours of one theme.
@immutable
class HSurfaceColors {
  /// Creates a neutral ladder. Use [dark] or [light].
  const HSurfaceColors({
    required this.bg0,
    required this.bg1,
    required this.bg2,
    required this.bg3,
    required this.line,
    required this.lineStrong,
    required this.fg0,
    required this.fg1,
    required this.fg2,
    required this.accent,
    required this.accentText,
    required this.accentFill,
    required this.onAccent,
  });

  /// The dark ladder of BACKLOG.md 5.
  ///
  /// Nicht mehr `const`: [accentText] und [accentFill] werden abgeleitet, wie
  /// die Textvariante jeder Zustandsfarbe (`docs/UX.md` 6).
  static final HSurfaceColors dark = HSurfaceColors(
    bg0: HColors.bg0,
    bg1: HColors.bg1,
    bg2: HColors.bg2,
    bg3: HColors.bg3,
    line: HColors.line,
    lineStrong: HColors.lineStrong,
    fg0: HColors.fg0,
    fg1: HColors.fg1,
    fg2: HColors.fg2,
    accent: HColors.accent,
    accentText: HColorDerivation.textVariant(
      HColors.accent,
      surfaces: HColorDerivation.darkSurfaces,
    ),
    accentFill: HColorDerivation.readableFill(HColors.accent, HColors.bg0),
    onAccent: HColors.bg0,
  );

  /// The light ladder: the dark one inverted.
  static final HSurfaceColors light = HSurfaceColors(
    bg0: HColors.lBg0,
    bg1: HColors.lBg1,
    bg2: HColors.lBg2,
    bg3: HColors.lBg3,
    line: HColors.lLine,
    lineStrong: HColors.lLineStrong,
    fg0: HColors.lFg0,
    fg1: HColors.lFg1,
    fg2: HColors.lFg2,
    accent: HColors.lAccent,
    accentText: HColorDerivation.textVariant(
      HColors.lAccent,
      surfaces: HColorDerivation.lightSurfaces,
    ),
    accentFill: HColorDerivation.readableFill(HColors.lAccent, HColors.lBg1),
    onAccent: HColors.lBg1,
  );

  /// Application background.
  final Color bg0;

  /// Panel background.
  final Color bg1;

  /// Raised surface.
  final Color bg2;

  /// Highest surface.
  final Color bg3;

  /// Hairline.
  final Color line;

  /// Emphasised hairline.
  final Color lineStrong;

  /// Primary text.
  final Color fg0;

  /// Secondary text.
  final Color fg1;

  /// Tertiary text.
  final Color fg2;

  /// The single accent: focus ring, selection, the fill of the one primary
  /// control. Eine Fläche, keine Textfarbe.
  final Color accent;

  /// Der Akzent, wie er ein Wort tragen darf.
  ///
  /// [accent] ist auf 3:1 geklemmt wie jede Fläche: hell misst er 3,16:1 bis
  /// 3,73:1 auf den vier Flächen und 2,63:1 auf seiner eigenen Tönung. Ein
  /// Link, ein Pfad in einer Diagnose und die Beschriftung eines Fix-Controls
  /// stehen deshalb in dieser Farbe hier, die auf allen Flächen und auf jeder
  /// Füllung aus [HColorDerivation.fillAlphas] 4,5:1 erreicht
  /// (`docs/UX.md` 6).
  final Color accentText;

  /// Der Akzent als Füllung eines Controls, das [onAccent] darauf schreibt.
  ///
  /// Weiß auf dem hellen Akzent misst 3,73:1; die Füllung weicht deshalb
  /// zurück, bis das Wort 4,5:1 erreicht. Im dunklen Theme ist sie der Akzent
  /// selbst, weil [onAccent] dort schon 7,14:1 misst.
  final Color accentFill;

  /// Text and glyphs drawn on top of [accentFill].
  final Color onAccent;

  /// The ladder from [bg0] to [bg3], darkest surface first in dark mode.
  List<Color> get ladder => <Color>[bg0, bg1, bg2, bg3];
}

/// The hue of each HTTP method in one theme.
///
/// Method hues are references, never states: `GET` borrows the accent, `POST`
/// the passthrough hue, `PUT`/`PATCH` the held hue and `DELETE` the blocked hue
/// at seventy percent. The dark table *is* the `HColors.method*` constants, so
/// editing a constant changes what the badge paints; the light table points at
/// the light counterparts of the same references.
@immutable
class HMethodColors {
  /// Creates a method palette. Use [dark] or [light].
  const HMethodColors({
    required this.get,
    required this.post,
    required this.putPatch,
    required this.delete,
    required this.unknown,
  });

  /// The dark table, read straight from [HColors].
  static const HMethodColors dark = HMethodColors(
    get: HColors.methodGet,
    post: HColors.methodPost,
    putPatch: HColors.methodPutPatch,
    delete: HColors.methodDelete,
    unknown: HColors.fg2,
  );

  /// The light table: the same references resolved in the light theme.
  ///
  /// `DELETE` is *not* the light blocked colour faded to seventy percent: at
  /// that alpha the fade composites to 2.8:1 on the raised light surfaces. It
  /// is derived from [HColors.methodDelete] the way a state colour is, so the
  /// legibility clamp of [HColorDerivation.lightState] runs on the translucent
  /// colour itself and guarantees 3:1 over every surface in
  /// [HColorDerivation.lightSurfaces]. Hue, saturation and the seventy percent
  /// alpha survive the derivation; only the lightness drops.
  static final HMethodColors light = HMethodColors(
    get: HColors.lAccent,
    post: HStateColors.light.passthroughLlm,
    putPatch: HStateColors.light.held,
    delete: HColorDerivation.lightState(HColors.methodDelete),
    unknown: HColors.lFg2,
  );

  /// Die dunkle Methodentabelle, wie sie ein Wort tragen darf.
  ///
  /// Ein Method-Badge zeichnet sein Kürzel auf die eigene Tönung; auf ihr
  /// misst `DELETE` dunkel 2,65:1, und damit trägt keine Methodenfarbe legal
  /// Text. Fläche und Label werden deshalb getrennt geführt: die Tönung bleibt
  /// die Tabellenfarbe, das Kürzel steht in dieser hier
  /// (`docs/UX.md` 9, Punkt 5).
  static final HMethodColors darkText = _textOf(
    dark,
    HColorDerivation.darkSurfaces,
  );

  /// Die helle Methodentabelle, wie sie ein Wort tragen darf.
  static final HMethodColors lightText = _textOf(
    light,
    HColorDerivation.lightSurfaces,
  );

  static HMethodColors _textOf(HMethodColors area, List<Color> surfaces) {
    Color text(Color color) =>
        HColorDerivation.textVariant(color, surfaces: surfaces);
    return HMethodColors(
      get: text(area.get),
      post: text(area.post),
      putPatch: text(area.putPatch),
      delete: text(area.delete),
      unknown: text(area.unknown),
    );
  }

  /// `GET` and `HEAD`.
  final Color get;

  /// `POST`.
  final Color post;

  /// `PUT` and `PATCH`.
  final Color putPatch;

  /// `DELETE`, deliberately below full strength so it never reads as a block.
  final Color delete;

  /// Any other verb. An unfamiliar method must not look like a familiar one.
  final Color unknown;

  /// The hue of [method], in any case.
  Color of(String method) => switch (method.toUpperCase()) {
    'GET' || 'HEAD' => get,
    'POST' => post,
    'PUT' || 'PATCH' => putPatch,
    'DELETE' => delete,
    _ => unknown,
  };

  /// The five hues in table order: [get], [post], [putPatch], [delete],
  /// [unknown]. For tests that sweep the whole table.
  List<Color> get all => <Color>[get, post, putPatch, delete, unknown];
}

/// Every design token of one theme, reachable from the widget tree.
///
/// Colours are per theme, the scales are not; both live here so that a widget
/// needs exactly one lookup. Read them with `HTheme.of(context)`.
@immutable
class HTokens {
  /// Creates a token set. Prefer [dark] and [light].
  const HTokens({
    required this.brightness,
    required this.colors,
    required this.state,
    required this.method,
    required this.stateText,
    required this.methodText,
    this.typography = HTypography.standard,
    this.spacing = HSpacingTokens.standard,
    this.radii = HRadiusTokens.standard,
    this.sizes = HSizeTokens.standard,
    this.motion = HMotionTokens.standard,
  });

  /// The dark theme, which the design is drawn for.
  ///
  /// Nicht mehr `const`: [stateText] und [methodText] werden abgeleitet, damit
  /// niemand eine zweite Farbe von Hand schreibt (`docs/UX.md` 6).
  static final HTokens dark = HTokens(
    brightness: Brightness.dark,
    colors: HSurfaceColors.dark,
    state: HStateColors.dark,
    method: HMethodColors.dark,
    stateText: HStateColors.darkText,
    methodText: HMethodColors.darkText,
  );

  /// The light theme, derived from the dark one.
  static final HTokens light = HTokens(
    brightness: Brightness.light,
    colors: HSurfaceColors.light,
    state: HStateColors.light,
    method: HMethodColors.light,
    stateText: HStateColors.lightText,
    methodText: HMethodColors.lightText,
  );

  /// The token set of [brightness].
  static HTokens forBrightness(Brightness brightness) =>
      brightness == Brightness.dark ? dark : light;

  /// Which of the two themes this is.
  final Brightness brightness;

  /// Surfaces, lines and text.
  final HSurfaceColors colors;

  /// The eight state colours.
  final HStateColors state;

  /// The method hues.
  final HMethodColors method;

  /// Die acht Zustandsfarben, wie sie ein Wort tragen dürfen.
  ///
  /// [state] ist die Fläche und auf 3:1 geklemmt; alles, was gelesen wird,
  /// nimmt diese Palette und erreicht 4,5:1 — auch auf einer Tönung und auf
  /// einer Füllung derselben Farbe (`docs/UX.md` 6).
  final HStateColors stateText;

  /// Die Methodenfarben, wie sie ein Kürzel tragen dürfen.
  final HMethodColors methodText;

  /// The type scale.
  final HTypography typography;

  /// The spacing scale.
  final HSpacingTokens spacing;

  /// The corner radii.
  final HRadiusTokens radii;

  /// The fixed sizes of the shell.
  final HSizeTokens sizes;

  /// Durations and curves.
  final HMotionTokens motion;

  /// The colour of [flowState] in this theme.
  Color stateColor(HFlowState flowState) => state.resolve(flowState);

  /// Die Farbe, in der [flowState] als Text stehen darf.
  ///
  /// Für jede Fläche, jede Rail, jeden Bogen und jedes Glyph gilt
  /// [stateColor]; für jedes Wort und jede Ziffer gilt diese hier. Ein Glyph
  /// steht bewusst auf der anderen Seite: es ist eine Grafik, seine Grenze
  /// ist die 3:1 aus `docs/UX.md` 6, und die Flächenpalette hält sie auf
  /// jeder Fläche beider Leitern. Nähme es die Textvariante, verlöre
  /// ausgerechnet `autoRule` seinen Ton — die Farbe trägt 60 % Deckkraft, und
  /// 4,5:1 erreicht sie nur dicht an Weiß oder Schwarz —, und das Glyph ist
  /// der Kanal, der die Farbe für Farbenblinde verdoppelt (3.3).
  Color stateTextColor(HFlowState flowState) => stateText.resolve(flowState);

  /// Die Farbe, in der das Kürzel von [method] stehen darf.
  Color methodTextColor(String method) => methodText.of(method);

  /// Die Textvariante von [color], falls [color] eine Flächenfarbe ist; sonst
  /// [color] selbst.
  ///
  /// Damit trägt ein Control, dem jemand eine Zustandsfarbe reicht, das Wort
  /// von selbst in der Farbe, die 4,5:1 erreicht, ohne dass jeder Aufrufer an
  /// zwei Farben denken muss. Eine Nachschlagetabelle über acht Farben und den
  /// Akzent, keine Ableitung zur Laufzeit: die Ableitung kostet einen
  /// Suchlauf, und dieser Aufruf steht in jeder Zeile einer zehntausend Zeilen
  /// langen Liste.
  ///
  /// Gesucht wird in **beiden** Paletten, nicht nur in der des laufenden
  /// Themes. Wer im hellen Theme eine dunkle Konstante reicht — und ein
  /// Aufrufer, der `HColors.held` schreibt statt `tokens.state.held`, tut das
  /// —, bekäme sonst seine Farbe unverändert zurück und malte bei rund 2,5:1,
  /// ohne dass etwas fehlschlägt. Die `fg`-Leiter kommt unverändert zurück:
  /// sie ist bereits eine Textleiter.
  Color stateTextOf(Color color) {
    for (final HFlowState flowState in HFlowState.values) {
      if (state.resolve(flowState) == color ||
          HStateColors.dark.resolve(flowState) == color ||
          HStateColors.light.resolve(flowState) == color) {
        return stateText.resolve(flowState);
      }
    }
    if (color == colors.accent ||
        color == HColors.accent ||
        color == HColors.lAccent) {
      return colors.accentText;
    }
    return color;
  }

  /// [color] as an area tint, capped at [HColors.tintAlpha].
  Color tint(Color color) => HColorDerivation.tint(color);
}
