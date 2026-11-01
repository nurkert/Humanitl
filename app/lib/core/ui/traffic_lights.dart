/// The traffic lights of macOS follow the scale of the app (HUM-378).
///
/// The system draws them, so they do not scale with the app. The bar they sit
/// on does: at 150 % it is 60 pt high, and buttons centred on the 40 pt of
/// 100 % would hang at its upper edge. This reports the height the bar has in
/// the window; `MainFlutterWindow.swift` centres the buttons on it.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import 'ui.dart';
import 'ui_scale.dart';

/// The channel to the window of the Runner.
const MethodChannel windowChannel = MethodChannel('humanitl/window');

/// Reports the height of the header bar in points whenever the scale changes.
///
/// Draws nothing, and does nothing outside macOS.
class TrafficLightSync extends StatefulWidget {
  /// Creates the sync around [child].
  const TrafficLightSync({required this.child, super.key});

  /// Everything below.
  final Widget child;

  @override
  State<TrafficLightSync> createState() => _TrafficLightSyncState();
}

class _TrafficLightSyncState extends State<TrafficLightSync> {
  double? _sent;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (defaultTargetPlatform != TargetPlatform.macOS) {
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
  Widget build(BuildContext context) => widget.child;
}
