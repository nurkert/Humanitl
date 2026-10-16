// Die Oberflächen-Hälfte des M4-Demolaufs (HUM-055).
//
// `tests/e2e/m4_trusted_editor.sh` startet Daemon, Ziel und Agent; dieser
// Test ist Schritt 7 dazwischen: Die gehaltene Anfrage mit Kundenname,
// Mailadresse und IBAN erscheint in der Warteschlange, die Aktionsleiste
// sagt „Send with 3 findings", `E` öffnet den Editor, „Replace all with
// pseudonyms" ersetzt alle drei Funde, die Mapping-Leiste zeigt drei Zeilen
// mit maskierten Originalen, `Ctrl+Enter` sendet die bearbeitete
// Fassung, und die Historie zeigt die Zeile mit dem Chip „Edited". Was beim
// Ziel ankam, prüft danach das Skript (Schritt 8), nicht dieser Test.
//
// Der Test startet nichts und räumt nichts auf. Er verbindet sich über den
// XDG-Baum, den das Skript ihm mitgibt, mit dem laufenden Daemon und redet
// über dieselbe gRPC-Naht wie die ausgelieferte Anwendung.
//
// Die Umgebung:
//
//   * `HUMANITL_E2E_M4_HOST` ist der Host der Anfrage; ohne ihn bricht der
//     Test sofort ab (er ist nur aus dem Skript heraus sinnvoll).
//   * `HUMANITL_E2E_M4_READY` schreibt der Test, sobald der Bildschirm steht
//     und der Daemon geantwortet hat; erst dann startet das Skript den Agenten.
//   * `HUMANITL_E2E_SHOTS` ist das Verzeichnis für das Bild eines
//     fehlgeschlagenen Laufs.
//   * `HUMANITL_E2E_M4_SKIP_REPLACE=1` ist die Negativprobe
//     `e2e_m4_fails_when_original_leaks` und nur für einen Lauf von Hand da:
//     Der Test ersetzt dann nur die IBAN (die harte Sperre aus HUM-049 ließe
//     sie sonst nicht hinaus), schickt Kundenname und Mailadresse über
//     „Trotzdem senden" im Klartext, und Schritt 8 des Skripts muss rot werden.
//
// Angepasst an den Stand von `main` (Umfangsentscheidungen im Kopf von
// `backlog/sprint-4.md`): Der Nutzerbegriff wird `<TERM_1>` und nicht
// `Client-A`, weil die Anwendung die Aliasse aus der Konfiguration des Daemons
// nicht liest (`editor_host.dart`), und die Zuordnung ist die Leiste aus
// HUM-047 im Speicher mit ihrer vorläufigen Maske, nicht das Panel aus HUM-048
// (nach 0.1.0). Die Sprache bleibt Englisch; Schritt 11 der
// Spezifikation entfällt mit HUM-172.
//
// Alle Wartezeiten laufen über `pumpUntil` mit einer Frist. Ein
// `pumpAndSettle` liefe in dieser Oberfläche nie aus: Die Warteschlange
// animiert dauernd.

import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/core/ui/edited_badge.dart';
import 'package:humanitl/features/editor/model/draft.dart';
import 'package:humanitl/features/editor/model/draft_ops.dart';
import 'package:humanitl/features/editor/providers/draft_provider.dart';
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:integration_test/integration_test.dart';

/// Die Auflösung, in der die Selektoren dieses Tests gelten; unter 1400×900
/// greift das schmale Layout (wie im M2-Treiber).
const Size screenSize = Size(1600, 1000);

/// Der Schlüssel der Grenze, aus der das Bild eines fehlgeschlagenen Laufs
/// entsteht.
final GlobalKey screenshotKey = GlobalKey(debugLabel: 'm4-screenshot');

/// Die drei Originale der Anfrage (`tests/e2e/fixtures/m4_request.json`).
const String originalTerm = 'Müller GmbH';
const String originalEmail = 'anna@mueller.de';
const String originalIban = 'DE89370400440532013000';

/// Die Pseudonyme, die „Replace all" für die erste Anfrage der Sitzung vergibt.
const List<String> pseudonyms = <String>['<TERM_1>', '<EMAIL_1>', '<IBAN_1>'];

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('M4: pseudonymise a held request in the editor and send it', (
    WidgetTester tester,
  ) async {
    final Map<String, String> env = Platform.environment;
    final String host = (env['HUMANITL_E2E_M4_HOST'] ?? '').trim();
    if (host.isEmpty) {
      fail(
        'HUMANITL_E2E_M4_HOST is not set; this test is driven by '
        'tests/e2e/m4_trusted_editor.sh',
      );
    }
    final String? readyPath = env['HUMANITL_E2E_M4_READY'];
    final String? shotDir = env['HUMANITL_E2E_SHOTS'];
    final bool skipReplace = env['HUMANITL_E2E_M4_SKIP_REPLACE'] == '1';

    tester.view.physicalSize = screenSize;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);

    // Die Beschriftungen, die dieser Test liest, sind die englischen; die
    // Sprache des Schreibtischs soll daran nichts ändern (wie im M2-Treiber).
    tester.platformDispatcher.localeTestValue = const Locale('en');
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('en')];
    addTearDown(() {
      tester.platformDispatcher
        ..clearLocaleTestValue()
        ..clearLocalesTestValue();
    });

    final LaunchOptions options = LaunchOptions.resolve(const <String>[]);
    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[launchOptionsProvider.overrideWithValue(options)],
        child: RepaintBoundary(key: screenshotKey, child: const HumanitlApp()),
      ),
    );
    await tester.pump();

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(HumanitlApp)),
    );

    try {
      await _run(
        tester,
        container,
        host: host,
        readyPath: readyPath,
        skipReplace: skipReplace,
      );
      // Das Bild des Bildschirms am Ende von Schritt 7, auch wenn der Treiber
      // durchkommt: Wird erst ein Schritt des Skripts danach rot (etwa die
      // Negativprobe in Schritt 8), soll das Artefakt zeigen, was gesendet
      // wurde (HUM-055, Akzeptanzkriterien).
      await _screenshot(tester, shotDir, 'm4-step7.png');
    } on Object {
      await _screenshot(tester, shotDir, 'm4-failure.png');
      rethrow;
    }
  }, timeout: const Timeout(Duration(minutes: 4)));
}

Future<void> _run(
  WidgetTester tester,
  ProviderContainer container, {
  required String host,
  required String? readyPath,
  required bool skipReplace,
}) async {
  // --- Der Bildschirm steht, und der Daemon antwortet ------------------------

  await pumpUntil(
    tester,
    () => find.byType(IconRail).evaluate().isNotEmpty,
    what: 'the shell drew its rail',
  );
  await pumpUntil(
    tester,
    () => container.read(linkLiveProvider),
    timeout: const Duration(seconds: 60),
    what: 'the daemon answered GetInfo',
  );
  if (readyPath != null && readyPath.isNotEmpty) {
    File(readyPath).writeAsStringSync('ready\n', flush: true);
  }

  // --- Die gehaltene Anfrage --------------------------------------------------

  await _go(
    tester,
    Section.intercept,
    ready: () {
      final Flow? held = _held(container, host);
      return held != null && _row(held.id).evaluate().isNotEmpty;
    },
    timeout: const Duration(seconds: 90),
    what: 'the queue draws the held request to $host',
  );
  final Flow flow = _held(container, host)!;
  expect(flow.path, startsWith('/tickets'));
  expect(
    flow.findingCount,
    3,
    reason: 'the user term, the mail address and the IBAN',
  );

  await tester.tap(_row(flow.id));
  await tester.pump();
  await pumpUntil(
    tester,
    () => find
        .descendant(
          of: find.byKey(const Key('intercept-allow')),
          matching: find.text('Send with 3 findings'),
        )
        .evaluate()
        .isNotEmpty,
    what: 'the release valve says "Send with 3 findings"',
  );

  // --- Der Editor -------------------------------------------------------------

  await tester.sendKeyEvent(LogicalKeyboardKey.keyE);
  await tester.pump();
  await pumpUntil(
    tester,
    () =>
        find.byKey(Key('editor-${flow.id.value}')).evaluate().isNotEmpty &&
        (container.read(draftProvider(flow.id))?.body ?? '').contains(
          originalEmail,
        ),
    what: 'E opened the editor with the original body',
  );

  // Der Editor baut sich in zwei Schritten auf (Detail, dann zerlegter Rumpf),
  // und dazwischen steht für einen Rahmen nichts; Knopf und Entwurf sind erst
  // da, wenn beides geladen ist.
  await pumpUntil(
    tester,
    () =>
        find.byKey(const Key('editor-replace-all')).evaluate().isNotEmpty &&
        find.byKey(const Key('editor-draft-body')).evaluate().isNotEmpty,
    what: 'the editor offers "Replace all with pseudonyms"',
  );

  if (skipReplace) {
    await _leakOnPurpose(tester, container, flow);
  } else {
    await tester.tap(find.byKey(const Key('editor-replace-all')));
    await tester.pump();
    await pumpUntil(tester, () {
      final String body = container.read(draftProvider(flow.id))?.body ?? '';
      return pseudonyms.every(body.contains);
    }, what: 'Replace all put the three pseudonyms into the draft');
    final Draft draft = container.read(draftProvider(flow.id))!;
    for (final String original in <String>[
      originalTerm,
      originalEmail,
      originalIban,
    ]) {
      expect(
        draft.body,
        isNot(contains(original)),
        reason: 'the draft still carries $original',
      );
    }
    expect(openFindings(draft), 0, reason: 'no finding is left open');

    // Und dasselbe auf dem Bildschirm, nicht nur im Provider: Ein Entwurf,
    // den niemand sieht, ist nicht der, den ein Mensch geprüft hat.
    final String shown = tester
        .widget<EditableText>(
          find.descendant(
            of: find.byKey(const Key('editor-draft-body')),
            matching: find.byType(EditableText),
          ),
        )
        .controller
        .text;
    for (final String pseudonym in pseudonyms) {
      expect(shown, contains(pseudonym), reason: 'the editor draws $pseudonym');
    }
    expect(shown, isNot(contains(originalEmail)));

    // Die Zuordnung unter dem Editor, so weit es sie auf `main` gibt: die
    // Leiste aus HUM-047 im Speicher, nicht das Panel aus HUM-048. Drei
    // Zeilen, jede mit Pseudonym und maskiertem Original, und kein Original
    // im Klartext.
    expect(draft.replacements, hasLength(3));
    await tester.tap(find.byKey(const Key('editor-mapping-toggle')));
    await tester.pump();
    for (final (String pseudonym, String original) in <(String, String)>[
      ('<TERM_1>', originalTerm),
      ('<EMAIL_1>', originalEmail),
      ('<IBAN_1>', originalIban),
    ]) {
      expect(
        find.text(_masked(original)),
        findsOneWidget,
        reason: 'the mapping shows $pseudonym with the masked original',
      );
      expect(
        find.text(original),
        findsNothing,
        reason: 'and never the original itself',
      );
    }

    // Senden mit `Ctrl+Enter`, nachdem der Mensch in den Entwurf geklickt
    // hat. Der Klick ist nötig: „Replace all" verschwindet mit dem letzten
    // offenen Fund, und mit ihm der Fokus, an dem die Tastenbindungen des
    // Editors hängen (HUM-246).
    await tester.tap(find.byKey(const Key('editor-draft-body')));
    await tester.pump();
    await _chord(tester, LogicalKeyboardKey.enter, control: true);
  }

  await pumpUntil(
    tester,
    () => _held(container, host) == null,
    timeout: const Duration(seconds: 30),
    what: 'the edited request left the queue',
  );

  // --- Die Historie -----------------------------------------------------------

  await _go(
    tester,
    Section.history,
    ready: () =>
        find.byKey(const Key('history-filter-input')).evaluate().isNotEmpty,
    what: 'the history screen is on the front',
  );
  await pumpUntil(
    tester,
    () => container
        .read(historyPageProvider)
        .rows
        .any((Flow row) => row.id == flow.id && row.edited),
    timeout: const Duration(seconds: 30),
    what: 'the history holds the request as edited',
  );
  final Finder historyRow = find.byWidgetPredicate(
    (Widget widget) => widget is HistoryRow && widget.flow.id == flow.id,
  );
  await pumpUntil(
    tester,
    () => historyRow.evaluate().isNotEmpty,
    what: 'the history draws the row of the request',
  );
  expect(
    find.descendant(of: historyRow, matching: find.byType(EditedBadge)),
    findsOneWidget,
    reason: 'the row carries the chip "Edited"',
  );
  expect(
    find.descendant(of: historyRow, matching: find.text('Edited')),
    findsOneWidget,
    reason: 'and the chip says so in words',
  );
}

/// Das Original, maskiert wie die Mapping-Leiste es zeigt: die ersten und
/// letzten zwei Zeichen bleiben (`Replacement.maskedOriginal`).
String _masked(String original) =>
    '${original.substring(0, 2)}${'*' * (original.length - 4)}'
    '${original.substring(original.length - 2)}';

/// Die Negativprobe: nur die IBAN ersetzen, den Rest im Klartext senden.
///
/// Die IBAN muss weg, weil ein offener Fund mit Prüfsumme die Anfrage hart
/// sperrt (HUM-049, „Trotzdem senden" fehlt dann). Kundenname und
/// Mailadresse bleiben stehen und gehen über „Trotzdem senden" hinaus; genau
/// das muss Schritt 8 des Skripts finden.
Future<void> _leakOnPurpose(
  WidgetTester tester,
  ProviderContainer container,
  Flow flow,
) async {
  final Finder field = find.descendant(
    of: find.byKey(const Key('editor-draft-body')),
    matching: find.byType(EditableText),
  );
  final EditableText input = tester.widget<EditableText>(field);
  final String leaking = input.controller.text.replaceAll(
    originalIban,
    '<IBAN_1>',
  );
  input.focusNode.requestFocus();
  await tester.pump();
  await tester.enterText(field, leaking);
  await tester.pump();
  await pumpUntil(
    tester,
    () => !(container.read(draftProvider(flow.id))?.body ?? originalIban)
        .contains(originalIban),
    what: 'the draft lost the IBAN',
  );
  await _chord(tester, LogicalKeyboardKey.enter, control: true);
  await pumpUntil(
    tester,
    () => find
        .byKey(const Key('intercept-findings-pause-send'))
        .evaluate()
        .isNotEmpty,
    what: 'the editor paused on the findings left open',
  );
  // Die Pause gleitet herein; ein Tipp mitten in der Bewegung träfe daneben.
  await tester.pump(const Duration(milliseconds: 600));
  await tester.tap(find.byKey(const Key('intercept-findings-pause-send')));
  await tester.pump();
}

/// Die gehaltene Anfrage an [host], oder null, wenn keine mehr wartet.
Flow? _held(ProviderContainer container, String host) {
  for (final HeldGroup group in container.read(heldGroupsProvider).groups) {
    for (final Flow flow in group.flows) {
      if (flow.host == host) {
        return flow;
      }
    }
  }
  return null;
}

/// Die gehaltene Zeile dieses Flusses (wie im M2-Treiber über den Typ).
Finder _row(FlowId id) => find.byWidgetPredicate(
  (Widget widget) => widget is QueueRow && widget.flow.id == id,
);

/// Wechselt den Abschnitt über die Rail und wartet auf [ready].
Future<void> _go(
  WidgetTester tester,
  Section section, {
  required bool Function() ready,
  required String what,
  Duration timeout = const Duration(seconds: 20),
}) async {
  await tester.tap(find.byType(RailEntry).at(section.index));
  await tester.pump();
  await pumpUntil(tester, ready, what: what, timeout: timeout);
}

/// Drückt [key] mit den genannten Zusatztasten.
Future<void> _chord(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool control = false,
}) async {
  if (control) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  }
  await tester.sendKeyEvent(key);
  if (control) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  }
  await tester.pump();
}

/// Pumpt, bis [ready] wahr ist, und bricht sonst mit [what] ab.
Future<void> pumpUntil(
  WidgetTester tester,
  bool Function() ready, {
  required String what,
  Duration timeout = const Duration(seconds: 20),
  Duration step = const Duration(milliseconds: 100),
}) async {
  final Stopwatch clock = Stopwatch()..start();
  while (clock.elapsed < timeout) {
    if (ready()) {
      return;
    }
    await tester.pump(step);
  }
  if (ready()) {
    return;
  }
  fail('$what did not happen within ${timeout.inSeconds}s');
}

/// Schreibt das Bild des Bildschirms nach [directory], nur bei einem
/// Fehlschlag und nie so, dass ein Fehler hier den echten verdeckt.
Future<void> _screenshot(
  WidgetTester tester,
  String? directory,
  String name,
) async {
  if (directory == null) {
    return;
  }
  try {
    final RenderObject? object = screenshotKey.currentContext
        ?.findRenderObject();
    if (object is! RenderRepaintBoundary) {
      return;
    }
    final ui.Image image = await object.toImage();
    final ByteData? png = await image.toByteData(
      format: ui.ImageByteFormat.png,
    );
    image.dispose();
    if (png == null) {
      return;
    }
    final Directory target = Directory(directory);
    if (!target.existsSync()) {
      target.createSync(recursive: true);
    }
    File('${target.path}${Platform.pathSeparator}$name').writeAsBytesSync(
      png.buffer.asUint8List(png.offsetInBytes, png.lengthInBytes),
      flush: true,
    );
  } on Object catch (error) {
    // ignore: avoid_print
    print('m4: could not write the screenshot: $error');
  }
}
