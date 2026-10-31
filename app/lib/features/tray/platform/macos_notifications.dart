/// The notification port on macOS (HUM-373).
///
/// `UNUserNotificationCenter` sits in the Runner
/// (`macos/Runner/DesktopBridge.swift`), behind a method channel. Text and
/// buttons are the ones `notificationFor` words for Linux: the host as the
/// title, method, path and remaining time as the body.
///
/// Where Linux puts the request into each action key, macOS fixes the keys
/// with the button set. The request travels in the message instead and comes
/// back with the press, so a press still names the request it was about.
library;

import 'dart:async';

import 'package:flutter/services.dart';

import '../../../core/domain/domain.dart';
import '../desktop_ports.dart';

/// The channel to the Runner.
const String macNotificationChannel = 'humanitl/notifications';

/// Notifications over `UNUserNotificationCenter`.
///
/// A failure is swallowed like on D-Bus: the menu bar item and the Dock badge
/// carry the count, and a failed announcement is not worth an error screen.
class MacNotificationPort implements NotificationPort {
  /// Creates a port and starts listening to the Runner.
  MacNotificationPort([
    this._channel = const MethodChannel(macNotificationChannel),
  ]) {
    _channel.setMethodCallHandler(_fromRunner);
  }

  final MethodChannel _channel;
  final StreamController<NotificationAnswer> _actions =
      StreamController<NotificationAnswer>.broadcast();

  @override
  Stream<NotificationAnswer> get actions => _actions.stream;

  @override
  Future<void> post(DesktopNotification notification) =>
      _quietly('post', <String, Object>{
        'flowId': notification.flowId.value,
        'title': notification.summary,
        'body': notification.body,
        'actions': <Map<String, String>>[
          for (final NotificationAction action in notification.actions)
            <String, String>{'id': action.kind.name, 'label': action.label},
        ],
      });

  @override
  Future<void> withdraw() => _quietly('withdraw');

  @override
  Future<void> dispose() async {
    _channel.setMethodCallHandler(null);
    await _actions.close();
  }

  Future<void> _quietly(String method, [Object? arguments]) async {
    try {
      await _channel.invokeMethod<void>(method, arguments);
    } on PlatformException {
      // See the head of this class.
    } on MissingPluginException {
      // No Runner: a test binding.
    }
  }

  Future<void> _fromRunner(MethodCall call) async {
    final Object? arguments = call.arguments;
    if (call.method != 'action' || arguments is! Map || _actions.isClosed) {
      return;
    }
    final NotificationActionKind? kind = NotificationActionKind.values
        .asNameMap()[arguments['action']];
    final Object? flowId = arguments['flowId'];
    if (kind != null && flowId is String && flowId.isNotEmpty) {
      _actions.add(NotificationAnswer(kind: kind, flowId: FlowId(flowId)));
    }
  }
}
