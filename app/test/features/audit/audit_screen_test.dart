// Die Widget-Tests des Audit-Screens (HUM-051): die fünf der Spezifikation und
// die aus dem ersten Review.
//
// Jeder von ihnen läuft als `TargetPlatform.linux`: `flutter test` spielt ohne
// Variante Android, und dieses Produkt liegt auf Linux. Wer eine Liste, eine
// Tastenbindung oder eine Bildlaufleiste misst, misst sonst die falsche
// Plattform.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/daemon_client.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/audit/audit_screen.dart';
import 'package:humanitl/features/audit/providers/audit_provider.dart';
import 'package:humanitl/features/audit/widgets/audit_table.dart';

import 'harness.dart';

final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Ein Ordner-Dialog, der keinen Dialog öffnet und einen festen Ordner nennt.
///
/// Der echte Dialog spricht auf Linux mit dem XDG-Portal; ein Widget-Test hat
/// keines, und ein Test, der einen Dialog aufmachte, misst das Portal statt
/// den Bildschirm.
class _RecordingChooser {
  int calls = 0;
  String? answer = '/exports';

  Future<String?> call({required String dialogTitle}) async {
    calls++;
    return answer;
  }
}

/// Fängt ab, was in die Zwischenablage geschrieben wird.
List<String> _captureClipboard(WidgetTester tester) {
  final List<String> written = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (MethodCall call) async {
      if (call.method == 'Clipboard.setData') {
        written.add(
          (call.arguments as Map<Object?, Object?>)['text']! as String,
        );
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return written;
}

/// Gibt [text] in das Zeitfeld [key] ein und schließt mit Enter ab.
Future<void> _submitRange(WidgetTester tester, Key key, String text) async {
  final Finder field = find.descendant(
    of: find.byKey(key),
    matching: find.byType(EditableText),
  );
  await tester.enterText(field, text);
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump();
}

/// Öffnet das Export-Menü und wählt CSV.
Future<void> _exportCsv(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('audit-export-open')));
  await tester.pump();
  await tester.tap(find.byKey(const Key('audit-export-csv')));
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 500));
}

void main() {
  testWidgets('status_ok_green', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpAudit(tester, client: client);

    final Text status = tester.widget<Text>(
      find.byKey(const Key('audit-status-text')),
    );
    expect(status.data, 'Verified');
    expect(
      status.style?.color,
      HTokens.dark.state.allowed,
      reason: 'a chain that holds is drawn in the allowed green',
    );
    expect(find.byKey(const Key('audit-broken-diagnostic')), findsNothing);

    // Der Head-Hash ist der Hash des jüngsten Records und kein zweiter Wert:
    // Genau das hält das Akzeptanzkriterium gegen `humanitl audit verify
    // --json | jq .head`.
    final AuditHead head = await client.auditHead();
    expect(head.hash, fakeAuditHash(fakeAuditRecordCount));
    expect(head.seq, fakeAuditRecordCount);
  }, variant: _linux);

  testWidgets('status_broken_shows_seq_and_reason', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..auditChainBroken = true
      ..auditBreakSeq = 150
      ..auditBreakReason = AuditBreakReason.hashMismatch;
    await pumpAudit(tester, client: client);

    final Text status = tester.widget<Text>(
      find.byKey(const Key('audit-status-text')),
    );
    expect(status.data, contains('150'));
    expect(status.data, contains('changed after it was written'));
    expect(
      status.style?.color,
      HTokens.dark.state.blocked,
      reason: 'a broken chain is drawn in the blocked red',
    );

    // Der Befund des Daemons steht darunter, mit seinem eigenen Satz.
    expect(find.byKey(const Key('audit-broken-diagnostic')), findsOneWidget);
    expect(find.textContaining('AUDIT_001'), findsOneWidget);
    expect(find.textContaining('149 records before it hold'), findsOneWidget);
  }, variant: _linux);

  testWidgets('filter_by_kind_calls_query_with_prefix', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final ProviderContainer container = await pumpAudit(tester, client: client);
    client.auditQueries.clear();

    await tester.tap(find.byKey(const Key('audit-filter-kind')));
    await tester.pump();
    await settleAudit(tester, container);

    expect(
      client.auditQueries,
      isNotEmpty,
      reason: 'a new kind is a new query, not a filter applied on the client',
    );
    expect(
      client.auditQueries.last.filter.kindPrefix,
      auditKindPrefixes.first,
      reason: 'the kind of the filter bar reaches the daemon as a prefix',
    );
    final AuditFilter filter = container.read(auditFilterProvider);
    expect(filter.kindPrefix, auditKindPrefixes.first);
    final AuditRecordsState page = container.read(auditRecordsProvider(filter));
    expect(page.rows, isNotEmpty);
    expect(
      page.rows.every(
        (AuditRecordRow row) => row.kind.startsWith(auditKindPrefixes.first),
      ),
      isTrue,
      reason: 'every row of the answer carries the prefix that was asked for',
    );
  }, variant: _linux);

  testWidgets('row_tap_opens_sheet_with_json', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await pumpAudit(tester, client: client);

    expect(find.byType(AuditRecordSheet), findsNothing);
    final AuditRecordRow top = fakeAuditRecord(fakeAuditRecordCount);
    await tester.tap(find.byKey(ValueKey<int>(top.seq)));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 500));

    expect(find.byType(AuditRecordSheet), findsOneWidget);
    final Text json = tester.widget<Text>(
      find.byKey(const Key('audit-record-json')),
    );
    final String shown = json.data ?? '';
    expect(shown, contains('"seq": ${top.seq}'));
    expect(shown, contains('"hash": "${fakeAuditHash(top.seq)}"'));
    expect(
      shown,
      contains('"prev": "${fakeAuditHash(top.seq - 1)}"'),
      reason: 'the sheet shows the whole record, chain fields included',
    );
  }, variant: _linux);

  testWidgets('export_csv_calls_export_with_range', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    final DateTime from = DateTime.utc(2026, 9, 11, 8);
    final DateTime to = DateTime.utc(2026, 9, 11, 9);
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    container.read(auditFilterProvider.notifier)
      ..setFrom(from)
      ..setTo(to);
    await settleAudit(tester, container);

    await _exportCsv(tester);

    expect(client.auditExports, hasLength(1));
    final FakeAuditExport call = client.auditExports.single;
    expect(call.format, AuditExportFormat.csv);
    expect(call.outPath, startsWith('/exports/humanitl-audit-'));
    expect(call.outPath, endsWith('.csv'));
    expect(
      call.from,
      from,
      reason: 'the range of the filter bar goes with the export',
    );
    expect(call.to, to);

    // Was geschrieben wurde, steht auf dem Schirm, mit Zahl und Pfad.
    expect(find.byKey(const Key('audit-export-done')), findsOneWidget);
    expect(find.textContaining(call.outPath), findsOneWidget);
  }, variant: _linux);

  testWidgets('export_never_names_an_existing_file', (
    WidgetTester tester,
  ) async {
    // Die Anwendung schreibt nichts; sie wählt einen Ordner und einen Namen,
    // der dort frei ist. Der Daemon überschreibt nie (HUM-156), und ein
    // Speichern-Dialog mit null Bytes hätte eine gewählte Datei geleert, bevor
    // der Daemon gefragt war (Review M2).
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    final Set<String> taken = <String>{};
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) {
          // Der erste Name ist schon da; der zweite nicht.
          final bool first = !path.contains(RegExp(r'-\d+\.csv$'));
          if (first) {
            taken.add(path);
          }
          return first;
        }),
      ],
    );
    await settleAudit(tester, container);

    await _exportCsv(tester);

    expect(client.auditExports, hasLength(1));
    final String outPath = client.auditExports.single.outPath;
    expect(taken, hasLength(1));
    expect(outPath, isNot(taken.single));
    expect(outPath, endsWith('-2.csv'));

    // Abgebrochen heißt: nichts gefragt, nichts geschrieben, und das steht da.
    container.read(auditExportProvider.notifier).dismiss();
    chooser.answer = null;
    await tester.pump();
    await _exportCsv(tester);
    expect(client.auditExports, hasLength(1));
    expect(find.byKey(const Key('audit-export-cancelled')), findsOneWidget);
  }, variant: _linux);

  testWidgets('head_copy_puts_the_whole_hash_on_the_clipboard', (
    WidgetTester tester,
  ) async {
    final List<String> clipboard = _captureClipboard(tester);
    await pumpAudit(tester, client: FakeDaemonClient());

    final String shown =
        tester.widget<Text>(find.byKey(const Key('audit-head-hash'))).data ??
        '';
    expect(shown, contains('…'), reason: 'the card shortens the hash');

    await tester.tap(find.byKey(const Key('audit-copy-head')));
    await tester.pump();

    expect(clipboard, <String>[fakeAuditHash(fakeAuditRecordCount)]);
  }, variant: _linux);

  testWidgets('warnings_are_shown_in_amber', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..auditWarnings = const <AuditWarning>[
        AuditWarning(kind: 'no_hmac_key'),
        AuditWarning(kind: 'unanchored_tail', records: 12),
      ];
    await pumpAudit(tester, client: client);

    final Finder noKey = find.textContaining('the MACs are unchecked');
    final Finder tail = find.textContaining('12 records stand behind');
    expect(noKey, findsOneWidget);
    expect(tail, findsOneWidget);
    for (final Finder finder in <Finder>[noKey, tail]) {
      expect(
        tester.widget<Text>(finder).style?.color,
        HTokens.dark.state.held,
        reason: 'what the check could not prove is amber, not green',
      );
    }
    // Die Kette hält trotzdem; die Warnungen machen sie nicht rot.
    expect(
      tester.widget<Text>(find.byKey(const Key('audit-status-text'))).data,
      'Verified',
    );
  }, variant: _linux);

  testWidgets('paging_appends_the_next_page', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient()..auditRecordCount = 450;
    final ProviderContainer container = await pumpAudit(tester, client: client);
    AuditRecordsState page() => container.read(
      auditRecordsProvider(container.read(auditFilterProvider)),
    );
    expect(page().rows, hasLength(auditPageSize));

    for (int i = 0; i < 6 && page().rows.length < 450; i++) {
      await tester.drag(find.byType(ListView), const Offset(0, -20000));
      await tester.pump();
      await settleAudit(tester, container);
    }

    final List<int> seqs = <int>[
      for (final AuditRecordRow row in page().rows) row.seq,
    ];
    expect(seqs, hasLength(450), reason: 'three pages, appended');
    expect(seqs, <int>[
      for (int seq = 450; seq >= 1; seq--) seq,
    ], reason: 'newest first, no row twice, no row missing');
    expect(
      client.auditQueries.where(
        (({AuditFilter filter, int limit, String cursor}) call) =>
            call.cursor.isNotEmpty,
      ),
      hasLength(2),
      reason: 'the next pages come over the cursor of the daemon',
    );
  }, variant: _linux);

  testWidgets('unreadable_bound_keeps_the_filter_and_says_so', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = await pumpAudit(
      tester,
      client: FakeDaemonClient(),
    );
    const Key fromKey = Key('audit-filter-from');

    // Ohne Zonenangabe gilt UTC, dieselbe Zone, in der das Feld anzeigt.
    await _submitRange(tester, fromKey, '2026-09-11 08:05');
    expect(
      container.read(auditFilterProvider).from,
      DateTime.utc(2026, 9, 11, 8, 5),
    );
    final EditableText shown = tester.widget<EditableText>(
      find.descendant(
        of: find.byKey(fromKey),
        matching: find.byType(EditableText),
      ),
    );
    expect(shown.controller.text, '2026-09-11 08:05:00Z');
    expect(find.byKey(ValueKey<String>('$fromKey-invalid')), findsNothing);

    // Was keine Zeit ist, nimmt die Grenze nicht weg.
    await _submitRange(tester, fromKey, '11.09.2026 08:05');
    expect(
      container.read(auditFilterProvider).from,
      DateTime.utc(2026, 9, 11, 8, 5),
      reason: 'an unreadable bound leaves the filter as it was',
    );
    expect(find.byKey(ValueKey<String>('$fromKey-invalid')), findsOneWidget);
  }, variant: _linux);

  testWidgets('worst_case_fits_the_smallest_window', (
    WidgetTester tester,
  ) async {
    // Gebrochene Kette, zwei Warnungen und ein gescheiterter Export im
    // kleinsten Fenster: Die Tabelle behält Zeilen (Review M4).
    final FakeDaemonClient client = FakeDaemonClient()
      ..auditChainBroken = true
      ..auditWarnings = const <AuditWarning>[
        AuditWarning(kind: 'no_hmac_key'),
        AuditWarning(kind: 'unanchored_tail', records: 12),
      ];
    final _RecordingChooser chooser = _RecordingChooser();
    await pumpAudit(
      tester,
      client: client,
      size: const Size(1044, 640),
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        // Kein Name ist frei: Der Export scheitert, bevor er fragt.
        auditPathTakenProvider.overrideWithValue((String path) => true),
      ],
    );
    await _exportCsv(tester);

    expect(find.byKey(const Key('audit-broken-diagnostic')), findsOneWidget);
    expect(find.byKey(const Key('audit-export-failure')), findsOneWidget);
    expect(tester.takeException(), isNull, reason: 'nothing overflows');
    final Finder top = find.byKey(ValueKey<int>(fakeAuditRecordCount));
    expect(top, findsOneWidget, reason: 'the table still shows rows');
    expect(tester.getSize(top).height, greaterThan(0));
  }, variant: _linux);

  testWidgets('anchors_not_reported_are_not_zero', (WidgetTester tester) async {
    await pumpAudit(
      tester,
      client: FakeDaemonClient()..auditAnchorsReported = false,
    );
    expect(
      tester.widget<Text>(find.byKey(const Key('audit-anchor-count'))).data,
      'anchors not reported',
    );
    expect(find.text('no anchors'), findsNothing);
    expect(find.text('no anchor yet'), findsNothing);
  }, variant: _linux);

  testWidgets('verify_now_reloads_the_table', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final ProviderContainer container = await pumpAudit(tester, client: client);
    client.auditQueries.clear();

    await tester.tap(find.byKey(const Key('audit-verify-now')));
    await tester.pump();
    await settleAudit(tester, container);

    expect(
      client.auditQueries.where(
        (({AuditFilter filter, int limit, String cursor}) call) =>
            call.cursor.isEmpty,
      ),
      isNotEmpty,
      reason: 'a new check reads the first page again',
    );
  }, variant: _linux);

  testWidgets('bound_typed_without_enter_goes_with_the_export', (
    WidgetTester tester,
  ) async {
    // Getippt, nicht bestätigt, dann Export: Die Datei gilt dem Zeitraum im
    // Feld, nicht allen Zeiten (Review 2, Befund 2).
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const Key('audit-filter-from')),
        matching: find.byType(EditableText),
      ),
      '2026-09-11 08:05',
    );
    await tester.pump();

    await _exportCsv(tester);

    expect(client.auditExports, hasLength(1));
    expect(client.auditExports.single.from, DateTime.utc(2026, 9, 11, 8, 5));
  }, variant: _linux);

  testWidgets('unreadable_bound_without_enter_blocks_the_export', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const Key('audit-filter-from')),
        matching: find.byType(EditableText),
      ),
      '11.09.2026 08:05',
    );
    await tester.pump();

    await _exportCsv(tester);

    expect(client.auditExports, isEmpty, reason: 'no export over all time');
    expect(chooser.calls, 0, reason: 'nobody is asked for a folder either');
    expect(
      find.byKey(ValueKey<String>('${const Key('audit-filter-from')}-invalid')),
      findsOneWidget,
    );
  }, variant: _linux);

  testWidgets('outside_filter_change_updates_the_field', (
    WidgetTester tester,
  ) async {
    final ProviderContainer container = await pumpAudit(
      tester,
      client: FakeDaemonClient(),
    );
    String shown() => tester
        .widget<EditableText>(
          find.descendant(
            of: find.byKey(const Key('audit-filter-from')),
            matching: find.byType(EditableText),
          ),
        )
        .controller
        .text;

    container
        .read(auditFilterProvider.notifier)
        .setFrom(DateTime.utc(2026, 9, 11, 8));
    await tester.pump();
    expect(shown(), '2026-09-11 08:00:00Z');

    container.read(auditFilterProvider.notifier).clear();
    await tester.pump();
    expect(shown(), '', reason: 'a cleared filter clears the field');
  }, variant: _linux);

  testWidgets('sheet_forgets_copied_for_another_row', (
    WidgetTester tester,
  ) async {
    _captureClipboard(tester);
    await pumpAudit(tester, client: FakeDaemonClient());
    Finder copyLabel(String text) => find.descendant(
      of: find.byKey(const Key('audit-record-copy')),
      matching: find.text(text),
    );

    await tester.tap(find.byKey(const ValueKey<int>(fakeAuditRecordCount)));
    await tester.pump(const Duration(milliseconds: 500));
    await tester.tap(find.byKey(const Key('audit-record-copy')));
    await tester.pump();
    expect(copyLabel('Copied'), findsOneWidget);

    // Eine andere Zeile, links vom Sheet angeklickt.
    await tester.tapAt(
      tester.getTopLeft(
            find.byKey(const ValueKey<int>(fakeAuditRecordCount - 1)),
          ) +
          const Offset(40, 8),
    );
    await tester.pump(const Duration(milliseconds: 500));
    expect(
      tester.widget<Text>(find.byKey(const Key('audit-record-json'))).data,
      contains('"seq": ${fakeAuditRecordCount - 1}'),
    );
    expect(copyLabel('Copy JSON'), findsOneWidget);
  }, variant: _linux);

  // --- Review 3: was den Bildschirm überlebt und was nicht -------------------

  testWidgets('invalid_mark_dies_with_the_screen', (WidgetTester tester) async {
    // Ein Feld, das sich als unlesbar gemeldet hat, darf den Export nicht über
    // das Schließen des Bildschirms hinaus sperren.
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    await tester.enterText(
      find.descendant(
        of: find.byKey(const Key('audit-filter-from')),
        matching: find.byType(EditableText),
      ),
      'not a time',
    );
    await tester.pump();
    await _exportCsv(tester);
    expect(client.auditExports, isEmpty, reason: 'refused while it stands');

    await _remount(tester, container);
    await _exportCsv(tester);

    expect(
      client.auditExports,
      hasLength(1),
      reason: 'a new screen has no field that is unreadable',
    );
  }, variant: _linux);

  testWidgets('old_generations_are_released', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final ProviderContainer container = await pumpAudit(tester, client: client);
    for (int i = 0; i < 2; i++) {
      await tester.tap(find.byKey(const Key('audit-verify-now')));
      await tester.pump();
      await settleAudit(tester, container);
    }

    expect(container.read(auditRunProvider), 2);
    expect(container.exists(auditVerifyProvider(0)), isFalse);
    expect(container.exists(auditHeadProvider(0)), isFalse);
    expect(container.exists(auditVerifyProvider(1)), isFalse);
    expect(container.exists(auditVerifyProvider(2)), isTrue);
    expect(client.auditVerifyCalls, 3);

    // Keine Schleife: Die aktuelle Generation fragt nicht von selbst noch
    // einmal.
    await tester.pump(const Duration(seconds: 3));
    expect(client.auditVerifyCalls, 3);
  }, variant: _linux);

  testWidgets('export_banner_does_not_outlive_the_screen', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    await _exportCsv(tester);
    expect(find.byKey(const Key('audit-export-done')), findsOneWidget);

    await _remount(tester, container);

    expect(find.byKey(const Key('audit-export-done')), findsNothing);
  }, variant: _linux);

  testWidgets('export_survives_leaving_mid_run', (WidgetTester tester) async {
    // Wer während des Ordner-Dialogs den Bildschirm verlässt, bricht den
    // Export nicht ab: Er läuft zu Ende und erreicht den Daemon.
    final FakeDaemonClient client = FakeDaemonClient();
    final Completer<String?> folder = Completer<String?>();
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(
          ({required String dialogTitle}) => folder.future,
        ),
        auditPathTakenProvider.overrideWithValue((String path) => false),
      ],
    );
    await _exportCsv(tester);
    expect(client.auditExports, isEmpty, reason: 'the dialog is still open');

    await tester.pumpWidget(
      UncontrolledProviderScope(container: container, child: const SizedBox()),
    );
    await tester.pump();
    folder.complete('/exports');
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 100));

    expect(client.auditExports, hasLength(1));
  }, variant: _linux);

  testWidgets('head_copy_resets_on_a_new_head', (WidgetTester tester) async {
    _captureClipboard(tester);
    final FakeDaemonClient client = FakeDaemonClient();
    final ProviderContainer container = await pumpAudit(tester, client: client);
    Finder label(String text) => find.descendant(
      of: find.byKey(const Key('audit-copy-head')),
      matching: find.text(text),
    );

    await tester.tap(find.byKey(const Key('audit-copy-head')));
    await tester.pump();
    expect(label('Copied'), findsOneWidget);

    client.auditRecordCount = fakeAuditRecordCount + 1;
    await tester.tap(find.byKey(const Key('audit-verify-now')));
    await tester.pump();
    await settleAudit(tester, container);

    expect(
      tester.widget<Text>(find.byKey(const Key('audit-head-hash'))).data,
      startsWith(fakeAuditHash(fakeAuditRecordCount + 1).substring(0, 7)),
      reason: 'the head did change',
    );
    expect(label('Copy'), findsOneWidget);
  }, variant: _linux);

  testWidgets('a_full_folder_reports_audit_008', (WidgetTester tester) async {
    // Kein Name im Ordner ist frei. Die Fähigkeit ist da, nur der Ordner ist
    // voll: `AUDIT_008`, nicht `IPC_006`, und der Daemon wird nicht gefragt
    // (HUM-158).
    final FakeDaemonClient client = FakeDaemonClient();
    final _RecordingChooser chooser = _RecordingChooser();
    final ProviderContainer container = await pumpAudit(
      tester,
      client: client,
      overrides: <Override>[
        auditFolderChooserProvider.overrideWithValue(chooser.call),
        auditPathTakenProvider.overrideWithValue((String path) => true),
      ],
    );
    await settleAudit(tester, container);

    await _exportCsv(tester);

    final AuditExportState state = container.read(auditExportProvider);
    expect(state.phase, AuditExportPhase.failed);
    expect(state.failure?.code, 'AUDIT_008');
    expect(state.failure?.why, contains('no free file name'));
    expect(state.failure?.fix, isNotNull, reason: 'the fix shows the folder');
    expect(client.auditExports, isEmpty, reason: 'the daemon is not asked');
    expect(find.byKey(const Key('audit-export-failure')), findsOneWidget);
  }, variant: _linux);
}

/// Nimmt den Bildschirm aus dem Baum und baut ihn im selben Container neu auf,
/// wie es ein Wechsel des Abschnitts täte, der ihn wirklich entfernt.
Future<void> _remount(WidgetTester tester, ProviderContainer container) async {
  await tester.pumpWidget(
    UncontrolledProviderScope(container: container, child: const SizedBox()),
  );
  await tester.pump();
  await tester.pumpWidget(
    UncontrolledProviderScope(container: container, child: auditApp()),
  );
  await settleAudit(tester, container);
}
