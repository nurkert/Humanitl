/// The sixteen colours a terminal emulator needs, in the ladder of this
/// program (HUM-042).
///
/// A terminal decides its own colours: the agent writes `ESC [ 31 m` and the
/// emulator looks up "red". The lookup table is the one place where a design
/// system can still speak — and it must, because a terminal fills sixty
/// percent of the sandbox screen and would otherwise be the one surface with
/// somebody else's palette on it.
///
/// The eight base colours are therefore **not** invented here. Four of them
/// are colours this program already uses for exactly that hue: `blocked` is
/// red, `allowed` is green, the accent is yellow, `passthroughLlm` is magenta.
/// Seit HUM-260 ist der Akzent das Gelb der Marke. Blau und Cyan trägt seit
/// HUM-355 keine Farbe der Oberfläche mehr (`held` ist eine Orchidee, `info`
/// ein Silber); ein Terminal braucht beide trotzdem, weil ein Agent mit
/// `ESC [ 34 m` und `ESC [ 36 m` zwei verschiedene Dinge meint. Beide stehen
/// deshalb als eigene Konstanten unten und gehören nur dem Terminal. What a
/// human learns on the queue screen holds in the terminal.
///
/// This file holds no dependency on a terminal package. It carries colours;
/// the pane that renders them maps them to whatever its emulator wants
/// (`app/lib/features/sandbox/widgets/terminal_pane.dart`).
library;

import 'package:flutter/widgets.dart';

import 'colors.dart';

/// How many colours the base ladder has. ANSI has eight, and eight again in
/// bright.
const int hTerminalRamp = 8;

/// The palette of a terminal: sixteen colours plus the four that frame them.
@immutable
class HTerminalPalette {
  /// Builds a palette. [normal] and [bright] carry [hTerminalRamp] colours
  /// each, in the ANSI order black, red, green, yellow, blue, magenta, cyan,
  /// white.
  const HTerminalPalette({
    required this.background,
    required this.foreground,
    required this.cursor,
    required this.selection,
    required this.normal,
    required this.bright,
  }) : assert(
         normal.length == hTerminalRamp && bright.length == hTerminalRamp,
         'a terminal palette has eight colours per ramp',
       );

  /// The surface the agent writes on.
  final Color background;

  /// The colour of text that names no colour of its own.
  final Color foreground;

  /// The block that shows where the agent is typing.
  final Color cursor;

  /// The fill behind a selection the human dragged.
  final Color selection;

  /// The eight base colours, ANSI order.
  final List<Color> normal;

  /// The eight bright colours, same order.
  final List<Color> bright;

  /// The dark ladder.
  static final HTerminalPalette dark = HTerminalPalette(
    background: HColors.bg0,
    foreground: HColors.fg0,
    cursor: HColors.accent,
    // The selection is a tint and not a fill: the text under it stays the
    // text, and a terminal has no second colour to fall back on.
    selection: HColorDerivation.tint(HColors.accent, HColors.fillPressedAlpha),
    normal: <Color>[
      HColors.bg2,
      HColors.blocked,
      HColors.allowed,
      HColors.accent,
      _blue,
      HColors.passthrough,
      _cyan,
      HColors.fg1,
    ],
    bright: <Color>[
      HColors.fg2,
      _lift(HColors.blocked),
      _lift(HColors.allowed),
      _lift(HColors.accent),
      _lift(_blue),
      _lift(HColors.passthrough),
      _lift(_cyan),
      HColors.fg0,
    ],
  );

  /// The light ladder, derived the same way the state colours are: twelve
  /// percent darker with a legibility clamp, so that no colour of the agent
  /// disappears into a white surface ([HColorDerivation.lightState]).
  static final HTerminalPalette light = HTerminalPalette(
    background: HColors.lBg0,
    foreground: HColors.lFg0,
    cursor: HColors.lAccent,
    selection: HColorDerivation.tint(HColors.lAccent, HColors.fillPressedAlpha),
    normal: <Color>[
      HColors.lLineStrong,
      HColorDerivation.lightState(HColors.blocked),
      HColorDerivation.lightState(HColors.allowed),
      HColors.lAccent,
      HColorDerivation.lightState(_blue),
      HColorDerivation.lightState(HColors.passthrough),
      HColorDerivation.lightState(_cyan),
      HColors.lFg1,
    ],
    bright: <Color>[
      HColors.lFg2,
      HColorDerivation.darken(
        HColorDerivation.lightState(HColors.blocked),
        _brightStep,
      ),
      HColorDerivation.darken(
        HColorDerivation.lightState(HColors.allowed),
        _brightStep,
      ),
      HColorDerivation.darken(HColors.lAccent, _brightStep),
      HColorDerivation.darken(HColorDerivation.lightState(_blue), _brightStep),
      HColorDerivation.darken(
        HColorDerivation.lightState(HColors.passthrough),
        _brightStep,
      ),
      HColorDerivation.darken(HColorDerivation.lightState(_cyan), _brightStep),
      HColors.lFg0,
    ],
  );

  /// The palette of this brightness.
  static HTerminalPalette forBrightness(Brightness brightness) =>
      brightness == Brightness.dark ? dark : light;

  /// The sixteen colours in one list, normal first. For tests and the
  /// gallery.
  List<Color> get ramp => <Color>[...normal, ...bright];
}

/// Blau, nur für das Terminal. Seit HUM-355 trägt keine Farbe der Oberfläche
/// mehr Blau: der Akzent ist gelb, `held` eine Orchidee, `passthroughLlm`
/// violett. Ein Terminal, das `ESC [ 34 m` mit Violett beantwortete, verlöre
/// eine Unterscheidung, die der Agent gemacht hat. Gleiche Helligkeit wie die
/// übrigen Grundfarben, damit kein Ton lauter ist als ein anderer.
const Color _blue = Color(0xFF6F8FEF);

/// Cyan, nur für das Terminal: das Himmelscyan, das bis HUM-355 `held` war.
///
/// Es bleibt hier, weil `ESC [ 36 m` Cyan heißt und nicht Orchidee; ein
/// Agent, der Cyan schreibt, bekommt Cyan.
const Color _cyan = Color(0xFF79C9EC);

/// How far a bright colour moves from its base.
///
/// On the dark ladder up, on the light ladder down: "bright" means "further
/// from the surface", not "lighter". A light theme whose bright ramp went
/// lighter would answer `ESC [ 91 m` with something paler than the text
/// beside it.
const double _brightStep = 0.10;

Color _lift(Color color) => HColorDerivation.darken(color, -_brightStep);
