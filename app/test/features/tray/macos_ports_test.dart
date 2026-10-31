// Die macOS-Adapter gegen einen falschen Kanal (HUM-373): Was beim Runner
// ankommt und was von ihm zurückkommt. Das Swift darunter prüft nur ein Mensch
// an einem Mac.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/platform/macos_notifications.dart';
import 'package:humanitl/features/tray/platform/macos_tray.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';

const FlowId flow = FlowId('018f0034-0000-7000-8000-000000000001');

const DesktopNotification message = DesktopNotification(
  flowId: flow,
  summary: 'api.github.com',
  body: 'GET /graphql\nabout 5 minutes left · + 2 more requests',
  actions: <NotificationAction>[
    NotificationAction(kind: NotificationActionKind.allow, label: 'Allow'),
    NotificationAction(kind: NotificationActionKind.block, label: 'Block'),
    NotificationAction(kind: NotificationActionKind.show, label: 'Show'),
  ],
);

TrayFace face({
  TrayIconState state = TrayIconState.held,
  int count = 3,
  String title = '3 requests held',
  String detail = '',
}) => TrayFace(
  state: state,
  count: count,
  title: title,
  detail: detail,
  menuShow: 'Show Humanitl',
  menuQuit: 'Quit',
);

/// Ein Runner, der jeden Aufruf mitschreibt und auf [answer] antwortet.
class FakeRunner {
  FakeRunner(this.name, {this.answer}) {
    messenger.setMockMethodCallHandler(MethodChannel(name), (
      MethodCall call,
    ) async {
      calls.add(call);
      return answer?.call(call);
    });
  }

  static TestDefaultBinaryMessenger get messenger =>
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  final String name;
  final Object? Function(MethodCall call)? answer;
  final List<MethodCall> calls = <MethodCall>[];

  /// Schickt einen Aufruf des Runners an die Dart-Seite.
  Future<void> send(String method, Object? arguments) {
    final Completer<void> done = Completer<void>();
    messenger.handlePlatformMessage(
      name,
      const StandardMethodCodec().encodeMethodCall(
        MethodCall(method, arguments),
      ),
      (ByteData? _) => done.complete(),
    );
    return done.future;
  }

  void remove() =>
      messenger.setMockMethodCallHandler(MethodChannel(name), null);
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('MacTrayPort', () {
    late FakeRunner runner;
    late MacTrayPort tray;

    Future<void> startWith(Object? Function(MethodCall) answer) async {
      runner = FakeRunner(macTrayChannel, answer: answer);
      tray = MacTrayPort();
      addTearDown(() async {
        await tray.dispose();
        runner.remove();
      });
    }

    test('a_granted_permission_raises_nothing', () async {
      await startWith((_) => null);
      expect(await tray.start(), isNull);
      expect(runner.calls.single.method, 'start');
    });

    test(
      'a_refused_permission_is_ui_002_with_the_way_to_the_settings',
      () async {
        await startWith((_) => 'denied in System Settings');
        final Diagnostic? missing = await tray.start();

        expect(missing, isNotNull);
        expect(missing!.code, DiagnosticCodes.noTray);
        expect(missing.severity, Severity.info);
        expect(missing.title, TrayDiagnostics.notificationsDeniedTitle);
        expect(missing.why, 'denied in System Settings');
        // Nicht der Fix für GNOME: Der führt auf macOS ins Leere.
        expect(
          missing.fix,
          const FixAction.copyCommand(
            command: TrayDiagnostics.notificationSettingsCommand,
          ),
        );
      },
    );

    test('a_failing_system_is_not_reported_as_a_refusal', () async {
      await startWith(
        (_) => throw PlatformException(
          code: 'notifications',
          message: 'Notifications are not allowed for this application',
        ),
      );
      final Diagnostic? failed = await tray.start();

      expect(failed?.code, DiagnosticCodes.noTray);
      expect(failed?.title, TrayDiagnostics.notificationsFailedTitle);
      expect(failed?.why, 'Notifications are not allowed for this application');
      expect(failed?.fix, isNull);
    });

    test(
      'a_face_reaches_the_runner_with_menu_tooltip_badge_and_icon',
      () async {
        await startWith((_) => null);
        await tray.show(
          face(state: TrayIconState.alert, detail: '1 request was blocked'),
        );

        final MethodCall call = runner.calls.single;
        expect(call.method, 'show');
        final Map<Object?, Object?> sent =
            call.arguments as Map<Object?, Object?>;
        expect(sent['title'], '3 requests held');
        expect(sent['tooltip'], '3 requests held\n1 request was blocked');
        expect(sent['menuShow'], 'Show Humanitl');
        expect(sent['menuQuit'], 'Quit');
        expect(sent['badge'], '3');

        final List<Object?> pixmaps = sent['pixmaps']! as List<Object?>;
        expect(pixmaps, hasLength(macTrayIconSizes.length));
        for (int i = 0; i < pixmaps.length; i++) {
          final Map<Object?, Object?> pixmap =
              pixmaps[i]! as Map<Object?, Object?>;
          final int size = macTrayIconSizes[i];
          expect(pixmap['width'], size);
          expect(pixmap['height'], size);
          expect((pixmap['argb']! as Uint8List).length, size * size * 4);
        }
      },
    );

    test('the_badge_says_what_the_icon_says', () async {
      await startWith((_) => null);
      Object? badge() =>
          (runner.calls.last.arguments as Map<Object?, Object?>)['badge'];

      await tray.show(face(state: TrayIconState.idle, count: 0, title: 'open'));
      expect(badge(), '');

      await tray.show(face(count: 12, title: '12 held'));
      expect(badge(), '9+');

      // Unbekannt, nicht null: Die letzte Zahl stehen zu lassen wäre gelogen.
      await tray.show(
        face(state: TrayIconState.offline, count: 0, title: 'gone'),
      );
      expect(badge(), '?');
    });

    test('the_same_face_is_not_drawn_twice', () async {
      await startWith((_) => null);
      await tray.show(face());
      await tray.show(face());
      expect(runner.calls, hasLength(1));
    });

    test('of_two_faces_in_a_row_the_newer_one_stands', () async {
      await startWith((_) => null);
      await Future.wait(<Future<void>>[
        tray.show(face(count: 1, title: 'one')),
        tray.show(face(count: 2, title: 'two')),
      ]);
      expect(
        (runner.calls.last.arguments as Map<Object?, Object?>)['title'],
        'two',
      );
      expect(runner.calls, hasLength(1));
    });

    test('a_show_without_a_runner_does_not_throw', () async {
      tray = MacTrayPort();
      await tray.show(face());
      await tray.dispose();
    });

    test('menu_entries_arrive_as_commands', () async {
      await startWith((_) => null);
      final List<TrayCommand> seen = <TrayCommand>[];
      final StreamSubscription<TrayCommand> listening = tray.commands.listen(
        seen.add,
      );
      addTearDown(listening.cancel);

      await runner.send('command', 'show');
      await runner.send('command', 'quit');
      await runner.send('command', 'reboot');
      await pumpEventQueue();

      expect(seen, <TrayCommand>[TrayCommand.show, TrayCommand.quit]);
    });
  });

  group('MacNotificationPort', () {
    late FakeRunner runner;
    late MacNotificationPort port;

    setUp(() {
      runner = FakeRunner(macNotificationChannel);
      port = MacNotificationPort();
    });

    tearDown(() async {
      await port.dispose();
      runner.remove();
    });

    test('a_message_carries_the_words_and_buttons_of_linux', () async {
      await port.post(message);

      final MethodCall call = runner.calls.single;
      expect(call.method, 'post');
      expect(call.arguments, <String, Object>{
        'flowId': flow.value,
        'title': 'api.github.com',
        'body': 'GET /graphql\nabout 5 minutes left · + 2 more requests',
        'actions': <Map<String, String>>[
          <String, String>{'id': 'allow', 'label': 'Allow'},
          <String, String>{'id': 'block', 'label': 'Block'},
          <String, String>{'id': 'show', 'label': 'Show'},
        ],
      });
    });

    test('a_message_without_allow_offers_no_allow', () async {
      await port.post(
        DesktopNotification(
          flowId: flow,
          summary: message.summary,
          body: message.body,
          actions: message.actions.sublist(1),
        ),
      );
      final Map<Object?, Object?> sent =
          runner.calls.single.arguments as Map<Object?, Object?>;
      final List<Object?> actions = sent['actions']! as List<Object?>;
      expect(
        actions.map((Object? a) => (a! as Map<Object?, Object?>)['id']),
        <String>['block', 'show'],
      );
    });

    test('withdraw_reaches_the_runner', () async {
      await port.withdraw();
      expect(runner.calls.single.method, 'withdraw');
    });

    test('a_failing_runner_does_not_throw', () async {
      runner.remove();
      runner = FakeRunner(
        macNotificationChannel,
        answer: (_) => throw PlatformException(code: 'broken'),
      );
      await port.post(message);
      await port.withdraw();
      expect(runner.calls, hasLength(2));
    });

    test('a_press_names_the_request_of_its_message', () async {
      final List<NotificationAnswer> seen = <NotificationAnswer>[];
      final StreamSubscription<NotificationAnswer> listening = port.actions
          .listen(seen.add);
      addTearDown(listening.cancel);

      for (final String id in <String>['allow', 'block', 'show']) {
        await runner.send('action', <String, String>{
          'action': id,
          'flowId': flow.value,
        });
      }
      await pumpEventQueue();

      expect(seen, const <NotificationAnswer>[
        NotificationAnswer(kind: NotificationActionKind.allow, flowId: flow),
        NotificationAnswer(kind: NotificationActionKind.block, flowId: flow),
        NotificationAnswer(kind: NotificationActionKind.show, flowId: flow),
      ]);
    });

    test('a_press_that_names_nothing_decides_nothing', () async {
      final List<NotificationAnswer> seen = <NotificationAnswer>[];
      final StreamSubscription<NotificationAnswer> listening = port.actions
          .listen(seen.add);
      addTearDown(listening.cancel);

      await runner.send('action', <String, String>{
        'action': 'allow',
        'flowId': '',
      });
      await runner.send('action', <String, String>{
        'action': 'com.apple.UNNotificationDismissActionIdentifier',
        'flowId': flow.value,
      });
      await pumpEventQueue();

      expect(seen, isEmpty);
    });
  });
}
