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

  /// Height of a selected queue row, which carries a second line.
  static const double rowSelected = 56;

  /// The smallest hit target the design allows.
  static const double hitMin = 28;

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

  /// Width of the selection rail that replaces the state rail when selected.
  static const double selectionRail = 2;

  /// Thickness of a hairline.
  static const double hairline = 1;

  /// Diameter of the state glyph in a row.
  static const double glyph = 16;

  /// Stroke width of the countdown ring around a state glyph.
  static const double ringStroke = 1.5;
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
    this.hitMin = HSize.hitMin,
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

  /// Height of a selected row.
  final double rowSelected;

  /// Smallest allowed hit target.
  final double hitMin;

  /// Minimum width of the queue pane.
  final double paneMinQueue;

  /// Minimum width of the inspector pane.
  final double paneMinInspector;

  /// Minimum width of the context pane.
  final double paneMinContext;

  /// Default pane ratio in percent.
  final (int, int, int) paneRatio;
}
