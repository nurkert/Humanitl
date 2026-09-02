import 'package:flutter/widgets.dart';

/// Durations and curves. Motion explains state changes, it does not decorate.
abstract final class HMotion {
  /// Entry easing. Not [Curves.easeOut]; do not substitute.
  static const Cubic enter = Cubic(0.2, 0, 0, 1);

  /// Exit easing.
  static const Cubic exit = Cubic(0.4, 0, 1, 1);

  /// A request arrives in the queue: 8 px slide plus fade.
  static const Duration arrive = Duration(milliseconds: 180);

  /// A button fills on press.
  static const Duration press = Duration(milliseconds: 120);

  /// The state rail sweeps after a decision.
  static const Duration sweep = Duration(milliseconds: 200);

  /// A decided row collapses and glides out.
  static const Duration leave = Duration(milliseconds: 220);

  /// A newly created rule draws itself in the rule list.
  static const Duration ruleDraw = Duration(milliseconds: 240);

  /// One breath of the countdown glyph below twenty percent.
  static const Duration breathe = Duration(milliseconds: 1200);

  /// Holding the left half of the release valve confirms after this long.
  static const Duration holdToConfirm = Duration(milliseconds: 400);

  /// Below this fraction of the hold budget the countdown glyph breathes.
  static const double breatheBelow = 0.2;

  /// Vertical offset of an arriving row.
  static const double arriveOffset = 8;
}

/// Motion as instance data, reachable from `HTokens.motion`.
@immutable
class HMotionTokens {
  /// Creates a motion set. Use [standard].
  const HMotionTokens({
    this.enter = HMotion.enter,
    this.exit = HMotion.exit,
    this.arrive = HMotion.arrive,
    this.press = HMotion.press,
    this.sweep = HMotion.sweep,
    this.leave = HMotion.leave,
    this.ruleDraw = HMotion.ruleDraw,
    this.breathe = HMotion.breathe,
    this.holdToConfirm = HMotion.holdToConfirm,
  });

  /// The motion of the design direction.
  static const HMotionTokens standard = HMotionTokens();

  /// Entry easing.
  final Cubic enter;

  /// Exit easing.
  final Cubic exit;

  /// Arrival duration.
  final Duration arrive;

  /// Press duration.
  final Duration press;

  /// Rail sweep duration.
  final Duration sweep;

  /// Leave duration.
  final Duration leave;

  /// Rule draw duration.
  final Duration ruleDraw;

  /// Breathing period.
  final Duration breathe;

  /// Hold-to-confirm duration.
  final Duration holdToConfirm;
}
