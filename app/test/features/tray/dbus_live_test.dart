// Die beiden D-Bus-Adapter gegen einen echten Session-Bus (HUM-034).
//
// Nicht Teil des normalen Laufs: der Test braucht einen Session-Bus, den CI
// nicht hat, und der Notification-Teil schreibt auf den Schirm des Menschen,
// der ihn startet. Er läuft nur mit `HUMANITL_DBUS_TESTS=1 flutter test
// test/features/tray/dbus_live_test.dart` und ist damit das Werkzeug, mit dem
// die Protokollseite von Hand belegt wird.

import 'dart:io' show Platform;

import 'package:dbus/dbus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/platform/dbus_notifications.dart';
import 'package:humanitl/features/tray/platform/sni_tray.dart';

/// Ob dieser Lauf einen Session-Bus benutzen darf.
bool get busAllowed =>
    Platform.environment['HUMANITL_DBUS_TESTS'] == '1' &&
    (Platform.environment['DBUS_SESSION_BUS_ADDRESS'] ?? '').isNotEmpty;

/// Ein Wächter, wie ihn ein Panel bereitstellt: er merkt sich, wer sich
/// angemeldet hat.
class FakeWatcher extends DBusObject {
  FakeWatcher() : super(DBusObjectPath('/StatusNotifierWatcher'));

  /// Die Bus-Namen, die sich angemeldet haben.
  final List<String> registered = <String>[];

  @override
  Future<DBusMethodResponse> handleMethodCall(DBusMethodCall call) async {
    if (call.interface != 'org.kde.StatusNotifierWatcher') {
      return DBusMethodErrorResponse.unknownInterface();
    }
    if (call.name != 'RegisterStatusNotifierItem') {
      return DBusMethodErrorResponse.unknownMethod();
    }
    registered.add((call.values.first as DBusString).value);
    return DBusMethodSuccessResponse();
  }

  @override
  Future<DBusMethodResponse> getProperty(String interface, String name) async =>
      switch (name) {
        'ProtocolVersion' => DBusGetPropertyResponse(const DBusInt32(0)),
        'IsStatusNotifierHostRegistered' => DBusGetPropertyResponse(
          const DBusBoolean(true),
        ),
        'RegisteredStatusNotifierItems' => DBusGetPropertyResponse(
          DBusArray.string(registered),
        ),
        _ => DBusMethodErrorResponse.unknownProperty(),
      };
}

TrayFace faceWith(int count) => TrayFace(
  state: TrayIconState.held,
  count: count,
  title: '$count requests held',
  detail: '',
  menuShow: 'Show Humanitl',
  menuQuit: 'Quit',
);

void main() {
  test('the_tray_registers_and_answers_the_host', () async {
    final DBusClient host = DBusClient.session();
    final FakeWatcher watcher = FakeWatcher();
    await host.registerObject(watcher);
    await host.requestName('org.kde.StatusNotifierWatcher');

    final SniTrayPort tray = SniTrayPort();
    final Diagnostic? missing = await tray.start();
    expect(missing, isNull, reason: 'a watcher answers, so nothing is missing');
    // Andere Programme melden sich beim selben Wächter an, sobald es einen
    // gibt; gesucht ist der Name dieses Prozesses.
    final String name = watcher.registered.firstWhere(
      (String each) => each.startsWith('org.kde.StatusNotifierItem-'),
    );

    await tray.show(faceWith(3));

    // Was der Host liest, wenn er das Icon zeichnet.
    final DBusRemoteObject item = DBusRemoteObject(
      host,
      name: name,
      path: DBusObjectPath('/StatusNotifierItem'),
    );
    final Map<String, DBusValue> properties = await item.getAllProperties(
      'org.kde.StatusNotifierItem',
    );
    expect((properties['Status']! as DBusString).value, 'Active');
    expect((properties['Category']! as DBusString).value, 'ApplicationStatus');
    expect(
      (properties['Menu']! as DBusObjectPath).value,
      '/StatusNotifierItem/Menu',
    );
    final DBusArray pixmaps = properties['IconPixmap']! as DBusArray;
    expect(pixmaps.children, hasLength(2));
    final DBusStruct first = pixmaps.children.first as DBusStruct;
    expect((first.children.first as DBusInt32).value, 22);
    final DBusStruct tooltip = properties['ToolTip']! as DBusStruct;
    expect(
      (tooltip.children.toList()[2] as DBusString).value,
      '3 requests held',
    );

    // Und was er liest, wenn er das Menü öffnet.
    final DBusRemoteObject menu = DBusRemoteObject(
      host,
      name: name,
      path: DBusObjectPath('/StatusNotifierItem/Menu'),
    );
    final DBusMethodSuccessResponse layout = await menu.callMethod(
      'com.canonical.dbusmenu',
      'GetLayout',
      <DBusValue>[
        const DBusInt32(0),
        const DBusInt32(-1),
        DBusArray.string(const <String>[]),
      ],
      replySignature: DBusSignature('u(ia{sv}av)'),
    );
    final DBusStruct root = layout.returnValues[1] as DBusStruct;
    final DBusArray children = root.children.toList()[2] as DBusArray;
    expect(children.children, hasLength(4));

    await tray.dispose();
    await host.close();
  }, skip: !busAllowed);

  test('the_notification_reaches_a_server', () async {
    final DBusNotificationPort port = DBusNotificationPort();
    await port.post(
      const DesktopNotification(
        flowId: FlowId('018f0034-0000-7000-8000-000000000001'),
        summary: 'api.github.com',
        body: 'GET /graphql\nabout 4 minutes left',
        actions: <NotificationAction>[
          NotificationAction(
            kind: NotificationActionKind.block,
            label: 'Block',
          ),
          NotificationAction(kind: NotificationActionKind.show, label: 'Show'),
        ],
      ),
    );
    await port.withdraw();
    await port.dispose();
  }, skip: !busAllowed);
}
