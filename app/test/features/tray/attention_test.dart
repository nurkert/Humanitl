// Die Zustandsmaschine hinter Tray, Notification und Rückkehr-Banner
// (HUM-034, docs/UX.md 4.9). Kein Widget, kein Schreibtisch: nur die Regeln,
// die entscheiden, wann dieses Programm einen Menschen anspricht.

import 'dart:io' show File;

import 'package:fake_async/fake_async.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/providers/attention.dart';

import 'fixtures.dart';

void main() {
  group('the setting the notifications hang on', () {
    test('the_register_really_has_both_keys', () {
      // `ui.notifications` hat sehr wohl ein Zuhause; der Kommentar am
      // Provider darf nichts anderes behaupten.
      final String config = File('../docs/CONFIG.md').readAsStringSync();
      expect(
        config,
        contains(
          RegExp(r'`ui\.notifications` \| boolean \| `true` \| advanced'),
        ),
      );
      expect(
        config,
        contains(RegExp(r'`ui\.sound` \| boolean \| `false` \| advanced')),
      );
      // HUM-034 nennt fuer `ui.notifications` die Stufe `basic`; das Register
      // nennt `advanced` und gewinnt (CONVENTIONS 4.19). Und die
      // Spezifikation verlangt, dass die Dokumentation die fehlende Wirkung
      // von `ui.sound` benennt. Seit HUM-101 steht das nicht mehr in der
      // Beschreibung, sondern in einer eigenen Spalte, die das Leser-Register
      // fuellt: `offen (HUM-xxx)` heisst, dass kein Code den Schluessel liest,
      // und nennt das Issue, das ihn wirksam macht. Die Zusicherung prueft
      // deshalb die Spalte und nicht mehr den Satz; sie muss angepasst werden,
      // sobald dieses Issue die Zeile auf `ja` dreht.
      expect(
        config,
        contains(RegExp(r'`ui\.sound`.*\| offen \(HUM-[0-9]+\) \|')),
      );
    });

    test('the_provider_names_the_real_reason', () {
      final String source = File('lib/features/tray/providers/attention.dart')
          .readAsStringSync();
      // Der Grund, warum jeder Meldungen bekommt, ist der fehlende Abruf,
      // nicht ein fehlender Eintrag im Register.
      for (final String named in <String>[
        'ui.notifications',
        'ui.sound',
        'advanced',
        'GetConfig',
      ]) {
        expect(
          source,
          contains(named),
          reason: 'der Kommentar zu notificationsEnabled nennt $named nicht',
        );
      }
    });
  });

  test('notify_on_zero_to_one_when_unfocused', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier);

    attention
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);

    final HeldNotice? notice = container.read(attentionProvider).notice;
    expect(notice, isNotNull);
    expect(notice!.flowId, trayFlowId(1));
    expect(notice.total, 1);
    expect(notice.serial, 1);
  });

  test('no_notify_when_focused', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier);

    // Der Fokus liegt beim Start auf dem Fenster: Was auf dem Schirm steht,
    // wird nicht zusätzlich angekündigt.
    attention.heldChanged(<Flow>[trayHeldFlow(n: 1)]);

    expect(container.read(attentionProvider).notice, isNull);
    expect(container.read(attentionProvider).held, 1);
  });

  test('notifications_off_stays_quiet', () {
    final ProviderContainer container = trayContainer(notifications: false);
    final Attention attention = container.read(attentionProvider.notifier);

    attention
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);

    expect(container.read(attentionProvider).notice, isNull);
    // Das Tray zählt weiter; abgeschaltet ist die Meldung, nicht die Zahl.
    expect(container.read(attentionProvider).tray, TrayIconState.held);
  });

  test('bundle_within_5s_updates_same_message', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);

      expect(container.read(attentionProvider).notice!.serial, 1);

      // Vier Ankünfte innerhalb des Fensters: die Meldung bleibt dieselbe.
      for (int n = 2; n <= 5; n++) {
        attention.heldChanged(<Flow>[
          for (int i = 1; i <= n; i++) trayHeldFlow(n: i),
        ]);
        async.elapse(const Duration(seconds: 1));
      }
      // Erst am Ende des Fensters wird die Meldung ein zweites Mal gestellt.
      async.elapse(notificationBundle);

      final HeldNotice notice = container.read(attentionProvider).notice!;
      expect(notice.serial, 2, reason: 'zwei Meldungen fuer fuenf Ankuenfte');
      expect(notice.total, 5);
      expect(notice.others, 4);
    });
  });

  test('burst_of_fifteen_is_never_one_per_request', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false);
      final List<int> serials = <int>[];
      container.listen(attentionProvider, (
        AttentionState? previous,
        AttentionState next,
      ) {
        final HeldNotice? notice = next.notice;
        if (notice != null && !serials.contains(notice.serial)) {
          serials.add(notice.serial);
        }
      });

      for (int n = 1; n <= 15; n++) {
        attention.heldChanged(<Flow>[
          for (int i = 1; i <= n; i++) trayHeldFlow(n: i),
        ]);
        async.elapse(const Duration(milliseconds: 300));
      }
      async.elapse(notificationBundle);

      // Fuenfzehn Ankuenfte in 4,5 s: eine Meldung, plus die eine
      // Aktualisierung am Ende des Fensters. Nie fuenfzehn.
      expect(serials, <int>[1, 2]);
    });
  });

  test('a_queue_that_changes_without_an_arrival_is_not_announced', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);
      expect(container.read(attentionProvider).notice!.serial, 1);

      // Das Bündelungsfenster läuft ab, ohne dass etwas ankam.
      async.elapse(notificationBundle * 2);

      // Nebenher läuft Verkehr. `heldFlowsProvider` rechnet bei jeder
      // Änderung von `flowsProvider` neu und liefert über `.toList()` jedes
      // Mal eine neue Liste mit denselben Anfragen darin; eine einzige
      // durchlaufende Anfrage erzeugt sechs solcher Änderungen.
      for (int i = 0; i < 6; i++) {
        attention.heldChanged(<Flow>[trayHeldFlow(n: 1)]);
        async.elapse(notificationBundle);
      }

      expect(
        container.read(attentionProvider).notice!.serial,
        1,
        reason: 'ohne Ankunft wird die Meldung nicht neu gestellt',
      );
    });
  });

  test('a_queue_that_shrinks_is_not_announced', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1), trayHeldFlow(n: 2)]);
      expect(container.read(attentionProvider).notice!.serial, 1);
      async.elapse(notificationBundle * 2);

      // Eine der beiden Anfragen wurde entschieden.
      attention.heldChanged(<Flow>[trayHeldFlow(n: 2)]);

      expect(container.read(attentionProvider).notice!.serial, 1);
    });
  });

  test('the_standing_message_is_updated_once_per_window', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false);
      final List<int> serials = <int>[];
      container.listen(attentionProvider, (
        AttentionState? previous,
        AttentionState next,
      ) {
        final HeldNotice? notice = next.notice;
        if (notice != null && !serials.contains(notice.serial)) {
          serials.add(notice.serial);
        }
      });

      // Dreißig Ankünfte über fünfzehn Sekunden, also drei Bündelungsfenster.
      for (int n = 1; n <= 30; n++) {
        attention.heldChanged(<Flow>[
          for (int i = 1; i <= n; i++) trayHeldFlow(n: i),
        ]);
        async.elapse(const Duration(milliseconds: 500));
      }

      // Eine Meldung je Fenster, nie eine je Ankunft.
      expect(serials, <int>[1, 2, 3, 4]);
      // Und am Ende eines Fensters nennt sie alles, was angekommen ist: die
      // stehende Meldung hängt nie mehr als ein Fenster hinterher
      // (`backlog/CONVENTIONS.md` 4.19).
      expect(container.read(attentionProvider).notice!.total, 30);
    });
  });

  test('the_standing_message_lags_at_most_one_window', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..focusChanged(focused: false)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);

      // Vierzehn weitere Ankünfte im offenen Fenster. Solange es offen ist,
      // steht die alte Zahl: das ist die in `backlog/CONVENTIONS.md` 4.19
      // festgehaltene Abweichung von `docs/UX.md` 4.9.
      attention.heldChanged(<Flow>[
        for (int i = 1; i <= 15; i++) trayHeldFlow(n: i),
      ]);
      expect(container.read(attentionProvider).notice!.total, 1);

      // Die Schranke: nach genau einem Fenster stimmt sie wieder.
      async.elapse(notificationBundle);
      expect(container.read(attentionProvider).notice!.total, 15);
    });
  });

  test('an_arrival_while_a_request_already_waits_is_announced', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      // Eine Anfrage wartet schon, das Fenster ist vorn: keine Meldung.
      final Attention attention = container.read(attentionProvider.notifier)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);
      expect(container.read(attentionProvider).notice, isNull);

      // Der Mensch geht. Was in diesem Augenblick wartet, hat er gesehen.
      attention.focusChanged(focused: false);
      expect(container.read(attentionProvider).notice, isNull);

      // Danach kommen fuenfzehn dazu. Unter der Lesart "Uebergang von null auf
      // eins" erfuhre er davon nie, weil die Schlange nie leer war.
      for (int n = 2; n <= 16; n++) {
        attention.heldChanged(<Flow>[
          for (int i = 1; i <= n; i++) trayHeldFlow(n: i),
        ]);
        async.elapse(const Duration(milliseconds: 200));
      }
      async.elapse(notificationBundle);

      final HeldNotice notice = container.read(attentionProvider).notice!;
      expect(notice.total, 16);
      expect(container.read(attentionProvider).held, 16);
    });
  });

  test('an_arrival_after_the_person_swept_the_message_away_updates_it', () {
    fakeAsync((FakeAsync async) {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..heldChanged(const <Flow>[])
        ..focusChanged(focused: false)
        ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);
      expect(container.read(attentionProvider).notice!.serial, 1);

      // Der Druck auf die Meldung nimmt sie vom Schirm, beendet aber nicht das
      // Gespraech: die naechste Ankunft haelt den Menschen auf dem Laufenden,
      // im selben Takt wie jede andere.
      attention.notificationAnswered();
      expect(container.read(attentionProvider).notice, isNull);

      attention.heldChanged(<Flow>[trayHeldFlow(n: 1), trayHeldFlow(n: 2)]);
      expect(
        container.read(attentionProvider).notice,
        isNull,
        reason: 'das offene Fenster haelt die Aktualisierung zurueck',
      );
      async.elapse(notificationBundle);
      expect(container.read(attentionProvider).notice!.total, 2);
    });
  });

  test('before_the_first_confirmed_queue_the_count_is_unknown', () {
    final ProviderContainer container = trayContainer();

    // Noch nichts gehoert: `Subscribe` heisst "ab jetzt", der Daemon kann drei
    // Anfragen halten, von denen dieser Client nichts weiss.
    final AttentionState first = container.read(attentionProvider);
    expect(first.tray, TrayIconState.offline);
    expect(first.held, 0);

    container.read(attentionProvider.notifier).heldChanged(<Flow>[
      for (int i = 1; i <= 3; i++) trayHeldFlow(n: i),
    ]);

    expect(container.read(attentionProvider).tray, TrayIconState.held);
    expect(container.read(attentionProvider).held, 3);
  });

  test('a_gap_in_the_stream_makes_the_count_unknown_again', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..heldChanged(<Flow>[for (int i = 1; i <= 2; i++) trayHeldFlow(n: i)]);
    expect(container.read(attentionProvider).held, 2);

    // Der Strom hat eine Luecke. Die Verbindung gilt weiter als verbunden;
    // der Herzschlag von `GetInfo` merkt davon nichts.
    attention.streamGapped();

    final AttentionState gapped = container.read(attentionProvider);
    expect(gapped.tray, TrayIconState.offline);
    expect(gapped.held, 0);

    attention.heldChanged(<Flow>[trayHeldFlow(n: 5)]);
    expect(container.read(attentionProvider).held, 1);
  });

  test('focus_takes_the_message_away', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);
    expect(container.read(attentionProvider).notice, isNotNull);

    attention.focusChanged(focused: true);

    expect(container.read(attentionProvider).notice, isNull);
  });

  test('empty_queue_takes_the_message_away', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[trayHeldFlow(n: 1)]);
    expect(container.read(attentionProvider).notice, isNotNull);

    attention.heldChanged(const <Flow>[]);

    expect(container.read(attentionProvider).notice, isNull);
    expect(container.read(attentionProvider).tray, TrayIconState.idle);
  });

  test('message_names_the_longest_waiting_not_the_first_row', () {
    final ProviderContainer container = trayContainer();
    // Der zweite Flow wartet laenger, steht aber wegen der spaeteren Frist
    // nicht oben in der nach Frist sortierten Queue.
    final Flow soon = trayHeldFlow(
      n: 1,
      waited: const Duration(seconds: 10),
      remaining: const Duration(minutes: 1),
      host: 'first.example',
    );
    final Flow older = trayHeldFlow(
      n: 2,
      waited: const Duration(minutes: 4),
      remaining: const Duration(minutes: 9),
      host: 'oldest.example',
    );

    container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[soon, older]);

    expect(container.read(attentionProvider).notice!.host, 'oldest.example');
  });

  test('findings_withhold_the_allow_button', () {
    final ProviderContainer container = trayContainer();
    container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[trayHeldFlow(n: 1, findings: 2)]);

    final HeldNotice notice = container.read(attentionProvider).notice!;
    expect(notice.findings, 2);
    expect(notice.mayAllow, isFalse);
  });

  group('tray_icon_state_mapping', () {
    test('0 is idle, 3 is held, 12 is held', () {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier);

      // Erst die bestaetigte leere Warteschlange, dann Ruhe: vorher weiss das
      // Programm nichts, und nichts ist nicht dasselbe wie null.
      attention.heldChanged(const <Flow>[]);
      expect(container.read(attentionProvider).tray, TrayIconState.idle);

      attention.heldChanged(<Flow>[
        for (int i = 1; i <= 3; i++) trayHeldFlow(n: i),
      ]);
      expect(container.read(attentionProvider).tray, TrayIconState.held);
      expect(container.read(attentionProvider).held, 3);

      attention.heldChanged(<Flow>[
        for (int i = 1; i <= 12; i++) trayHeldFlow(n: i),
      ]);
      expect(container.read(attentionProvider).held, 12);
    });

    test('a timeout while away turns the icon to alert', () {
      final ProviderContainer container = trayContainer();
      final Attention attention = container.read(attentionProvider.notifier)
        ..heldChanged(const <Flow>[])
        ..focusChanged(focused: false)
        ..holdTimedOut();

      expect(container.read(attentionProvider).tray, TrayIconState.alert);
      expect(container.read(attentionProvider).timedOutAway, 1);

      // Der Blick auf das Fenster ist die Kenntnisnahme.
      attention.focusChanged(focused: true);
      expect(container.read(attentionProvider).tray, TrayIconState.idle);
      expect(container.read(attentionProvider).timedOutAway, 0);
    });

    test('a timeout under the eye is not an alert', () {
      final ProviderContainer container = trayContainer();
      container.read(attentionProvider.notifier)
        ..heldChanged(const <Flow>[])
        ..holdTimedOut();

      expect(container.read(attentionProvider).tray, TrayIconState.idle);
    });
  });

  test('tray_says_unknown_when_the_daemon_is_gone', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[for (int i = 1; i <= 3; i++) trayHeldFlow(n: i)]);
    expect(container.read(attentionProvider).held, 3);

    attention.connectionChanged(connected: false);

    final AttentionState gone = container.read(attentionProvider);
    expect(gone.tray, TrayIconState.offline);
    // Nicht die letzte bekannte Zahl: die ist ein Schnappschuss eines Daemons,
    // der nicht mehr antwortet (CONVENTIONS 4.13).
    expect(gone.held, 0);
    expect(gone.notice, isNull);
  });

  test('after_a_gap_the_count_stays_unknown_until_the_queue_answers', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..heldChanged(<Flow>[for (int i = 1; i <= 3; i++) trayHeldFlow(n: i)]);
    expect(container.read(attentionProvider).held, 3);

    attention.connectionChanged(connected: false);
    expect(container.read(attentionProvider).tray, TrayIconState.offline);

    // `GetInfo` antwortet wieder. Der Ereignisstrom verbindet sich unabhängig
    // davon neu, mit eigenem Backoff bis 30 s; in diesem Fenster hat der
    // Daemon die alte Zahl nicht bestätigt, und behauptet wird sie deshalb
    // auch nicht (CONVENTIONS 4.13).
    attention.connectionChanged(connected: true);
    final AttentionState between = container.read(attentionProvider);
    expect(between.tray, TrayIconState.offline);
    expect(between.held, 0);

    // Erst die erste echte Queue-Meldung nach der Lücke bringt die Zahl
    // zurück, und es ist die neue, nicht die alte.
    attention.heldChanged(<Flow>[trayHeldFlow(n: 7)]);
    final AttentionState after = container.read(attentionProvider);
    expect(after.tray, TrayIconState.held);
    expect(after.held, 1);
  });

  test('the_return_banner_does_not_come_from_a_stale_queue', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[
        trayHeldFlow(n: 1, waited: const Duration(minutes: 4)),
      ])
      ..connectionChanged(connected: false)
      ..connectionChanged(connected: true)
      ..focusChanged(focused: true);

    expect(container.read(attentionProvider).banner, isNull);

    // Nach der ersten echten Meldung darf es wieder etwas behaupten.
    attention
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[
        trayHeldFlow(n: 1, waited: const Duration(minutes: 4)),
      ])
      ..focusChanged(focused: true);
    expect(container.read(attentionProvider).banner, isNotNull);
  });

  test('banner_after_60s_on_focus', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[
        trayHeldFlow(n: 1, waited: const Duration(seconds: 30)),
        trayHeldFlow(n: 2, waited: const Duration(minutes: 4)),
      ]);
    expect(container.read(attentionProvider).banner, isNull);

    attention.focusChanged(focused: true);

    final ReturnNotice banner = container.read(attentionProvider).banner!;
    expect(banner.flowId, trayFlowId(2));
    expect(banner.waited.inMinutes, 4);
  });

  test('banner_stays_away_below_the_threshold', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[
        trayHeldFlow(n: 1, waited: const Duration(seconds: 59)),
      ]);

    attention.focusChanged(focused: true);

    expect(container.read(attentionProvider).banner, isNull);
  });

  test('banner_goes_when_the_queue_empties', () {
    final ProviderContainer container = trayContainer();
    final Attention attention = container.read(attentionProvider.notifier)
      ..focusChanged(focused: false)
      ..heldChanged(<Flow>[
        trayHeldFlow(n: 1, waited: const Duration(minutes: 2)),
      ])
      ..focusChanged(focused: true);
    expect(container.read(attentionProvider).banner, isNotNull);

    attention.heldChanged(const <Flow>[]);

    expect(container.read(attentionProvider).banner, isNull);
  });

  test('the_window_port_drives_the_focus', () async {
    final FakeDesktop desktop = FakeDesktop();
    final ProviderContainer container = trayContainer(desktop: desktop);
    container.read(attentionProvider.notifier).heldChanged(<Flow>[
      trayHeldFlow(n: 1),
    ]);
    expect(container.read(attentionProvider).notice, isNull);

    desktop.window.emit(focused: false);
    await Future<void>.delayed(Duration.zero);
    container.read(attentionProvider.notifier).heldChanged(<Flow>[
      trayHeldFlow(n: 1),
      trayHeldFlow(n: 2),
    ]);

    // Der Fokuswechsel kam aus dem Port, nicht aus dem Test, und die Ankunft
    // danach wird gemeldet: `docs/CONFIG.md` beschreibt die Bedingung als
    // "eine Anfrage wartet und das Fenster ist nicht vorn", nicht als
    // Uebergang von null auf eins.
    final HeldNotice announced = container.read(attentionProvider).notice!;
    expect(announced.total, 2);
    // Genannt wird weiter die aelteste Anfrage, nicht die neueste.
    expect(announced.flowId, trayFlowId(1));
  });
}
