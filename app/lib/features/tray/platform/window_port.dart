/// The window port on `window_manager` (HUM-034).
///
/// The plugin is already a dependency of this app; it carries the title, the
/// minimum size and the two events this feature turns on: the window came to
/// the front, the window left it.
library;

import 'dart:async';

import 'package:flutter/services.dart' show MissingPluginException;
import 'package:window_manager/window_manager.dart';

import '../desktop_ports.dart';

/// The real window.
class WindowManagerPort with WindowListener implements WindowPort {
  /// Registers the listener with `window_manager`.
  WindowManagerPort() {
    windowManager.addListener(this);
  }

  final StreamController<bool> _focus = StreamController<bool>.broadcast();

  @override
  Stream<bool> get focus => _focus.stream;

  @override
  void onWindowFocus() => _emit(true);

  @override
  void onWindowBlur() => _emit(false);

  void _emit(bool focused) {
    if (!_focus.isClosed) {
      _focus.add(focused);
    }
  }

  @override
  Future<void> setTitle(String title) =>
      _quietly(windowManager.setTitle(title));

  @override
  Future<void> reveal() async {
    await _quietly(windowManager.show());
    await _quietly(windowManager.focus());
  }

  @override
  Future<void> quit() => _quietly(windowManager.destroy());

  @override
  Future<void> dispose() async {
    windowManager.removeListener(this);
    await _focus.close();
  }

  /// Swallows the one failure that is not a defect: no native window at all,
  /// which is what a test binding or an unsupported platform looks like.
  static Future<void> _quietly(Future<void> call) async {
    try {
      await call;
    } on MissingPluginException {
      // No native window.
    }
  }
}
