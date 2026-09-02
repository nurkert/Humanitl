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
    required this.onAccent,
  });

  /// The dark ladder of BACKLOG.md 5.
  static const HSurfaceColors dark = HSurfaceColors(
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
    onAccent: HColors.bg0,
  );

  /// The light ladder: the dark one inverted.
  static const HSurfaceColors light = HSurfaceColors(
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

  /// The single accent.
  final Color accent;

  /// Text and glyphs drawn on top of [accent].
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
    this.typography = HTypography.standard,
    this.spacing = HSpacingTokens.standard,
    this.radii = HRadiusTokens.standard,
    this.sizes = HSizeTokens.standard,
    this.motion = HMotionTokens.standard,
  });

  /// The dark theme, which the design is drawn for.
  static const HTokens dark = HTokens(
    brightness: Brightness.dark,
    colors: HSurfaceColors.dark,
    state: HStateColors.dark,
    method: HMethodColors.dark,
  );

  /// The light theme, derived from the dark one.
  static final HTokens light = HTokens(
    brightness: Brightness.light,
    colors: HSurfaceColors.light,
    state: HStateColors.light,
    method: HMethodColors.light,
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

  /// [color] as an area tint, capped at [HColors.tintAlpha].
  Color tint(Color color) => HColorDerivation.tint(color);
}
