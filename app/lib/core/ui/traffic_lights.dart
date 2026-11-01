/// The traffic lights of macOS follow the scale of the app (HUM-378) and the
/// full screen of the window (HUM-383).
///
/// The system draws them, so they do not scale with the app. The bar they sit
/// on does: at 150 % it is 60 pt high, and buttons centred on the 40 pt of
/// 100 % would hang at its upper edge. This reports the height the bar has in
/// the window; `MainFlutterWindow.swift` centres the buttons on it.
///
/// In full screen the system hides them until the pointer reaches the top
/// edge of the screen. The Runner reports where they stand
/// ([TrafficLightState]), and [TrafficLightRoom] carries how much of their
/// room the header bar leaves free: all of it in a window and while they are
/// revealed, none while they are hidden, gliding in between.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'ui.dart';
import 'ui_scale.dart';

/// The channel to the window of the Runner.
const MethodChannel windowChannel = MethodChannel('humanitl/window');

/// Where the traffic lights stand (HUM-383), in the words of the channel.
enum TrafficLightState {
  /// In a window, at the left end of the header bar.
  windowed,

  /// In full screen, hidden above the top edge of the screen.
  fullScreenHidden,

  /// In full screen, revealed because the pointer reached the top edge.
  fullScreenRevealed;

  /// The state [wire] names; null for a word this build does not know.
  static TrafficLightState? fromWire(Object? wire) {
    for (final TrafficLightState state in values) {
      if (state.name == wire) {
        return state;
      }
    }
    return null;
  }

  /// How much of the room of the traffic lights the header bar leaves free:
  /// 1 for all of it, 0 for none.
  double get room => this == fullScreenHidden ? 0 : 1;
}

/// Where the traffic lights stand, as the Runner last reported it.
///
/// Stays [TrafficLightState.windowed] outside macOS: nobody writes it there.
final NotifierProvider<TrafficLights, TrafficLightState>
trafficLightStateProvider = NotifierProvider<TrafficLights, TrafficLightState>(
  TrafficLights.new,
);

/// The notifier behind [trafficLightStateProvider].
class TrafficLights extends Notifier<TrafficLightState> {
  @override
  TrafficLightState build() => TrafficLightState.windowed;

  /// Uses [next].
  void set(TrafficLightState next) => state = next;
}

/// How much of the room of the traffic lights the header bar leaves free, as
/// it glides: 1 for all of it, 0 for none.
///
/// Read through [TrafficLightRoom.of]; without a [TrafficLightSync] above,
/// the whole room stays free, as before HUM-383.
class TrafficLightRoom extends InheritedWidget {
  /// Carries [room] down to [child].
  const TrafficLightRoom({required this.room, required super.child, super.key});

  /// The share of the room that stays free, between 0 and 1.
  final double room;

  /// The share under [context]; 1 without a [TrafficLightRoom] above.
  static double of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<TrafficLightRoom>()?.room ?? 1;

  @override
  bool updateShouldNotify(TrafficLightRoom oldWidget) => room != oldWidget.room;
}

/// Reports the height of the header bar in points whenever the scale changes,
/// hears where the traffic lights stand and lets their room glide.
///
/// Draws nothing, and does nothing outside macOS.
class TrafficLightSync extends ConsumerStatefulWidget {
  /// Creates the sync around [child].
  const TrafficLightSync({required this.child, super.key});

  /// Everything below.
  final Widget child;

  @override
  ConsumerState<TrafficLightSync> createState() => _TrafficLightSyncState();
}

class _TrafficLightSyncState extends ConsumerState<TrafficLightSync>
    with SingleTickerProviderStateMixin {
  double? _sent;

  /// Whether this state set the handler of [windowChannel]; it takes it back
  /// on dispose, whatever the platform reads by then.
  bool _listening = false;

  /// The share of the room that stays free. A glide and no confirmation, so
  /// under reduced motion it jumps ([HReducedMotion.displace]).
  late final AnimationController _room = AnimationController(
    vsync: this,
    value: ref.read(trafficLightStateProvider).room,
    duration: HMotion.arrive,
    animationBehavior: AnimationBehavior.preserve,
  );

  bool get _macOS => defaultTargetPlatform == TargetPlatform.macOS;

  @override
  void initState() {
    super.initState();
    ref.listenManual<TrafficLightState>(
      trafficLightStateProvider,
      (TrafficLightState? previous, TrafficLightState next) => _glide(next),
    );
    if (!_macOS) {
      return;
    }
    windowChannel.setMethodCallHandler(_heard);
    _listening = true;
    unawaited(_ask());
  }

  Future<Object?> _heard(MethodCall call) async {
    if (call.method != 'trafficLights') {
      throw MissingPluginException(call.method);
    }
    _apply(call.arguments);
    return null;
  }

  // A window restored in full screen entered it before Dart listened. Its
  // first frame stands where the traffic lights are; there is nothing to
  // glide from, so the room takes the answer without animation.
  Future<void> _ask() async {
    try {
      final Object? wire = await windowChannel.invokeMethod<Object?>(
        'getTrafficLights',
      );
      final TrafficLightState? state = TrafficLightState.fromWire(wire);
      if (state == null || !mounted) {
        return;
      }
      _room.value = state.room;
      _apply(wire);
    } on MissingPluginException {
      return;
    } on PlatformException {
      return;
    }
  }

  void _apply(Object? wire) {
    final TrafficLightState? state = TrafficLightState.fromWire(wire);
    if (state != null && mounted) {
      ref.read(trafficLightStateProvider.notifier).set(state);
    }
  }

  void _glide(TrafficLightState next) {
    if (!mounted) {
      return;
    }
    final Duration duration = HReducedMotion.displace(context, HMotion.arrive);
    if (duration == Duration.zero) {
      _room.value = next.room;
      return;
    }
    unawaited(
      _room.animateTo(next.room, duration: duration, curve: HMotion.enter),
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (!_macOS) {
      return;
    }
    final int scale =
        UiScaleScope.maybeOf(context)?.effective ?? uiScaleDefault;
    final double height = HSize.headerBar * scale / 100;
    if (height != _sent) {
      _sent = height;
      unawaited(_send(height));
    }
  }

  Future<void> _send(double height) async {
    try {
      await windowChannel.invokeMethod<void>('setHeaderBar', <String, Object>{
        'height': height,
      });
    } on MissingPluginException {
      return;
    }
  }

  @override
  void dispose() {
    if (_listening) {
      windowChannel.setMethodCallHandler(null);
    }
    _room.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
    animation: _room,
    builder: (BuildContext context, Widget? child) =>
        TrafficLightRoom(room: _room.value, child: child!),
    child: widget.child,
  );
}
