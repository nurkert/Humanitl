// Die beiden D-Bus-Adapter gegen einen echten Session-Bus (HUM-034, HUM-118).
//
// Nicht Teil des normalen Laufs: der Test braucht einen Session-Bus, den CI
// nicht hat, und der Notification-Teil schreibt auf den Schirm des Menschen,
// der ihn startet. Er läuft nur mit `HUMANITL_DBUS_TESTS=1 flutter test
// test/features/tray/dbus_live_test.dart` und ist damit das Werkzeug, mit dem
// die Protokollseite von Hand belegt wird.
//
// Zwei Arten von Sitzung, und jeder Test gehört genau einer davon (HUM-118).
// Auf einem privaten Bus (`make flutter-test-dbus`, also `dbus-run-session`)
// sind die beiden Namen `org.kde.StatusNotifierWatcher` und
// `org.freedesktop.Notifications` frei, und die Attrappen dieses Tests halten
// sie selbst: dort wird gemessen, was auf der Leitung steht. Auf dem Bus eines
// echten Desktops hält sie das Panel beziehungsweise der Meldungsdienst, und
// gemessen werden kann nichts; dort bleibt nur die Meldung auf dem Schirm, die
// ein Mensch ansieht. Deshalb prüft jeder Test das Ergebnis von `requestName`,
// bevor er irgendetwas anmeldet, und überspringt sich mit Grund, statt sich
// beim Wächter des echten Desktops einzutragen und auf dessen Panel zu messen.

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

/// Nimmt den Bus-Namen [name] für [bus] in Besitz, oder meldet, dass er
/// vergeben ist.
///
/// Ohne diese Frage misst der Test am falschen Gegenüber. `requestName`
/// beantwortet einen vergebenen Namen nicht mit einem Wurf, sondern mit
/// `DBusRequestNameReply.inQueue`; wer das Ergebnis wegwirft, meldet sich
/// anschließend beim Wächter des echten Desktops an und sucht dessen Antwort
/// in seiner eigenen Attrappe. `doNotQueue` hält den Prozess außerdem aus der
/// Warteschlange, so dass er den Namen nicht später übernimmt, wenn das Panel
/// des Menschen ihn abgibt.
Future<bool> claim(DBusClient bus, String name) async {
  final DBusRequestNameReply reply = await bus.requestName(
    name,
    flags: <DBusRequestNameFlag>{DBusRequestNameFlag.doNotQueue},
  );
  return reply == DBusRequestNameReply.primaryOwner ||
      reply == DBusRequestNameReply.alreadyOwner;
}

/// Was ein übersprungener Test über den Namen [name] sagt.
String taken(String name) =>
    'another program already owns $name on this session bus; '
    'run under dbus-run-session to measure the protocol';

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

/// Der Bus-Name des Meldungsdienstes.
const String notifications = 'org.freedesktop.Notifications';

/// Ein Meldungsdienst, wie ihn ein Desktop bereitstellt: er merkt sich, was
/// von der Leitung kam.
class FakeNotificationServer extends DBusObject {
  /// Erzeugt den Dienst unter dem Pfad, den die Spezifikation nennt.
  FakeNotificationServer()
    : super(DBusObjectPath('/org/freedesktop/Notifications'));

  /// Die Argumente des letzten `Notify`, gelesen aus den Bytes des Busses.
  List<DBusValue>? lastNotify;

  @override
  Future<DBusMethodResponse> handleMethodCall(DBusMethodCall call) async {
    if (call.interface != notifications) {
      return DBusMethodErrorResponse.unknownInterface();
    }
    switch (call.name) {
      case 'GetCapabilities':
        return DBusMethodSuccessResponse(<DBusValue>[
          DBusArray.string(const <String>['actions', 'body']),
        ]);
      case 'Notify':
        lastNotify = call.values;
        return DBusMethodSuccessResponse(<DBusValue>[DBusUint32(23)]);
      case 'CloseNotification':
        return DBusMethodSuccessResponse();
      default:
        return DBusMethodErrorResponse.unknownMethod();
    }
  }
}

/// Die Meldung, die beide Notification-Tests stellen.
const DesktopNotification liveMessage = DesktopNotification(
  flowId: FlowId('018f0034-0000-7000-8000-000000000001'),
  summary: 'api.github.com',
  body: 'GET /graphql\nabout 4 minutes left',
  actions: <NotificationAction>[
    NotificationAction(kind: NotificationActionKind.block, label: 'Block'),
    NotificationAction(kind: NotificationActionKind.show, label: 'Show'),
  ],
);

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
    // Vor jeder Anmeldung: gehört der Name dem Panel des Menschen, meldet
    // sich `SniTrayPort` gleich dort an und diese Attrappe bleibt leer.
    if (!await claim(host, 'org.kde.StatusNotifierWatcher')) {
      await host.close();
      markTestSkipped(taken('org.kde.StatusNotifierWatcher'));
      return;
    }
    final FakeWatcher watcher = FakeWatcher();
    await host.registerObject(watcher);

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

  // Der Handlauf: auf dem Bus eines echten Desktops steht die Meldung auf dem
  // Schirm, und ein Mensch sieht sie an. Auf einem privaten Bus gibt es keinen
  // Dienst, der sie zeichnen könnte; dort übernimmt der Test darunter.
  test('the_notification_reaches_a_server', () async {
    final DBusClient probe = DBusClient.session();
    final String? server = await probe.getNameOwner(notifications);
    await probe.close();
    if (server == null) {
      markTestSkipped(
        'no notification server on this bus, so nothing draws the message; '
        'the_hints_reach_the_server_unwrapped measures the protocol instead',
      );
      return;
    }

    final DBusNotificationPort port = DBusNotificationPort();
    await port.post(liveMessage);
    await port.withdraw();
    await port.dispose();
  }, skip: !busAllowed);

  // Der Beweis auf echten Bytes: der Aufruf geht durch den Bus-Daemon, wird
  // dort serialisiert und hier wieder gelesen. Was diese Attrappe sieht, ist
  // das, was jeder Notification-Server sieht.
  test('the_hints_reach_the_server_unwrapped', () async {
    final DBusClient host = DBusClient.session();
    if (!await claim(host, notifications)) {
      await host.close();
      markTestSkipped(taken(notifications));
      return;
    }
    final FakeNotificationServer server = FakeNotificationServer();
    await host.registerObject(server);

    final DBusNotificationPort port = DBusNotificationPort();
    await port.post(liveMessage);
    await port.withdraw();
    await port.dispose();

    expect(
      server.lastNotify,
      isNotNull,
      reason:
          'the port swallows every failure, so a message that never '
          'arrived looks the same as one that was never sent',
    );
    final List<DBusValue> notify = server.lastNotify!;
    // Die Signatur der Notifications-Spezifikation, Argument für Argument,
    // und der Text, den der Mensch zu sehen bekäme.
    expect(
      notify.map((DBusValue value) => value.signature.value).join(),
      'susssasa{sv}i',
    );
    expect(notify[3], DBusString(liveMessage.summary));
    expect(notify[4], DBusString(liveMessage.body));
    final DBusDict hints = notify[6] as DBusDict;
    expect(hints.signature.value, 'a{sv}');
    // Eine Verpackung, nicht zwei: was im Variant steht, ist ein Byte und eine
    // Zeichenkette. Mit einem `DBusVariant` im Aufruf stünde hier `v`, und der
    // Server verwürfe beide Hints stumm.
    final DBusVariant urgency =
        hints.children[const DBusString('urgency')]! as DBusVariant;
    final DBusVariant entry =
        hints.children[const DBusString('desktop-entry')]! as DBusVariant;
    expect(urgency.value.signature.value, 'y');
    expect(entry.value.signature.value, 's');
    expect(urgency.value, const DBusByte(1));
    expect(entry.value, const DBusString('humanitl'));

    await host.close();
  }, skip: !busAllowed);
}
