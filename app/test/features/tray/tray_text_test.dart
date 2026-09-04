// Die Worte, die dieses Programm sagt, während niemand hinsieht (HUM-034,
// docs/UX.md 4.9): Tooltip, Fenstertitel und die Meldung selbst, in beiden
// Sprachen. Keine Ziffer für eine Frist, kein `mm:ss`.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/tray/attention_text.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';
import 'package:humanitl/features/tray/tray_icon.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

/// Die Beschriftung, die das Icon zu [face] tragen wuerde.
String trayFaceLabelOf(TrayFace face) => trayIconLabel(face.state, face.count);

void main() {
  late AppLocalizations en;
  late AppLocalizations de;

  setUpAll(() async {
    en = await AppLocalizations.delegate.load(const Locale('en'));
    de = await AppLocalizations.delegate.load(const Locale('de'));
  });

  group('remaining_is_a_word_never_a_clock', () {
    test('under a minute', () {
      expect(
        remainingPhrase(en, const Duration(seconds: 40)),
        'less than a minute left',
      );
      expect(
        remainingPhrase(de, const Duration(seconds: 40)),
        'noch weniger als eine Minute',
      );
    });

    test('minutes, rounded', () {
      expect(
        remainingPhrase(en, const Duration(seconds: 226)),
        'about 4 minutes left',
      );
      expect(
        remainingPhrase(de, const Duration(seconds: 226)),
        'noch etwa 4 Minuten',
      );
      expect(
        remainingPhrase(en, const Duration(seconds: 62)),
        'about a minute left',
      );
    });

    test('hours above ninety minutes', () {
      expect(
        remainingPhrase(en, const Duration(minutes: 115)),
        'about 2 hours left',
      );
    });

    test('no digit pair anywhere', () {
      for (final Duration left in <Duration>[
        const Duration(seconds: 5),
        const Duration(minutes: 3, seconds: 7),
        const Duration(hours: 2),
      ]) {
        for (final AppLocalizations l10n in <AppLocalizations>[en, de]) {
          expect(
            remainingPhrase(l10n, left),
            isNot(matches(RegExp(r'\d+:\d\d'))),
            reason: 'a still image must not carry a running clock',
          );
        }
      }
    });
  });

  group('waited_is_rounded_down', () {
    test('minutes', () {
      expect(
        waitedSentence(en, const Duration(seconds: 119)),
        'The agent has been waiting a minute',
      );
      expect(
        waitedSentence(de, const Duration(minutes: 4, seconds: 50)),
        'Der Agent wartet seit 4 Minuten',
      );
    });

    test('hours', () {
      expect(
        waitedSentence(en, const Duration(minutes: 121)),
        'The agent has been waiting 2 hours',
      );
    });
  });

  group('the_tray_says_what_it_counts', () {
    test('idle names the next event, not the absence', () {
      final TrayFace face = trayFace(en, const AttentionState());
      expect(face.state, TrayIconState.idle);
      expect(face.title, 'The queue is open');
      expect(face.menuShow, 'Show Humanitl');
      expect(face.menuQuit, 'Quit');
    });

    test('held names the number and the noun', () {
      final TrayFace face = trayFace(
        en,
        const AttentionState(tray: TrayIconState.held, held: 3),
      );
      expect(face.title, '3 requests held');
      expect(face.count, 3);
      expect(trayFaceLabelOf(face), '3');
    });

    test('ten and more collapse to 9+ in the icon, never in the words', () {
      final TrayFace face = trayFace(
        en,
        const AttentionState(tray: TrayIconState.held, held: 12),
      );
      expect(face.title, '12 requests held');
      expect(trayFaceLabelOf(face), '9+');
    });

    test('alert carries both lines', () {
      final TrayFace face = trayFace(
        en,
        const AttentionState(
          tray: TrayIconState.alert,
          held: 2,
          timedOutAway: 1,
        ),
      );
      expect(face.title, '2 requests held');
      expect(face.detail, '1 request was blocked after its time ran out');
    });

    test('offline says unknown instead of a stale number', () {
      final TrayFace face = trayFace(
        en,
        const AttentionState(tray: TrayIconState.offline),
      );
      expect(face.title, 'The daemon does not answer');
      expect(face.detail, contains('unknown'));
      expect(face.count, 0);
      expect(trayFaceLabelOf(face), '?');
    });
  });

  group('window_title', () {
    test('carries the count and drops it again', () {
      expect(windowTitle(en, const AttentionState()), 'Humanitl');
      expect(
        windowTitle(
          en,
          const AttentionState(tray: TrayIconState.held, held: 3),
        ),
        '(3) Humanitl',
      );
      expect(
        windowTitle(en, const AttentionState(tray: TrayIconState.offline)),
        'Humanitl',
      );
    });
  });

  group('the_message', () {
    HeldNotice notice({
      int total = 1,
      int findings = 0,
      Duration remaining = const Duration(minutes: 4),
    }) => HeldNotice(
      flowId: trayFlowId(1),
      host: 'api.github.com',
      method: 'POST',
      path: '/repos/acme/tools/issues',
      remaining: remaining,
      total: total,
      findings: findings,
      serial: 1,
    );

    test('the host is the summary, everything else the body', () {
      final DesktopNotification message = notificationFor(en, notice());
      expect(message.summary, 'api.github.com');
      expect(
        message.body,
        'POST /repos/acme/tools/issues\nabout 4 minutes left',
      );
    });

    test('several arrivals are one message with a count', () {
      final DesktopNotification message = notificationFor(en, notice(total: 5));
      expect(message.body, contains('+ 4 more requests'));
    });

    test('three buttons, and the German ones name the action', () {
      final DesktopNotification message = notificationFor(de, notice());
      expect(
        message.actions.map((NotificationAction a) => a.kind).toList(),
        <NotificationActionKind>[
          NotificationActionKind.allow,
          NotificationActionKind.block,
          NotificationActionKind.show,
        ],
      );
      expect(message.actions.first.label, 'Senden');
      expect(message.actions[1].label, 'Blockieren');
    });

    test('a finding takes the allow button away and says why', () {
      final DesktopNotification message = notificationFor(
        en,
        notice(findings: 1),
      );
      expect(
        message.actions.map((NotificationAction a) => a.kind).toList(),
        <NotificationActionKind>[
          NotificationActionKind.block,
          NotificationActionKind.show,
        ],
      );
      expect(message.body, contains('1 finding · decide in the window'));
    });
  });
}
