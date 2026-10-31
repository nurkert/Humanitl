/// The tray port on macOS (HUM-373): the menu bar item and the Dock badge.
///
/// The drawing sits in the Runner (`macos/Runner/DesktopBridge.swift`), behind
/// a method channel; everything it shows is decided here or above. The icon
/// is the one Linux shows (`tray_icon.dart`), so count and colours cannot
/// differ between the two.
///
/// What differs from StatusNotifierItem:
///
/// * A click opens the menu, as it does for every menu bar item on macOS.
///   `Show Humanitl` is its first entry.
/// * Registering cannot fail: macOS always has a menu bar. What can be
///   missing is the permission for notifications, and [start] asks for it,
///   because this is the one call whose answer reaches the notice.
library;

import 'dart:async';

import 'package:flutter/services.dart';

import '../../../core/domain/domain.dart';
import '../desktop_ports.dart';
import '../tray_diagnostics.dart';
import '../tray_icon.dart';

/// The channel to the Runner.
const String macTrayChannel = 'humanitl/tray';

/// The edge lengths of the icon in pixels: 18 points at one and two pixels
/// per point, which is what a menu bar has room for.
const List<int> macTrayIconSizes = <int>[18, 36];

/// The tray over `NSStatusItem`.
class MacTrayPort implements TrayPort {
  /// Creates a port and starts listening to the Runner.
  MacTrayPort([this._channel = const MethodChannel(macTrayChannel)]) {
    _channel.setMethodCallHandler(_fromRunner);
  }

  final MethodChannel _channel;
  final StreamController<TrayCommand> _commands =
      StreamController<TrayCommand>.broadcast();

  TrayFace? _drawn;

  @override
  Stream<TrayCommand> get commands => _commands.stream;

  /// Puts the item into the menu bar and asks for notifications.
  ///
  /// The Runner answers null when they may be shown, a sentence when the
  /// person refused them, and an error when the system could not be asked.
  /// This waits for the person's answer to the system dialog on a first run.
  @override
  Future<Diagnostic?> start() async {
    try {
      final String? denied = await _channel.invokeMethod<String>('start');
      return denied == null
          ? null
          : TrayDiagnostics.notificationsDenied(denied);
    } on PlatformException catch (error) {
      return TrayDiagnostics.notificationsFailed(error.message ?? '$error');
    } on MissingPluginException catch (error) {
      return TrayDiagnostics.notificationsFailed('$error');
    }
  }

  @override
  Future<void> show(TrayFace face) async {
    if (face == _drawn) {
      return;
    }
    _drawn = face;
    final List<TrayPixmap> pixmaps = await renderTrayIcon(
      state: face.state,
      count: face.count,
      sizes: macTrayIconSizes,
    );
    // Rendering takes a while, and a newer face may have been asked for in
    // the meantime. An older one arriving last would stand for good.
    if (face != _drawn || _commands.isClosed) {
      return;
    }
    try {
      await _channel.invokeMethod<void>('show', <String, Object>{
        'title': face.title,
        'tooltip': face.detail.isEmpty
            ? face.title
            : '${face.title}\n${face.detail}',
        'menuShow': face.menuShow,
        'menuQuit': face.menuQuit,
        // The label of the icon and not the bare count: `9+` above the cap,
        // and `?` while the daemon does not answer, because the number is
        // then unknown (`backlog/CONVENTIONS.md` 4.13).
        'badge': trayIconLabel(face.state, face.count),
        'pixmaps': <Map<String, Object>>[
          for (final TrayPixmap pixmap in pixmaps)
            <String, Object>{
              'width': pixmap.width,
              'height': pixmap.height,
              'argb': pixmap.argb,
            },
        ],
      });
    } on PlatformException {
      // The next face tries again; nothing here is worth an error on the
      // screen.
    } on MissingPluginException {
      // No Runner: a test binding.
    }
  }

  @override
  Future<void> dispose() async {
    _channel.setMethodCallHandler(null);
    await _commands.close();
  }

  Future<void> _fromRunner(MethodCall call) async {
    if (call.method != 'command' || _commands.isClosed) {
      return;
    }
    final TrayCommand? command = TrayCommand.values.asNameMap()[call.arguments];
    if (command != null) {
      _commands.add(command);
    }
  }
}
