/// Where the floating terminal sits and how large it is (HUM-300).
///
/// The floating terminal is a small window over the sections: the owner
/// watches the agent while deciding its requests. It snaps to one of nine
/// anchors of the sections (the corners, the middles of the edges and the
/// centre, HUM-352), keeps its anchor and its size across restarts in
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

/// Where the floating terminal snaps to: a corner, the middle of an edge, or
/// the centre of the sections (HUM-300, HUM-352).
///
/// Nine anchors on a grid of three by three. [x] and [y] are the column and
/// the row, each -1 (left or top), 0 (middle) or 1 (right or bottom). The
/// names are stored in the state file as they are; the four corners keep the
/// names they had before HUM-352, so a stored choice survives the update.
enum FloatAnchor {
  /// Top left.
  topLeft(-1, -1),

  /// The middle of the top edge.
  topCenter(0, -1),

  /// Top right.
  topRight(1, -1),

  /// The middle of the left edge.
  centerLeft(-1, 0),

  /// The centre of the sections.
  center(0, 0),

  /// The middle of the right edge.
  centerRight(1, 0),

  /// Bottom left.
  bottomLeft(-1, 1),

  /// The middle of the bottom edge.
  bottomCenter(0, 1),

  /// Bottom right: the default. On Intercept it lies over the bottom of the
  /// context pane, the one corner without a decision: the queue with its
  /// rows and notices fills the left, the bar with Allow and Block the bottom
  /// of the middle, and the notices of the shell stand top right.
  bottomRight(1, 1);

  const FloatAnchor(this.x, this.y);

  /// The column: -1 left, 0 middle, 1 right.
  final int x;

  /// The row: -1 top, 0 middle, 1 bottom.
  final int y;

  /// The anchor in column [x] and row [y], each clamped to the grid.
  static FloatAnchor at(int x, int y) {
    final int cx = x.clamp(-1, 1);
    final int cy = y.clamp(-1, 1);
    return values.firstWhere((FloatAnchor a) => a.x == cx && a.y == cy);
  }

  /// The corner a placement falls back from: the anchor itself for a
  /// corner, and for a middle the corner towards the right and the bottom,
  /// the side of the default.
  FloatAnchor get homeCorner => at(x == 0 ? 1 : x, y == 0 ? 1 : y);

  /// The anchor with the same row on the other side.
  FloatAnchor get acrossX => at(-x, y);

  /// The anchor in the same column at the other height.
  FloatAnchor get acrossY => at(x, -y);

  /// The column of the grip: across from the anchor, and on a middle column
  /// on the right (HUM-352). The grip sits in the corner of the window that
  /// faces away from the edge the window holds to; where the window holds to
  /// no edge on an axis, the grip takes the side of the default corner.
  int get gripX => x == 1 ? -1 : 1;

  /// The row of the grip: across from the anchor, and on a middle row at
  /// the bottom, by the same rule as [gripX].
  int get gripY => y == 1 ? -1 : 1;

  /// The anchor nearest to [window] in [region]: the one whose rectangle of
  /// the same size has the closest centre.
  static FloatAnchor nearest(Rect region, Rect window, double inset) {
    FloatAnchor best = bottomRight;
    double bestDistance = double.infinity;
    for (final FloatAnchor anchor in values) {
      final double distance =
          (terminalFloatRectAt(region, anchor, window.size, inset).center -
                  window.center)
              .distance;
      if (distance < bestDistance) {
        best = anchor;
        bestDistance = distance;
      }
    }
    return best;
  }

  /// Where a window let go at [window] with [velocity] (in logical pixels
  /// per second) snaps to (HUM-352).
  ///
  /// Below [terminalFloatFlingSpeed] the nearest anchor wins. Above it the
  /// throw counts more than the distance: from the nearest anchor one step
  /// on the grid in the direction of the throw, cut into eight directions,
  /// and never beyond the grid. A short throw to the left from bottom right
  /// thus lands bottom centre, not back where it started.
  static FloatAnchor release(
    Rect region,
    Rect window,
    double inset,
    Offset velocity,
  ) {
    final FloatAnchor near = nearest(region, window, inset);
    final double speed = velocity.distance;
    if (speed < terminalFloatFlingSpeed) {
      return near;
    }
    // Ein Achtel des Kreises je Richtung: Eine Achse zählt, sobald ihr Anteil
    // über sin(22,5°) liegt.
    const double share = 0.3827;
    int step(double component) =>
        component.abs() < speed * share ? 0 : component.sign.toInt();
    return at(near.x + step(velocity.dx), near.y + step(velocity.dy));
  }
}

/// The release speed, in logical pixels per second, from which a throw of
/// the window counts more than where it was let go (HUM-352). About a hand
/// that flicks the window a few centimetres; a calm drop stays below.
const double terminalFloatFlingSpeed = 400;

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

/// The key of the anchor in the state file. The name stays from the time of
/// four corners, so a stored corner survives HUM-352.
const String terminalFloatCornerKey = 'terminalFloatCorner';

/// The key of the size in the state file, as `[width, height]`.
const String terminalFloatSizeKey = 'terminalFloatSize';

/// The anchor and the size the person chose.
@immutable
class TerminalFloatGeometry {
  /// Creates the geometry.
  const TerminalFloatGeometry({
    this.anchor = FloatAnchor.bottomRight,
    this.size = terminalFloatDefaultSize,
  });

  /// Where the window snaps to while nothing is in the way.
  final FloatAnchor anchor;

  /// How large it is while the sections leave room for it.
  final Size size;

  @override
  bool operator ==(Object other) =>
      other is TerminalFloatGeometry &&
      other.anchor == anchor &&
      other.size == size;

  @override
  int get hashCode => Object.hash(anchor, size);
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

/// The size a window at [anchor] of [region] takes when the grip asks it to
/// grow [by] from [from] (HUM-352).
///
/// Within the limits of [clampTerminalFloatSize] for [region], the same at
/// every anchor. Shrinking is always taken. Growing is taken as far as the
/// window reaches into none of [keepClear]: when the whole step would, the
/// largest part of it that does not, so the window stops at the edge of such
/// a region and not one step of the pointer before it. A fast hand used to
/// lose the whole step there, and the window stopped short of Allow and
/// Block by however far the pointer had moved in that frame.
Size growTerminalFloat({
  required Rect region,
  required FloatAnchor anchor,
  required Size from,
  required Size by,
  required List<Rect> keepClear,
  required double inset,
}) {
  Size clamp(Size size) => clampTerminalFloatSize(size, region: region.size);
  final Size start = clamp(from);
  final Size wanted = clamp(
    Size(start.width + by.width, start.height + by.height),
  );
  bool free(Size size) {
    final Rect rect = terminalFloatRectAt(region, anchor, size, inset);
    return !keepClear.any((Rect zone) => zone.overlaps(rect));
  }

  final bool grows = wanted.width > start.width || wanted.height > start.height;
  if (!grows || free(wanted)) {
    return wanted;
  }
  if (!free(start)) {
    return start;
  }
  // Halbieren, bis die Grenze auf einen Bruchteil eines Pixels genau sitzt.
  double low = 0;
  double high = 1;
  for (int i = 0; i < 16; i++) {
    final double mid = (low + high) / 2;
    if (free(Size.lerp(start, wanted, mid)!)) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return Size.lerp(start, wanted, low)!;
}

/// The rectangle of a window of [size] at [anchor] of [region], [inset] from
/// the edges it holds to; on a middle axis centred in [region].
///
/// One rule for every anchor, and the only one: the room a window has is
/// [region] less [inset] on each side, whichever edge it holds to (HUM-352).
Rect terminalFloatRectAt(
  Rect region,
  FloatAnchor anchor,
  Size size,
  double inset,
) {
  double along(int side, double start, double end, double extent) =>
      switch (side) {
        -1 => start + inset,
        1 => end - inset - extent,
        _ => (start + end - extent) / 2,
      };
  return Offset(
        along(anchor.x, region.left, region.right, size.width),
        along(anchor.y, region.top, region.bottom, size.height),
      ) &
      size;
}

/// Where the floating terminal goes and the anchor it holds to.
typedef FloatPlacement = ({Rect rect, FloatAnchor anchor});

/// Where the floating terminal goes, or null when it fits nowhere.
///
/// The anchor the person chose comes first. Then the four corners, from the
/// corner of that anchor ([FloatAnchor.homeCorner]): the one beside it, the
/// one above or below it, and the one across. A middle is only ever taken
/// when chosen (HUM-352): giving way into the middle of the page would cover
/// the work it gives way to. At each anchor the chosen size is
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
    first: geometry.anchor,
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
/// The same anchors in the same order as [placeFloatingTerminal], from the
/// anchor the person chose, with the one [size] of the strip
/// (`HSize.terminalStrip`). The same rule as for the window: inside
/// [region], and over none of [keepClear].
FloatPlacement? placeTerminalStrip({
  required Rect region,
  required FloatAnchor anchor,
  required Size size,
  required List<Rect> keepClear,
  required double inset,
}) => _firstFree(
  region: region,
  first: anchor,
  sizes: <Size>[size],
  keepClear: keepClear,
  inset: inset,
);

/// The first rectangle of [sizes] at [first], then in the corners from its
/// home corner on, that lies inside [region] and overlaps none of
/// [keepClear].
FloatPlacement? _firstFree({
  required Rect region,
  required FloatAnchor first,
  required List<Size> sizes,
  required List<Rect> keepClear,
  required double inset,
}) {
  final FloatAnchor home = first.homeCorner;
  final List<FloatAnchor> anchors = <FloatAnchor>{
    first,
    home,
    home.acrossX,
    home.acrossY,
    home.acrossX.acrossY,
  }.toList();
  final Rect room = region.deflate(inset);
  for (final FloatAnchor anchor in anchors) {
    for (final Size size in sizes) {
      final Rect candidate = terminalFloatRectAt(region, anchor, size, inset);
      if (!_inside(room, candidate)) {
        continue;
      }
      if (keepClear.any((Rect zone) => zone.overlaps(candidate))) {
        continue;
      }
      return (rect: candidate, anchor: anchor);
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

/// The anchor and size of the floating terminal.
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
    final Object? anchor = stored[terminalFloatCornerKey];
    final Object? size = stored[terminalFloatSizeKey];
    return TerminalFloatGeometry(
      anchor: FloatAnchor.values.firstWhere(
        (FloatAnchor a) => a.name == anchor,
        orElse: () => FloatAnchor.bottomRight,
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

  /// Snaps to [anchor] and keeps it.
  void snapTo(FloatAnchor anchor) {
    if (anchor == state.anchor) {
      return;
    }
    state = TerminalFloatGeometry(anchor: anchor, size: state.size);
    ref.read(uiStateFileProvider).setValue(terminalFloatCornerKey, anchor.name);
  }

  /// Takes [size], within the limits, and keeps it.
  void resize(Size size) {
    final Size next = clampTerminalFloatSize(size);
    if (next == state.size) {
      return;
    }
    state = TerminalFloatGeometry(anchor: state.anchor, size: next);
    ref.read(uiStateFileProvider).setValue(terminalFloatSizeKey, <double>[
      next.width,
      next.height,
    ]);
  }
}
