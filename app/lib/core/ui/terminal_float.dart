/// Where the floating terminal sits and how large it is (HUM-300).
///
/// The floating terminal is a small window over the sections: the owner
/// watches the agent while deciding its requests. It snaps to one of the four
/// corners of the sections, keeps its corner and its size across restarts in
/// the small state file of the interface (`UiStateFile`, HUM-254), and never
/// covers what must stay clickable: every region marked with `HKeepClear`
/// (the queue, the bar with Allow and Block, the notices, a sheet, a modal).
///
/// The rule that picks the place is a pure function, [placeFloatingTerminal],
/// so it can be measured without a widget; the shell's host only feeds it the
/// region, the regions to keep clear and the choice of the person.
library;

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../ui_state/ui_state_file.dart';

/// A corner of the sections.
enum FloatCorner {
  /// Top left.
  topLeft,

  /// Top right.
  topRight,

  /// Bottom left.
  bottomLeft,

  /// Bottom right: the default. On Intercept it lies over the bottom of the
  /// context pane, the one corner without a decision: the queue with its
  /// rows and notices fills the left, the bar with Allow and Block the bottom
  /// of the middle, and the notices of the shell stand top right.
  bottomRight;

  /// Whether the corner lies on the left edge.
  bool get left => this == topLeft || this == bottomLeft;

  /// Whether the corner lies on the top edge.
  bool get top => this == topLeft || this == topRight;

  /// The corner with the same height on the other side.
  FloatCorner get acrossX => switch (this) {
    topLeft => topRight,
    topRight => topLeft,
    bottomLeft => bottomRight,
    bottomRight => bottomLeft,
  };

  /// The corner on the same side at the other height.
  FloatCorner get acrossY => switch (this) {
    topLeft => bottomLeft,
    topRight => bottomRight,
    bottomLeft => topLeft,
    bottomRight => topRight,
  };

  /// The corner whose quadrant of [region] holds [point]: where a drag that
  /// lets go at [point] snaps to.
  static FloatCorner nearest(Rect region, Offset point) {
    final bool left = point.dx < region.center.dx;
    final bool top = point.dy < region.center.dy;
    return top
        ? (left ? topLeft : topRight)
        : (left ? bottomLeft : bottomRight);
  }
}

/// The smallest the floating terminal gets: about 38 columns and 9 rows of
/// `mono13` under its title bar and the banner, enough to see what the agent
/// does. Below that it is a thumbnail nobody can read.
const Size terminalFloatMinSize = Size(320, 200);

/// The size it has before anybody resized it.
const Size terminalFloatDefaultSize = Size(480, 300);

/// The largest it gets, whatever the window: above that it is no longer a
/// picture in the picture but a second layout.
const Size terminalFloatMaxSize = Size(960, 640);

/// The share of the sections it may take at most, in each direction, so the
/// page below stays a page.
const double terminalFloatMaxShare = 0.6;

/// The key of the corner in the state file.
const String terminalFloatCornerKey = 'terminalFloatCorner';

/// The key of the size in the state file, as `[width, height]`.
const String terminalFloatSizeKey = 'terminalFloatSize';

/// The corner and the size the person chose.
@immutable
class TerminalFloatGeometry {
  /// Creates the geometry.
  const TerminalFloatGeometry({
    this.corner = FloatCorner.bottomRight,
    this.size = terminalFloatDefaultSize,
  });

  /// Where the window snaps to while nothing is in the way.
  final FloatCorner corner;

  /// How large it is while the sections leave room for it.
  final Size size;

  @override
  bool operator ==(Object other) =>
      other is TerminalFloatGeometry &&
      other.corner == corner &&
      other.size == size;

  @override
  int get hashCode => Object.hash(corner, size);
}

/// The largest size the floating terminal may take in [region].
Size terminalFloatMaxIn(Size region) => Size(
  math.min(terminalFloatMaxSize.width, region.width * terminalFloatMaxShare),
  math.min(terminalFloatMaxSize.height, region.height * terminalFloatMaxShare),
);

/// [size] within the limits: never below [terminalFloatMinSize], and below
/// that only the maximum of [region] when one is given and larger than the
/// minimum.
Size clampTerminalFloatSize(Size size, {Size? region}) {
  final Size max = region == null
      ? terminalFloatMaxSize
      : terminalFloatMaxIn(region);
  double clampTo(double value, double low, double high) =>
      high < low ? low : value.clamp(low, high).toDouble();
  return Size(
    clampTo(size.width, terminalFloatMinSize.width, max.width),
    clampTo(size.height, terminalFloatMinSize.height, max.height),
  );
}

/// The rectangle of a window of [size] in [corner] of [region], [inset] from
/// its edges.
Rect terminalFloatRectAt(
  Rect region,
  FloatCorner corner,
  Size size,
  double inset,
) {
  final double left = corner.left
      ? region.left + inset
      : region.right - inset - size.width;
  final double top = corner.top
      ? region.top + inset
      : region.bottom - inset - size.height;
  return Offset(left, top) & size;
}

/// Where the floating terminal goes and the corner it holds to.
typedef FloatPlacement = ({Rect rect, FloatCorner corner});

/// Where the floating terminal goes, or null when it fits nowhere.
///
/// The corner the person chose comes first, then the one beside it, the one
/// above or below it, and the one across. In each corner the chosen size is
/// tried first, then the smallest. The first rectangle that lies inside
/// [region] and overlaps none of [keepClear] wins.
///
/// **Null is an answer, not a failure.** When no corner is free -- a modal
/// covers everything, a sheet and the queue take both sides --, the window
/// shrinks to its title strip ([placeTerminalStrip], HUM-315) or, without
/// room even for that, steps back until there is room again; the session and
/// its scrollback stay where they always live. Covering a decision would be
/// the failure.
FloatPlacement? placeFloatingTerminal({
  required Rect region,
  required TerminalFloatGeometry geometry,
  required List<Rect> keepClear,
  required double inset,
}) {
  final Size chosen = clampTerminalFloatSize(
    geometry.size,
    region: region.size,
  );
  return _firstFree(
    region: region,
    first: geometry.corner,
    sizes: <Size>[
      chosen,
      if (chosen != terminalFloatMinSize) terminalFloatMinSize,
    ],
    keepClear: keepClear,
    inset: inset,
  );
}

/// Where the slim title strip of the floating terminal goes while no corner
/// leaves room for the window, or null when not even the strip fits
/// (HUM-315).
///
/// The same corners in the same order as [placeFloatingTerminal], from the
/// corner the person chose, with the one [size] of the strip
/// (`HSize.terminalStrip`). The same rule as for the window: inside
/// [region], and over none of [keepClear].
FloatPlacement? placeTerminalStrip({
  required Rect region,
  required FloatCorner corner,
  required Size size,
  required List<Rect> keepClear,
  required double inset,
}) => _firstFree(
  region: region,
  first: corner,
  sizes: <Size>[size],
  keepClear: keepClear,
  inset: inset,
);

/// The first rectangle of [sizes] in the corners from [first] on that lies
/// inside [region] and overlaps none of [keepClear].
FloatPlacement? _firstFree({
  required Rect region,
  required FloatCorner first,
  required List<Size> sizes,
  required List<Rect> keepClear,
  required double inset,
}) {
  final List<FloatCorner> corners = <FloatCorner>[
    first,
    first.acrossX,
    first.acrossY,
    first.acrossX.acrossY,
  ];
  final Rect room = region.deflate(inset);
  for (final FloatCorner corner in corners) {
    for (final Size size in sizes) {
      final Rect candidate = terminalFloatRectAt(region, corner, size, inset);
      if (!_inside(room, candidate)) {
        continue;
      }
      if (keepClear.any((Rect zone) => zone.overlaps(candidate))) {
        continue;
      }
      return (rect: candidate, corner: corner);
    }
  }
  return null;
}

/// Whether [inner] lies within [outer], with a tolerance for rounding.
bool _inside(Rect outer, Rect inner) {
  const double slack = 0.5;
  return inner.left >= outer.left - slack &&
      inner.top >= outer.top - slack &&
      inner.right <= outer.right + slack &&
      inner.bottom <= outer.bottom + slack;
}

/// The corner and size of the floating terminal.
final NotifierProvider<TerminalFloat, TerminalFloatGeometry>
terminalFloatProvider = NotifierProvider<TerminalFloat, TerminalFloatGeometry>(
  TerminalFloat.new,
);

/// The notifier behind [terminalFloatProvider].
///
/// Read once and synchronously from the state file at the first build, like
/// the zoom of the terminal (`terminal_zoom.dart`): a window that appeared at
/// the default and jumped a frame later would resize the session twice. A
/// write that does not go through keeps the choice in memory for this run.
class TerminalFloat extends Notifier<TerminalFloatGeometry> {
  @override
  TerminalFloatGeometry build() {
    final Map<String, Object?> stored = ref.read(uiStateFileProvider).read();
    final Object? corner = stored[terminalFloatCornerKey];
    final Object? size = stored[terminalFloatSizeKey];
    return TerminalFloatGeometry(
      corner: FloatCorner.values.firstWhere(
        (FloatCorner c) => c.name == corner,
        orElse: () => FloatCorner.bottomRight,
      ),
      size: _size(size) ?? terminalFloatDefaultSize,
    );
  }

  static Size? _size(Object? value) {
    if (value is! List<Object?> || value.length != 2) {
      return null;
    }
    final Object? width = value[0];
    final Object? height = value[1];
    if (width is! num ||
        height is! num ||
        !width.isFinite ||
        !height.isFinite) {
      return null;
    }
    return clampTerminalFloatSize(Size(width.toDouble(), height.toDouble()));
  }

  /// Snaps to [corner] and keeps it.
  void snapTo(FloatCorner corner) {
    if (corner == state.corner) {
      return;
    }
    state = TerminalFloatGeometry(corner: corner, size: state.size);
    ref.read(uiStateFileProvider).setValue(terminalFloatCornerKey, corner.name);
  }

  /// Takes [size], within the limits, and keeps it.
  void resize(Size size) {
    final Size next = clampTerminalFloatSize(size);
    if (next == state.size) {
      return;
    }
    state = TerminalFloatGeometry(corner: state.corner, size: next);
    ref.read(uiStateFileProvider).setValue(terminalFloatSizeKey, <double>[
      next.width,
      next.height,
    ]);
  }
}
