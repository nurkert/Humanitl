// Was die beiden D-Bus-Adapter tun, wenn es gar keinen Bus gibt (HUM-034).
//
// Kein Session-Bus nötig und keiner erwünscht: die Adapter bekommen einen
// Client auf eine Adresse, die das `dbus`-Paket nicht öffnen kann. Genau da
// wirft es eine nackte Zeichenkette statt einer `Exception`, und genau da
// vergiftet es seinen eigenen Verbindungs-Completer.

import 'package:dbus/dbus.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/platform/dbus_notifications.dart';
import 'package:humanitl/features/tray/platform/sni_tray.dart';

/// Ein Client auf eine Adresse, deren Transport das Paket nicht kennt.
///
/// `autolaunch:` steht so in `DBUS_SESSION_BUS_ADDRESS` auf Systemen ohne
/// laufenden Bus. `dbus` wirft dafür `'D-Bus address transport not
/// supported: ...'` -- eine Zeichenkette, keine `Exception`.
DBusClient unopenableBus() => DBusClient(DBusAddress('autolaunch:'));

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

/// Eine Frist, unter der ein Aufruf zurück sein muss.
///
/// Ohne den Merker in [DBusNotificationPort] wartet der zweite Aufruf auf
/// eine Zusage, die nie eingelöst wird: der Test bliebe ohne Frist bis zum
/// Test-Timeout hängen, statt zu scheitern.
const Duration soon = Duration(seconds: 5);

void main() {
  group('the action keys on the wire', () {
    test('every_key_carries_the_request_it_acts_on', () {
      const FlowId id = FlowId('018f0034-0000-7000-8000-000000000001');
      // HUM-034 nennt `allow:<flowId>`. Ohne die Anfrage im Schlüssel trägt
      // ein Druck auf eine Meldung, die der Dienst nie ersetzt hat, nichts,
      // woran man sie erkennen könnte.
      expect(actionKey(NotificationActionKind.allow, id), 'allow:${id.value}');
      expect(actionKey(NotificationActionKind.block, id), 'block:${id.value}');
      expect(actionKey(NotificationActionKind.show, id), 'show:${id.value}');
    });

    test('a_key_reads_back_to_the_button_and_the_request', () {
      const FlowId id = FlowId('018f0034-0000-7000-8000-000000000007');
      for (final NotificationActionKind kind in NotificationActionKind.values) {
        expect(
          answerFromKey(actionKey(kind, id)),
          NotificationAnswer(kind: kind, flowId: id),
        );
      }
    });

    test('a_key_that_is_not_ours_reads_back_as_nothing', () {
      for (final String key in <String>[
        '',
        'default',
        'allow',
        'allow:',
        ':018f0034-0000-7000-8000-000000000001',
        'open:018f0034-0000-7000-8000-000000000001',
      ]) {
        expect(answerFromKey(key), isNull, reason: key);
      }
    });

    test('an_old_key_keeps_its_own_request', () {
      // Der Fall, um den es geht: die neue Meldung nennt eine Anfrage, die
      // alte Meldung steht daneben, und der Druck darauf trägt die alte.
      const FlowId old = FlowId('018f0034-0000-7000-8000-000000000001');
      const FlowId fresh = FlowId('018f0034-0000-7000-8000-000000000002');
      final NotificationAnswer? answer = answerFromKey(
        actionKey(NotificationActionKind.allow, old),
      );
      expect(answer!.flowId, old);
      expect(answer.flowId, isNot(fresh));
    });
  });

  test('a_transport_the_package_cannot_open_is_a_diagnostic', () async {
    final SniTrayPort tray = SniTrayPort(bus: unopenableBus());

    final Diagnostic? missing = await tray.start().timeout(soon);

    // Nicht ein Wurf, der durch den Post-Frame-Rückruf in die Zone fliegt und
    // die App stumm lässt, sondern der registrierte Code mit einer Ursache.
    expect(missing, isNotNull);
    expect(missing!.code, DiagnosticCodes.noTray);
    expect(missing.severity, Severity.info);
    expect(missing.why, isNotEmpty);
    expect(missing.fix, isNotNull);

    // Und ein Gesicht darauf zu zeichnen wirft auch nicht.
    await tray
        .show(
          const TrayFace(
            state: TrayIconState.held,
            count: 3,
            title: '3 requests held',
            detail: '',
            menuShow: 'Show Humanitl',
            menuQuit: 'Quit',
          ),
        )
        .timeout(soon);
    await tray.dispose().timeout(soon);
  });

  test('a_message_without_a_bus_is_swallowed_not_thrown', () async {
    final DBusNotificationPort port = DBusNotificationPort(
      bus: unopenableBus(),
    );

    await port.post(message).timeout(soon);

    await port.dispose().timeout(soon);
  });

  test('after_the_first_failure_post_and_withdraw_return_at_once', () async {
    final DBusNotificationPort port = DBusNotificationPort(
      bus: unopenableBus(),
    );

    // Der erste Aufruf scheitert am Socket. `dbus` setzt seinen
    // Verbindungs-Completer, bevor es den Socket öffnet, und schließt ihn bei
    // einem Wurf nie; jeder spätere Aufruf wartete auf dieselbe Zusage.
    await port.post(message).timeout(soon);
    await port.post(message).timeout(soon);
    await port.post(message).timeout(soon);
    await port.withdraw().timeout(soon);

    await port.dispose().timeout(soon);
  });
}
