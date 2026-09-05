/// The notification port on `org.freedesktop.Notifications` (HUM-034).
///
/// The desktop notification specification is a D-Bus interface, and this
/// talks to it directly: `Notify` with a `replaces_id`, `CloseNotification`,
/// and the two signals that come back. No plugin sits in between, so nothing
/// is added to `linux/flutter/generated_plugin_registrant.cc`.
///
/// Two honest limits of the protocol, both visible in the code below:
///
/// * The identifier of a notification is chosen by the **server**, not by us.
///   HUM-034 asks for a fixed `id = 1`; that is not something a client can
///   ask for. What the specification offers instead is `replaces_id`, and it
///   does the thing the fixed id was meant to do: one message that updates,
///   never a stack.
/// * Buttons only exist where the server announces the `actions` capability.
///   Notification daemons without it (several minimal ones) show the text and
///   nothing else. The message is therefore worded so that it is complete
///   without its buttons, and the buttons are left off rather than drawn as
///   something that would not work.
///
/// Two things about the `dbus` package decide how the failures below are
/// caught, and neither is obvious from its API:
///
/// * It throws bare strings, not exceptions -- `throw 'D-Bus address
///   transport not supported: ...'` is one of several. A `String` is not an
///   `Exception`, so `on Exception` lets exactly the failures through that
///   this file exists to swallow. Every catch here is therefore
///   `on Object catch`.
/// * It assigns its connection completer *before* it opens the socket and
///   never completes it when the socket throws. Every later call then waits
///   on a promise that will never be kept, so a client whose transport failed
///   once has to be left alone from then on: that is what [_broken] is for.
library;

import 'dart:async';

import 'package:dbus/dbus.dart';

import '../../../core/domain/domain.dart';
import '../desktop_ports.dart';

/// The bus name of the notification server.
const String _service = 'org.freedesktop.Notifications';

/// Its object path.
final DBusObjectPath _path = DBusObjectPath('/org/freedesktop/Notifications');

/// The name this program reports to the notification server.
const String _appName = 'Humanitl';

/// The desktop entry the server looks the icon up by.
const String _desktopEntry = 'humanitl';

/// Action verbs on the wire. They are not shown to anybody.
const Map<NotificationActionKind, String> _verbs =
    <NotificationActionKind, String>{
      NotificationActionKind.allow: 'allow',
      NotificationActionKind.block: 'block',
      NotificationActionKind.show: 'show',
    };

/// What separates the verb from the request in an action key.
const String _keySeparator = ':';

/// The key of one button: the verb and the request it acts on.
///
/// HUM-034 asks for `allow:<flowId>`, and the request in it is not decoration.
/// A server that ignores `replaces_id` leaves the previous popup standing, and
/// a press on that popup has to be answered for the request *it* named. With
/// the verb alone such a press would be read as being about the message that
/// stands now, or dropped without a word.
String actionKey(NotificationActionKind kind, FlowId flowId) =>
    '${_verbs[kind]}$_keySeparator${flowId.value}';

/// Reads an action key back, or null when it is not one of ours.
NotificationAnswer? answerFromKey(String key) {
  final int cut = key.indexOf(_keySeparator);
  if (cut <= 0 || cut == key.length - 1) {
    return null;
  }
  final String verb = key.substring(0, cut);
  for (final MapEntry<NotificationActionKind, String> entry in _verbs.entries) {
    if (entry.value == verb) {
      return NotificationAnswer(
        kind: entry.key,
        flowId: FlowId(key.substring(cut + 1)),
      );
    }
  }
  return null;
}

/// Notifications over D-Bus.
class DBusNotificationPort implements NotificationPort {
  /// Creates a port that talks to [bus].
  ///
  /// The caller owns [bus] when it passes one; otherwise the port opens the
  /// session bus itself and closes it again in [dispose].
  DBusNotificationPort({DBusClient? bus})
    : _bus = bus ?? DBusClient.session(),
      _ownsBus = bus == null;

  final DBusClient _bus;
  final bool _ownsBus;
  final StreamController<NotificationAnswer> _actions =
      StreamController<NotificationAnswer>.broadcast();

  StreamSubscription<DBusSignal>? _invoked;
  StreamSubscription<DBusSignal>? _closed;

  /// The id the server gave the message that stands; zero for none.
  int _id = 0;

  /// True once the transport failed and the client became unusable.
  ///
  /// `dbus` poisons its own connection completer when the socket cannot be
  /// opened (see the head of this file), so every call after the first
  /// failure would wait for ever. A program without a session bus must go on
  /// showing its window, not hang on every message it wanted to post.
  bool _broken = false;

  /// Whether the server announced that it can draw buttons; null until asked.
  ///
  /// Only an answer is remembered. A transport that could not be reached is
  /// no answer about buttons, and it travels out of [_serverDrawsButtons] to
  /// [post], which is the one place that decides the client is unusable.
  bool? _hasActions;

  @override
  Stream<NotificationAnswer> get actions => _actions.stream;

  @override
  Future<void> post(DesktopNotification notification) async {
    if (_broken) {
      return;
    }
    try {
      // The capability question first, and only then the subscription: it is
      // the one call that finds out whether this bus can be reached at all,
      // and a signal stream that is listened to on a bus that cannot be
      // opened poisons the client's connection completer for good.
      final bool buttons = await _serverDrawsButtons();
      await _listen();
      final List<DBusValue> pairs = <DBusValue>[];
      if (buttons) {
        for (final NotificationAction action in notification.actions) {
          pairs
            ..add(DBusString(actionKey(action.kind, notification.flowId)))
            ..add(DBusString(action.label));
        }
      }
      final DBusMethodSuccessResponse reply = await _bus.callMethod(
        destination: _service,
        path: _path,
        interface: _service,
        name: 'Notify',
        values: <DBusValue>[
          const DBusString(_appName),
          DBusUint32(_id),
          const DBusString(''),
          DBusString(notification.summary),
          DBusString(notification.body),
          DBusArray(DBusSignature('s'), pairs),
          // `DBusDict.stringVariant` wraps each value in a `DBusVariant`
          // itself, so the values handed to it are the bare ones. A
          // `DBusVariant` passed in here would arrive as a variant inside a
          // variant, which is a well-formed `a{sv}` and useless: every
          // notification server reads the inner type as `v`, finds no hint it
          // knows, and drops the entry without a word.
          DBusDict.stringVariant(<String, DBusValue>{
            // Normal urgency: this is worth knowing, it is not an emergency.
            // A byte, as the notification specification demands; a
            // `DBusUint32` would be discarded just as silently.
            'urgency': const DBusByte(1),
            'desktop-entry': const DBusString(_desktopEntry),
          }),
          // The server decides how long its own popups stand; a client that
          // dictates a timeout overrides a setting the person made.
          const DBusInt32(-1),
        ],
        replySignature: DBusSignature('u'),
      );
      _id = (reply.returnValues.first as DBusUint32).value;
    } on DBusMethodResponseException {
      // The server answered and refused. The bus works, so the next message
      // is worth trying; the tray and the window title carry the count in the
      // meantime, and a failed announcement is not worth an error screen.
    } on Object {
      // Not an answer but a broken transport: no session bus, an address this
      // package cannot open, a socket that went away. The client is unusable
      // from here on (see [_broken]).
      _broken = true;
    }
  }

  @override
  Future<void> withdraw() async {
    final int id = _id;
    if (_broken || id == 0) {
      return;
    }
    _id = 0;
    try {
      await _bus.callMethod(
        destination: _service,
        path: _path,
        interface: _service,
        name: 'CloseNotification',
        values: <DBusValue>[DBusUint32(id)],
        replySignature: DBusSignature(''),
      );
    } on DBusMethodResponseException {
      // Already gone.
    } on Object {
      _broken = true;
    }
  }

  @override
  Future<void> dispose() async {
    await _invoked?.cancel();
    await _closed?.cancel();
    await _actions.close();
    if (_ownsBus) {
      await _bus.close();
    }
  }

  /// Subscribes to the two signals, once.
  Future<void> _listen() async {
    if (_invoked != null) {
      return;
    }
    // `onError` is not decoration: a signal stream reports a transport that
    // cannot be opened through the stream, and an error without a handler
    // leaves the subscription and lands in the zone, where nobody is waiting
    // for it. There is nothing to do about it but go on without buttons.
    _invoked = DBusSignalStream(
      _bus,
      sender: _service,
      interface: _service,
      name: 'ActionInvoked',
      path: _path,
    ).listen(_onInvoked, onError: _signalFailed);
    _closed = DBusSignalStream(
      _bus,
      sender: _service,
      interface: _service,
      name: 'NotificationClosed',
      path: _path,
    ).listen(_onClosed, onError: _signalFailed);
  }

  /// Swallows a signal that could not be delivered.
  ///
  /// Nothing is remembered from it: the failure may be the transport, and
  /// then [post] has already noticed, or a single malformed signal, and then
  /// the next one is fine. Either way the only thing on offer is a message
  /// without its buttons.
  void _signalFailed(Object error) {}

  void _onInvoked(DBusSignal signal) {
    if (signal.values.length < 2) {
      return;
    }
    final DBusValue key = signal.values[1];
    if (key is! DBusString) {
      return;
    }
    // The identifier of the message is deliberately not compared. `ActionInvoked`
    // is broadcast for every notification the server carries, and the key is
    // what says whose button it was: it holds one of our three verbs and the
    // request. Comparing the identifier as well would drop exactly the press
    // this key exists for -- the one on a popup the server never replaced.
    final NotificationAnswer? answer = answerFromKey(key.value);
    if (answer != null && !_actions.isClosed) {
      _actions.add(answer);
    }
  }

  void _onClosed(DBusSignal signal) {
    if (signal.values.isEmpty) {
      return;
    }
    final DBusValue id = signal.values.first;
    if (id is DBusUint32 && id.value == _id) {
      // The message is gone; the next one starts fresh rather than replacing
      // an identifier the server has already forgotten.
      _id = 0;
    }
  }

  Future<bool> _serverDrawsButtons() async {
    final bool? known = _hasActions;
    if (known != null) {
      return known;
    }
    try {
      final DBusMethodSuccessResponse reply = await _bus.callMethod(
        destination: _service,
        path: _path,
        interface: _service,
        name: 'GetCapabilities',
        values: const <DBusValue>[],
        replySignature: DBusSignature('as'),
      );
      final DBusArray capabilities = reply.returnValues.first as DBusArray;
      final bool draws = capabilities.children.whereType<DBusString>().any(
        (DBusString value) => value.value == 'actions',
      );
      _hasActions = draws;
      return draws;
    } on DBusMethodResponseException {
      // The server answered and does not offer the question. It draws no
      // buttons, and it will not start to; the message goes out without them.
      _hasActions = false;
      return false;
    }
  }
}
