// Was der Notification-Adapter wirklich auf die Leitung legt (HUM-118).
//
// Kein Session-Bus nötig: der Adapter bekommt einen Bus, der jeden Aufruf
// selbst beantwortet, statt einen Socket zu öffnen. Damit steht hier genau
// das, was `dbus` sonst serialisieren würde -- also auch, wie oft ein Wert
// verpackt ist. `DBusDict.stringVariant` verpackt jeden Wert selbst; ein
// bereits verpackter Wert kommt als Variant im Variant an, und jeder
// Notification-Server verwirft ihn stumm.
//
// Der Beweis auf echten Bytes steht in `dbus_live_test.dart`
// (`the_hints_reach_the_server_unwrapped`, `make flutter-test-dbus`); dieser
// Test hält dieselbe Aussage im normalen Lauf ohne Bus fest.

import 'package:dbus/dbus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/platform/dbus_notifications.dart';

/// Ein Bus, der den Notification-Server spielt, ohne einen Socket zu öffnen.
///
/// `DBusClient` verlangt eine Adresse, verbindet sich aber erst beim ersten
/// Aufruf. Da jeder Weg nach draußen -- auch `AddMatch` und `GetNameOwner`
/// der Signalströme -- durch das öffentliche [callMethod] führt, reicht es,
/// dieses eine Verfahren zu überschreiben.
class FakeNotificationBus extends DBusClient {
  /// Erzeugt einen Bus, dessen Server die Fähigkeiten [capabilities] meldet.
  FakeNotificationBus({this.capabilities = const <String>['actions', 'body']})
    : super(DBusAddress('unix:path=/nonexistent/humanitl-hum118'));

  /// Was `GetCapabilities` beantwortet.
  final List<String> capabilities;

  /// Die Argumente des letzten `Notify`, in der Reihenfolge der Leitung.
  List<DBusValue>? lastNotify;

  /// Die Signatur, die `Notify` bekommen hat, etwa `susssasa{sv}i`.
  String get notifySignature => (lastNotify ?? const <DBusValue>[])
      .map((DBusValue value) => value.signature.value)
      .join();

  /// Die Hints des letzten `Notify`, so wie der Server sie liest.
  Map<String, DBusValue> get hints {
    final DBusDict dict = lastNotify![6] as DBusDict;
    return dict.children.map(
      (DBusValue key, DBusValue value) =>
          MapEntry<String, DBusValue>((key as DBusString).value, value),
    );
  }

  @override
  Future<DBusMethodSuccessResponse> callMethod({
    String? destination,
    required DBusObjectPath path,
    String? interface,
    required String name,
    Iterable<DBusValue> values = const <DBusValue>[],
    DBusSignature? replySignature,
    bool noReplyExpected = false,
    bool noAutoStart = false,
    bool allowInteractiveAuthorization = false,
  }) async {
    switch (name) {
      case 'GetCapabilities':
        return DBusMethodSuccessResponse(<DBusValue>[
          DBusArray.string(capabilities),
        ]);
      case 'Notify':
        lastNotify = values.toList();
        return DBusMethodSuccessResponse(<DBusValue>[DBusUint32(17)]);
      case 'CloseNotification':
      case 'AddMatch':
      case 'RemoveMatch':
        return DBusMethodSuccessResponse();
      case 'GetNameOwner':
        return DBusMethodSuccessResponse(<DBusValue>[const DBusString(':1.7')]);
      default:
        throw DBusMethodResponseException(
          DBusMethodErrorResponse.unknownMethod(name),
        );
    }
  }
}

/// Eine Meldung, wie das Tray sie stellt.
const DesktopNotification message = DesktopNotification(
  flowId: FlowId('018f0034-0000-7000-8000-000000000001'),
  summary: 'api.github.com',
  body: 'GET /graphql\nabout 4 minutes left',
  actions: <NotificationAction>[
    NotificationAction(kind: NotificationActionKind.allow, label: 'Allow'),
    NotificationAction(kind: NotificationActionKind.block, label: 'Block'),
  ],
);

void main() {
  late FakeNotificationBus bus;
  late DBusNotificationPort port;

  setUp(() {
    bus = FakeNotificationBus();
    port = DBusNotificationPort(bus: bus);
  });

  tearDown(() async {
    await port.dispose();
    await bus.close();
  });

  test('notification_hints_are_wrapped_once', () async {
    await port.post(message);

    // Genau eine Verpackung. Mit einem `DBusVariant` im Aufruf stünde hier
    // `DBusVariant(DBusVariant(DBusByte(1)))`.
    expect(bus.hints['urgency'], const DBusVariant(DBusByte(1)));
    expect(
      bus.hints['desktop-entry'],
      const DBusVariant(DBusString('humanitl')),
    );
    // Und was der Server nach dem Auspacken des Variants findet, in den Typen
    // der Notifications-Spezifikation: ein Byte und eine Zeichenkette. Steht
    // dort `v`, weil zweimal verpackt wurde, oder `u`, weil die Dringlichkeit
    // als `DBusUint32` kam, verwirft er den Hint stumm.
    final DBusVariant urgency = bus.hints['urgency']! as DBusVariant;
    final DBusVariant entry = bus.hints['desktop-entry']! as DBusVariant;
    expect(urgency.value.signature.value, 'y');
    expect(entry.value.signature.value, 's');
  });

  test('notify_carries_the_arguments_of_the_specification', () async {
    await port.post(message);

    // Die äußere Form des Aufrufs und alles, was nicht Hint ist. Die
    // Doppelverpackung sieht dieser Test ausdrücklich **nicht**: `a{sv}`
    // bleibt `a{sv}`, ob im Variant ein `DBusByte` steht oder noch ein
    // `DBusVariant`. Den Inhalt der Hints bewacht
    // `notification_hints_are_wrapped_once` nebenan.
    final List<DBusValue> notify = bus.lastNotify!;
    expect(bus.notifySignature, 'susssasa{sv}i');
    expect(notify[0], const DBusString('Humanitl'));
    // Die erste Meldung ersetzt nichts; die Kennung vergibt der Server.
    expect(notify[1], DBusUint32(0));
    // Kein Symbolname: das Symbol kommt über den Hint `desktop-entry`.
    expect(notify[2], const DBusString(''));
    expect(notify[3], DBusString(message.summary));
    expect(notify[4], DBusString(message.body));
    // Die Knöpfe, abwechselnd Schlüssel und Beschriftung, in der Reihenfolge
    // der Meldung. Der Schlüssel trägt die Anfrage, auf die er wirkt.
    expect(
      (notify[5] as DBusArray).children
          .map((DBusValue value) => (value as DBusString).value)
          .toList(),
      <String>[
        'allow:${message.flowId.value}',
        'Allow',
        'block:${message.flowId.value}',
        'Block',
      ],
    );
    expect(notify[6].signature.value, 'a{sv}');
    // Die Standzeit bestimmt der Dienst, nicht dieses Programm.
    expect(notify[7], const DBusInt32(-1));
  });
}
