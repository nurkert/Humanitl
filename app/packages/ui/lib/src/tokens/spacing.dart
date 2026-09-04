import 'package:flutter/widgets.dart';

/// The spacing scale. Everything is a multiple of four.
abstract final class HSpace {
  /// The base unit. Nothing in the layout is not a multiple of it.
  static const double unit = 4;

  /// 4 px.
  static const double x1 = unit;

  /// 8 px.
  static const double x2 = unit * 2;

  /// 12 px, the panel padding.
  static const double x3 = unit * 3;

  /// 16 px.
  static const double x4 = unit * 4;

  /// 20 px.
  static const double x5 = unit * 5;

  /// 24 px.
  static const double x6 = unit * 6;

  /// 28 px.
  static const double x7 = unit * 7;

  /// 32 px.
  static const double x8 = unit * 8;

  /// The padding inside a panel.
  static const double panelPadding = x3;
}

/// Corner radii. Panels are square, controls are barely rounded.
abstract final class HRadius {
  /// Buttons, pills, inputs.
  static const double control = 4;

  /// Cards, modals, sheets.
  static const double card = 6;

  /// Panels: no radius at all.
  static const double panel = 0;

  /// Badges, the smallest rounded thing in the system.
  static const double badge = 2;

  /// [control] as a [BorderRadius].
  static const BorderRadius controlRadius = BorderRadius.all(
    Radius.circular(control),
  );

  /// [card] as a [BorderRadius].
  static const BorderRadius cardRadius = BorderRadius.all(
    Radius.circular(card),
  );

  /// [badge] as a [BorderRadius].
  static const BorderRadius badgeRadius = BorderRadius.all(
    Radius.circular(badge),
  );
}

/// Fixed sizes of the shell.
abstract final class HSize {
  /// Height of the window header.
  static const double headerBar = 40;

  /// Height of the status bar.
  static const double statusBar = 24;

  /// Height of a collapsed queue row.
  static const double row = 36;

  /// Höhe der Detailzeile in der History.
  ///
  /// Früher die Höhe einer ausgewählten Queue-Zeile mit Zweitzeile. Die
  /// Queue-Zeile wächst nicht mehr mit ihrem Zustand — 36 px in jedem Zustand,
  /// Mindesthöhe, keine feste (`docs/UX.md` 3.4) —, also ist der Wert auf die
  /// zweizeilige Detailzeile der History umgewidmet (`docs/UX.md` 9, Punkt 1).
  static const double rowSelected = 56;

  /// Zeilenhöhe der History-Tabelle.
  ///
  /// Eine der drei Dichten aus `docs/UX.md` 3.2 und wie die anderen beiden
  /// eine Mindesthöhe: bei `TextScaler.linear(2.0)` wächst die Zeile mit der
  /// Schrift. Ohne das Token schreibt der erste History-Screen eine 28 in eine
  /// Feature-Datei, und die Regel gegen Literale bricht an dem Dokument, das
  /// sie aufgestellt hat.
  static const double rowHistory = 28;

  /// Zeilenhöhe der Body- und Hex-Ansichten, die dichteste der drei Dichten.
  static const double rowBody = 24;

  /// Breite des Aktionsslots am rechten Rand einer Zeile.
  ///
  /// Immer reserviert, bei Ruhe leer; Hover **und** Fokus blenden dort die
  /// Aktion ein, ohne etwas zu verschieben (`docs/UX.md` 3.4). Ein eigenes
  /// Token, weil der Slot sich bisher [hitMin] borgt und das etwas anderes
  /// bedeutet: [hitMin] ist eine Untergrenze, der Slot eine feste Breite.
  static const double rowActionSlot = 28;

  /// The smallest hit target the design allows.
  static const double hitMin = 28;

  /// Die kleinste Fläche der beiden Entscheidungen: 120 px breit, 32 px hoch.
  ///
  /// [hitMin] ist die Untergrenze für Nebensächliches. Erlauben und Blockieren
  /// stehen darüber, weil ein 28-px-Ziel neben einem anderen 28-px-Ziel genau
  /// die Geometrie ist, in der ein hastiger Klick daneben geht — und daneben
  /// liegt hier die unumkehrbare Handlung (`docs/UX.md` 5.4 und 9).
  static const Size hitDecision = Size(120, 32);

  /// Minimum width of the queue pane.
  static const double paneMinQueue = 280;

  /// Minimum width of the inspector pane.
  static const double paneMinInspector = 480;

  /// Minimum width of the context pane.
  static const double paneMinContext = 260;

  /// Default width ratio of the three intercept panes, in percent.
  static const (int, int, int) paneRatio = (28, 44, 28);

  /// Width of the state rail on the left edge of a row.
  static const double stateRail = 4;

  /// Breite der zwei Pixel, die in der Icon-Rail die aktive Sektion markieren.
  ///
  /// Nicht mehr die Rail einer Zeile: dort **ersetzt** die Auswahl die
  /// Zustands-Rail über die vollen [stateRail] Pixel, statt ihre linke Hälfte
  /// zu überlagern (`docs/UX.md` 3.4 und 9, Punkt 2).
  static const double selectionRail = 2;

  /// Thickness of a hairline.
  static const double hairline = 1;

  /// Breite des Griffs eines Splitters.
  ///
  /// Ungerade und damit ausdrücklich nicht auf dem Vierer-Raster: das Raster
  /// ordnet Abstände, und dieser Wert ist eine Griff-Geometrie. Bei sieben
  /// Pixeln liegt die [hairline] in der Mitte auf einem ganzen Pixel; bei
  /// acht liefe sie über eine halbe Pixelgrenze und wäre auf einem Schirm
  /// ohne Skalierung sichtbar weicher als jede andere Haarlinie des Systems.
  static const double splitter = 7;

  /// Breite der Linie eines Splitters, während er gezogen wird.
  ///
  /// Ein Splitter ruht als Haarlinie und wird beim Ziehen doppelt so breit,
  /// damit der Griff unter dem Zeiger sichtbar bleibt. Ein Token, weil sonst
  /// jede zweite Achse (`docs/UX.md` 9, Punkt 30) ihre eigene 2 schreibt.
  static const double splitterActive = hairline * 2;

  /// Wie weit eine Pfeiltaste einen fokussierten Splitter verschiebt.
  ///
  /// Jeder Zeigerweg hat eine Taste (`docs/UX.md` 5.1); ein Ziehen um ein
  /// Pixel je Druck wäre keine. Zwei Rastereinheiten sind der kleinste
  /// Schritt, den man auf dem Schirm auch sieht.
  static const double splitterStep = HSpace.x2;

  /// Diameter of the state glyph in a row.
  static const double glyph = 16;

  /// Stroke width of the countdown ring around a state glyph.
  static const double ringStroke = 1.5;

  /// Das Textmaß: höchstens neunzig Monospace-Zeichen je Zeile.
  ///
  /// Eine Zeichenzahl und keine Pixelbreite, weil die Breite von der
  /// installierten Schrift abhängt (`docs/UX.md` 3.2 und 9, Punkt 7).
  /// Überschüssige Panebreite wird Rinne, nie Zeilenlänge; bei 2560 px liefe
  /// eine URL sonst über 137 Zeichen, und das Auge sucht den nächsten
  /// Zeilenanfang. Code, Hex und Tabellen bekommen kein Maß: sie scrollen
  /// waagerecht und brechen nie um.
  static const int measureChars = 90;

  /// Der Vorschub eines Monospace-Zeichens als Anteil der Schriftgröße.
  ///
  /// Illustration, nicht Norm: JetBrains Mono und jeder Fallback der Kette
  /// laufen auf rund 0,6 em. Wer aus [measureChars] eine Pixelbreite braucht,
  /// rechnet damit; normativ bleibt die Zeichenzahl (`docs/UX.md` 3.2).
  static const double monoAdvance = 0.6;

  /// [measureChars] in logischen Pixeln bei Schriftgröße [fontSize].
  static double measureWidth(double fontSize) =>
      measureChars * monoAdvance * fontSize;
}

/// Spacing as instance data, reachable from `HTokens.spacing`.
@immutable
class HSpacingTokens {
  /// Creates a spacing scale. Use [standard].
  const HSpacingTokens({
    this.unit = HSpace.unit,
    this.x1 = HSpace.x1,
    this.x2 = HSpace.x2,
    this.x3 = HSpace.x3,
    this.x4 = HSpace.x4,
    this.x5 = HSpace.x5,
    this.x6 = HSpace.x6,
    this.x7 = HSpace.x7,
    this.x8 = HSpace.x8,
  });

  /// The scale of the design direction.
  static const HSpacingTokens standard = HSpacingTokens();

  /// The base unit, 4.
  final double unit;

  /// 4 px.
  final double x1;

  /// 8 px.
  final double x2;

  /// 12 px.
  final double x3;

  /// 16 px.
  final double x4;

  /// 20 px.
  final double x5;

  /// 24 px.
  final double x6;

  /// 28 px.
  final double x7;

  /// 32 px.
  final double x8;
}

/// Radii as instance data, reachable from `HTokens.radii`.
@immutable
class HRadiusTokens {
  /// Creates a radius set. Use [standard].
  const HRadiusTokens({
    this.control = HRadius.control,
    this.card = HRadius.card,
    this.panel = HRadius.panel,
    this.badge = HRadius.badge,
  });

  /// The radii of the design direction.
  static const HRadiusTokens standard = HRadiusTokens();

  /// Controls.
  final double control;

  /// Cards, modals, sheets.
  final double card;

  /// Panels.
  final double panel;

  /// Badges.
  final double badge;
}

/// Shell sizes as instance data, reachable from `HTokens.sizes`.
@immutable
class HSizeTokens {
  /// Creates a size set. Use [standard].
  const HSizeTokens({
    this.headerBar = HSize.headerBar,
    this.statusBar = HSize.statusBar,
    this.row = HSize.row,
    this.rowSelected = HSize.rowSelected,
    this.rowHistory = HSize.rowHistory,
    this.rowBody = HSize.rowBody,
    this.rowActionSlot = HSize.rowActionSlot,
    this.measureChars = HSize.measureChars,
    this.hitMin = HSize.hitMin,
    this.hitDecision = HSize.hitDecision,
    this.paneMinQueue = HSize.paneMinQueue,
    this.paneMinInspector = HSize.paneMinInspector,
    this.paneMinContext = HSize.paneMinContext,
    this.paneRatio = HSize.paneRatio,
  });

  /// The sizes of the design direction.
  static const HSizeTokens standard = HSizeTokens();

  /// Height of the window header.
  final double headerBar;

  /// Height of the status bar.
  final double statusBar;

  /// Height of a collapsed row.
  final double row;

  /// Höhe der zweizeiligen Detailzeile der History.
  final double rowSelected;

  /// Zeilenhöhe der History-Tabelle, eine Mindesthöhe.
  final double rowHistory;

  /// Zeilenhöhe der Body-Ansichten, eine Mindesthöhe.
  final double rowBody;

  /// Breite des Aktionsslots am rechten Rand einer Zeile.
  final double rowActionSlot;

  /// Das Textmaß in Monospace-Zeichen.
  final int measureChars;

  /// Smallest allowed hit target.
  final double hitMin;

  /// Kleinste Fläche der beiden Entscheidungen.
  final Size hitDecision;

  /// Minimum width of the queue pane.
  final double paneMinQueue;

  /// Minimum width of the inspector pane.
  final double paneMinInspector;

  /// Minimum width of the context pane.
  final double paneMinContext;

  /// Default pane ratio in percent.
  final (int, int, int) paneRatio;
}
