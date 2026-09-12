// Die Oberflächen-Hälfte des M2-Demolaufs (HUM-097).
//
// Derselbe Lauf wie `tests/e2e/m2_first_decision/run.sh`, nur über den
// Bildschirm statt über die Kommandozeile: Die Warteschlange zeigt drei
// Gruppen, ein Mensch gibt die ganze npm-Gruppe mit einer Sitzungsregel frei,
// blockt eine Anfrage, erlaubt eine, lässt eine verfallen, filtert die
// Historie und schreibt zum Schluss den HAR-Export.
//
// Der Test startet nichts und räumt nichts auf. Daemon, Ziel und Sandbox
// gehören dem Skript; er verbindet sich mit dem laufenden Daemon über den
// XDG-Baum, den `run.sh` ihm in der Umgebung mitgibt, und redet über dieselbe
// gRPC-Naht wie die ausgelieferte Anwendung.
//
// Die Verzahnung mit `run.sh` läuft über drei Dateien, deren Pfade in der
// Umgebung stehen:
//
//   * `HUMANITL_E2E_READY` schreibt dieser Test, sobald der Bildschirm steht
//     und der Daemon geantwortet hat. Erst danach startet `run.sh` den Agenten,
//     denn ein Treiber, der erst nach dem Halten erscheint, entscheidet nichts.
//   * `HUMANITL_E2E_GO` schreibt `run.sh`, sobald es die Ids des npm-Stapels
//     festgehalten hat. Erst danach entscheidet dieser Test, sonst wären die
//     zwölf Flüsse weg, bevor das Skript sie lesen konnte.
//   * `HUMANITL_E2E_HAR` ist der Pfad des Exports. Er kommt nicht in diesen
//     Test, sondern in die Startoptionen der Anwendung: Das Exportziel wird im
//     Produktivcode gewählt, damit der Lauf denselben Weg prüft, den ein
//     Mensch geht (`backlog/CONVENTIONS.md` 4.13, 4.22).
//
// Alle Wartezeiten laufen über `pumpUntil` mit einer Frist. Ein
// `pumpAndSettle` liefe in dieser Oberfläche nie aus: Die Warteschlange
// animiert dauernd (Countdown, Puls des Zählers).

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
import 'package:humanitl/features/history/history_table.dart';
import 'package:humanitl/features/history/providers/history_export.dart';
import 'package:humanitl/features/history/providers/history_page.dart';
import 'package:humanitl/features/history/providers/history_query.dart';
import 'package:humanitl/features/intercept/providers/decision.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/providers/selection.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/rules/providers/rules.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:integration_test/integration_test.dart';

/// Die Auflösung, in der die Selektoren dieses Tests gelten.
///
/// Unter 1400×900 greift das schmale Layout, und die Panes, die dieser Test
/// bedient, sind dann nicht alle auf dem Schirm. `run.sh` startet `xvfb` mit
/// derselben Größe; hier steht sie noch einmal, damit der Test auch auf einem
/// Schreibtisch mit anderer Fenstergröße dasselbe misst.
const Size screenSize = Size(1600, 1000);

/// Der Schlüssel der Grenze, aus der das Bild eines fehlgeschlagenen Laufs
/// entsteht.
final GlobalKey screenshotKey = GlobalKey(debugLabel: 'm2-screenshot');

/// Die Notiz, die der Mensch der geblockten Anfrage mitgibt.
///
/// Wörtlich dieselbe, die der Zweig ohne Oberfläche über
/// `humanitl flows decide ... block` schickt: Abschnitt 5 von `run.sh` prüft
/// sie im Rumpf des `403` und im Kopf `X-Humanitl-Note` und ist damit
/// unabhängig davon, wer entschieden hat.
const String blockNote = 'not in this run';

/// Die Apex-Namen der drei Gruppen dieses Laufs.
///
/// Die Warteschlange gruppiert nach der registrierbaren Domäne, die Zeilen der
/// Historie nennen den Host. Beides ist nur beim dritten Ziel dasselbe Wort,
/// und deshalb stehen die Hosts daneben statt einmal geraten zu werden.
const String npmApex = 'npmjs.org';
const String githubApex = 'github.com';
const String evilApex = 'evil.example';

/// Die Hosts, die der Agent anspricht (`script.json`).
const String npmHost = 'registry.npmjs.org';
const String githubHost = 'api.github.com';
const String evilHost = 'evil.example';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('the first decision of M2, driven from the screen', (
    WidgetTester tester,
  ) async {
    final Map<String, String> env = Platform.environment;
    final String harPath = _required(env, LaunchOptions.harExportVariable);
    final String? readyPath = env['HUMANITL_E2E_READY'];
    final String? goPath = env['HUMANITL_E2E_GO'];
    final String? shotDir = env['HUMANITL_E2E_SHOTS'];

    tester.view.physicalSize = screenSize;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);

    // Die Sprache wird festgenagelt, und zwar hier und nicht über `LANG` im
    // Env-Block von `run.sh`. Dieser Test liest Beschriftungen — „Temporary
    // (1)", „Blocked (timed out)" —, und die Anwendung zeichnet sie in der
    // Sprache des Schreibtischs. Eine Umgebungsvariable im Skript schützte nur
    // den Lauf aus dem Skript; ein Entwickler, der die Datei von Hand gegen
    // seinen eigenen Daemon fährt (die Spezifikation verlangt genau das),
    // säße auf einem deutschen Rechner wieder vor einem roten Test. Der
    // Override gilt für jeden Aufrufer.
    tester.platformDispatcher.localeTestValue = const Locale('en');
    tester.platformDispatcher.localesTestValue = const <Locale>[Locale('en')];
    addTearDown(() {
      tester.platformDispatcher
        ..clearLocaleTestValue()
        ..clearLocalesTestValue();
    });

    // `LaunchOptions.resolve` und nicht ein Provider-Override von Hand: Der
    // Test liest dieselbe Umgebung, aus der die ausgelieferte `main()` ihre
    // Optionen zieht, und bekommt darüber Socket **und** Exportziel.
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
        harPath: harPath,
        readyPath: readyPath,
        goPath: goPath,
      );
    } on Object {
      await _screenshot(tester, shotDir, 'm2-failure.png');
      rethrow;
    }
  }, timeout: const Timeout(Duration(minutes: 4)));
}

Future<void> _run(
  WidgetTester tester,
  ProviderContainer container, {
  required String harPath,
  required String? readyPath,
  required String? goPath,
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
  if (readyPath != null) {
    File(readyPath).writeAsStringSync('ready\n', flush: true);
  }

  // --- 1. Drei Gruppen ------------------------------------------------------

  // Der Agent läuft jetzt; seine zwölf Anfragen an die Registry sammeln sich.
  await pumpUntil(
    tester,
    () => _group(container, npmApex)?.length == 12,
    timeout: const Duration(seconds: 60),
    what: 'twelve requests to the package registry are held',
  );
  await pumpUntil(
    tester,
    () =>
        _group(container, githubApex)?.length == 2 &&
        _group(container, evilApex)?.length == 1,
    what: 'the other two hosts are groups of their own',
  );

  // Der Setup-Bildschirm kann die Shell einmalig zu sich geholt haben, solange
  // eine seiner vier Zeilen den Start anhält (`shell_screen.dart`
  // `_offerSetup`). Ein Mensch klickt dann auf „Intercept"; dieser Test tut
  // dasselbe, über dieselbe Rail.
  await _go(
    tester,
    Section.intercept,
    ready: () =>
        find.byKey(const Key('queue-group-$npmApex')).evaluate().isNotEmpty,
    what: 'the queue draws the group of the package registry',
  );

  expect(
    container.read(heldGroupsProvider).groups,
    hasLength(3),
    reason: 'three registrable domains, not seventeen rows',
  );
  expect(find.byKey(const Key('queue-group-$npmApex')), findsOneWidget);
  expect(find.byKey(const Key('queue-group-$githubApex')), findsOneWidget);

  // Eingeklappt, weil zwölf Anfragen der Schwall sind, für den es die Gruppe
  // gibt; die zwei des Code-Hosts stehen offen darunter.
  expect(
    container
        .read(expandedGroupsProvider.notifier)
        .isOpen(_group(container, npmApex)!),
    isFalse,
    reason: 'the burst of twelve opens folded',
  );
  expect(
    find.byKey(const Key('queue-group-findings-$npmApex')),
    findsNothing,
    reason: 'no request to the registry carries a finding',
  );
  expect(
    find.byKey(const Key('queue-group-findings-$githubApex')),
    findsOneWidget,
    reason: 'the POST carries the mail address',
  );
  expect(_group(container, githubApex)!.findingsTotal, 1);
  expect(_group(container, evilApex)!.findingsTotal, 1);

  // Und dasselbe noch einmal auf dem Bildschirm statt im Container. Ein Zähler
  // im Anbieter beweist nicht, dass jemand ihn sehen kann; erst das Abzeichen
  // im Kopf der Gruppe tut das.
  expect(
    find.descendant(
      of: find.byKey(const Key('queue-group-$npmApex')),
      matching: find.text('12'),
    ),
    findsOneWidget,
    reason: 'the head of the npm group counts its twelve held requests',
  );
  expect(
    find.descendant(
      of: find.byKey(const Key('queue-group-$githubApex')),
      matching: find.text('2'),
    ),
    findsOneWidget,
    reason: 'and the head of the code host its two',
  );
  expect(
    find.descendant(
      of: find.byKey(const Key('queue-group-findings-$githubApex')),
      matching: find.text('1'),
    ),
    findsOneWidget,
    reason: 'the findings badge says one, not just that it is there',
  );

  // Die eine Anfrage an den dritten Host steht als Zeile da, nicht als Kopf:
  // unter zwei Anfragen gibt es keine Gruppe (`held_groups.dart` `groupFrom`).
  final Flow evilRow = _group(container, evilApex)!.flows.single;
  expect(find.byKey(const Key('queue-group-$evilApex')), findsNothing);
  expect(_row(evilRow.id), findsOneWidget);
  expect(
    find.descendant(of: _row(evilRow.id), matching: find.text(evilHost)),
    findsOneWidget,
    reason: 'the row names the host it belongs to',
  );

  // Eingeklappt heißt: die zwölf Zeilen sind nicht im Baum. Auf dem Schirm
  // stehen genau drei Zeilen — die zwei des Code-Hosts und die eine des
  // dritten —, und keine einzige der Registry.
  for (final Flow flow in _group(container, npmApex)!.flows) {
    expect(
      _row(flow.id),
      findsNothing,
      reason: 'a folded group draws no rows (${flow.path})',
    );
  }
  expect(
    find.byType(QueueRow),
    findsNWidgets(3),
    reason: 'two rows under the code host, one for the third, none folded out',
  );

  // --- Warten, bis `run.sh` die Ids des Stapels festgehalten hat ------------

  if (goPath != null) {
    await pumpUntil(
      tester,
      () => File(goPath).existsSync(),
      timeout: const Duration(seconds: 30),
      what: 'the script wrote down the ids of the batch',
    );
  }

  // --- 2. Die ganze npm-Gruppe, mit einer Sitzungsregel ---------------------

  await tester.tap(find.byKey(const Key('queue-group-$npmApex')));
  await tester.pump();
  expect(
    container.read(selectionProvider),
    hasLength(12),
    reason: 'the tap on the header reaches the whole group',
  );

  // `Remember` öffnen und auf `session` × `apex` stellen: Umschalt+Eingabe
  // klappt das Gitter auf, die Ziffern wählen in den beiden Segmenten.
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.pump();
  expect(find.byKey(const Key('intercept-remember')), findsOneWidget);

  await tester.sendKeyEvent(LogicalKeyboardKey.digit2);
  await tester.pump();
  await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  await tester.sendKeyEvent(LogicalKeyboardKey.digit3);
  await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  await tester.pump();

  final RememberState draft = container.read(rememberDraftProvider);
  expect(draft.duration, RememberDuration.session);
  expect(draft.target, RememberTarget.apex);

  // Das Ventil ist erst scharf, wenn die Auswahl 350 ms stillstand
  // (`docs/UX.md` 5.4). Ohne dieses Warten weist die Leiste die Freigabe ab.
  await pumpUntil(
    tester,
    () => container.read(allowArmedProvider),
    what: 'the release valve armed itself',
  );

  // Strg+Umschalt+F über die Aktionsleiste; über fünf Anfragen fragt das Modal
  // erst nach. Der Gruppenkopf selbst kann nur blocken (CONVENTIONS 4.15).
  await _chord(tester, LogicalKeyboardKey.keyF, control: true, shift: true);
  await pumpUntil(
    tester,
    () => find.byKey(const Key('intercept-batch-modal')).evaluate().isNotEmpty,
    what: 'the modal asks before twelve requests leave',
  );
  await tester.tap(find.byKey(const Key('intercept-batch-confirm')));
  await tester.pump();

  await pumpUntil(
    tester,
    () => _group(container, npmApex) == null,
    timeout: const Duration(seconds: 30),
    what: 'the twelve requests left the queue',
  );

  // Der Temporär-Tab des Regel-Bildschirms führt genau eine Regel, und sie
  // trägt die Herkunft der Anfrage, aus der sie entstand. Das Abzeichen heißt
  // `from <Id>` und nicht `from #1`: Die Flow-Ids des Daemons sind UUIDs, und
  // `rule_row.dart` zeigt davon das letzte Segment. Gesucht wird deshalb der
  // Schlüssel `rule-origin-<Id>` und nicht sein Text.
  await _go(
    tester,
    Section.rules,
    ready: () => find.byKey(const Key('rules-filter')).evaluate().isNotEmpty,
    what: 'the rules screen is on the front',
  );
  await pumpUntil(
    tester,
    () => _sessionRules(container).length == 1,
    timeout: const Duration(seconds: 20),
    what: 'the rule store holds exactly one session rule',
  );
  final Rule sessionRule = _sessionRules(container).single;
  expect(sessionRule.action, RuleAction.allow);
  expect(sessionRule.matcher.host, '**.npmjs.org');
  expect(sessionRule.createdFrom, isNotNull);

  await tester.tap(find.textContaining('Temporary (').first);
  await tester.pump();
  await pumpUntil(
    tester,
    () => find.textContaining('**.npmjs.org').evaluate().isNotEmpty,
    what: 'the temporary tab shows the sentence of the session rule',
  );
  expect(
    find.byWidgetPredicate(
      (Widget widget) =>
          widget.key is ValueKey<String> &&
          (widget.key! as ValueKey<String>).value.startsWith('rule-origin-'),
    ),
    findsOneWidget,
    reason: 'the rule says which request it was made from',
  );

  await _go(
    tester,
    Section.intercept,
    ready: () =>
        find.byKey(const Key('queue-group-$githubApex')).evaluate().isNotEmpty,
    what: 'the queue is on the front again',
  );

  // --- 3. Der AWS-Schlüssel wird geblockt -----------------------------------

  final Flow evil = _group(container, evilApex)!.flows.single;
  await tester.tap(_row(evil.id));
  await tester.pump();

  // Mit Notiz, und deshalb über `Strg+Eingabe`: `N` öffnet das Feld und gibt
  // ihm die Tastatur, ein blankes `B` landete danach im Text statt in der
  // Entscheidung (HUM-072, `note_test.dart` „B does not block while the field
  // has the keyboard"). Der Agent liest dieselbe Zeile im Rumpf des `403` und
  // im Kopf `X-Humanitl-Note`; Abschnitt 5 des Laufs prüft beides.
  await tester.sendKeyEvent(LogicalKeyboardKey.keyN);
  await tester.pump();
  await tester.pump();
  await tester.enterText(
    find.byKey(const Key('intercept-note-input')),
    blockNote,
  );
  await tester.pump();
  await _chord(tester, LogicalKeyboardKey.enter, control: true);
  await pumpUntil(
    tester,
    () => _group(container, evilApex) == null,
    timeout: const Duration(seconds: 20),
    what: 'the request with the AWS key is blocked',
  );

  // --- 4. Die POST-Anfrage mit der Mailadresse wird erlaubt -----------------

  final Flow post = _group(
    container,
    githubApex,
  )!.flows.firstWhere((Flow flow) => flow.path.startsWith('/graphql'));
  expect(post.findingCount, 1, reason: 'the mail address in the body');
  await tester.tap(_row(post.id));
  await tester.pump();
  await pumpUntil(
    tester,
    () => container.read(allowArmedProvider),
    what: 'the valve armed itself for the single request',
  );
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await tester.pump();
  await pumpUntil(
    tester,
    () =>
        _group(container, githubApex) == null ||
        !_group(
          container,
          githubApex,
        )!.flows.any((Flow flow) => flow.id == post.id),
    timeout: const Duration(seconds: 20),
    what: 'the POST left the queue',
  );

  // --- 5. Die GET-Anfrage daneben verfällt ----------------------------------

  // Niemand entscheidet sie. Die Karte sagt es, sobald die Frist um ist; drei
  // Sekunden später räumt die Warteschlange die Zeile ab, also wird hier auf
  // den Text gewartet und nicht auf einen Zustand danach.
  //
  // Die Frist hier muss größer sein als die Haltefrist des Laufs, nicht gleich
  // groß: Die Anfrage geht 4,4 Sekunden nach dem Start hinaus und verfällt
  // 30 Sekunden danach, während dieser Schritt schon nach wenigen Sekunden
  // erreicht ist. Mit 30 Sekunden blieben unter zwei Sekunden Abstand, und je
  // schneller die Schritte davor liefen, desto knapper würde es — ein Test,
  // der auf einem schnellen Rechner scheitert, wäre der falsche Wächter.
  await pumpUntil(
    tester,
    () => find.text('Blocked (timed out)').evaluate().isNotEmpty,
    timeout: const Duration(seconds: 60),
    what: 'the request nobody decided says it ran out of time',
  );

  // --- 6. Die Historie und ihre Filter --------------------------------------

  await _go(
    tester,
    Section.history,
    ready: () =>
        find.byKey(const Key('history-filter-input')).evaluate().isNotEmpty,
    what: 'the history screen is on the front',
  );
  await _filter(tester, container, '');
  await pumpUntil(
    tester,
    () => container.read(historyPageProvider).rows.length == 17,
    timeout: const Duration(seconds: 30),
    what: 'the history holds every request of the run',
  );

  // Gezählt **und** gelesen: Ein Bildschirm, der für jeden Filter dieselbe
  // falsche Zeile zeigt, käme mit einer Zahl allein durch.
  await _filter(tester, container, 'decision:block');
  _expectRows(tester, <String>['$evilHost /exfil'], 'decision:block');

  await _filter(tester, container, 'decision:timed_out');
  _expectRows(tester, <String>['$githubHost /repos/x/y'], 'decision:timed_out');

  await _filter(tester, container, 'findings:>0');
  _expectRows(tester, <String>[
    '$evilHost /exfil',
    '$githubHost /graphql',
  ], 'findings:>0');

  // --- 7. Der Export --------------------------------------------------------

  await _filter(tester, container, '');
  await pumpUntil(
    tester,
    () => container.read(historyPageProvider).rows.length == 17,
    timeout: const Duration(seconds: 20),
    what: 'the unfiltered set is back',
  );

  await tester.tap(find.byKey(const Key('history-export-open')));
  await tester.pump();
  await pumpUntil(
    tester,
    () => find.byKey(const Key('history-export-save')).evaluate().isNotEmpty,
    what: 'the export modal offers its save button',
  );
  await tester.tap(find.byKey(const Key('history-export-save')));
  await tester.pump();
  await pumpUntil(
    tester,
    () =>
        container.read(historyExportProvider).phase == HistoryExportPhase.done,
    timeout: const Duration(seconds: 60),
    what: 'the export finished',
  );
  final HistoryExportState job = container.read(historyExportProvider);
  expect(job.total, 17);
  expect(job.written, contains(harPath));
  expect(
    File(harPath).lengthSync(),
    greaterThan(0),
    reason: 'the export wrote the file the run asked for',
  );
}

/// Der Pfad einer Umgebungsvariablen, oder ein Abbruch mit ihrem Namen.
String _required(Map<String, String> env, String name) {
  final String value = (env[name] ?? '').trim();
  if (value.isEmpty) {
    fail(
      '$name is not set; this test is driven by tests/e2e/m2_first_decision/run.sh',
    );
  }
  return value;
}

/// Die gehaltene Zeile dieses Flusses.
///
/// Über den Typ und nicht über einen Schlüssel: Eine gehaltene Zeile trägt
/// keinen, nur die Bestätigungszeile einer entschiedenen tut das
/// (`queue-strip-<id>`, `queue_row.dart`).
Finder _row(FlowId id) => find.byWidgetPredicate(
  (Widget widget) => widget is QueueRow && widget.flow.id == id,
);

/// Die Gruppe mit diesem Apex, oder null, wenn nichts mehr für sie gehalten wird.
HeldGroup? _group(ProviderContainer container, String apex) {
  for (final HeldGroup group in container.read(heldGroupsProvider).groups) {
    if (group.apex == apex) {
      return group;
    }
  }
  return null;
}

/// Die Regeln, die mit der Sitzung enden.
List<Rule> _sessionRules(ProviderContainer container) => <Rule>[
  for (final Rule rule
      in container.read(rulesProvider).value?.rules ?? const <Rule>[])
    if (rule.expires is RuleExpirySession) rule,
];

/// Wechselt den Abschnitt über die Rail, so wie ein Mensch es täte.
///
/// Gewartet wird auf [ready] und nicht auf eine feste Frist: Wie lange ein
/// Abschnitt braucht, bis er steht, hängt am Rechner, und eine Zahl, die auf
/// diesem Schreibtisch reicht, reicht auf einem ausgelasteten CI-Läufer
/// vielleicht nicht.
Future<void> _go(
  WidgetTester tester,
  Section section, {
  required bool Function() ready,
  required String what,
}) async {
  await tester.tap(find.byType(RailEntry).at(section.index));
  await tester.pump();
  await pumpUntil(tester, ready, what: what);
}

/// Behauptet, dass die Historie genau [wanted] zeigt, und sonst nichts.
///
/// Jeder Eintrag ist „Host Pfad-Anfang". Gelesen wird, was in den Zeilen
/// steht, nicht nur wie viele es sind: Ein Bildschirm, der für jeden Filter
/// dieselbe falsche Zeile zeichnete, käme mit einer Zahl allein durch
/// (`backlog/CONVENTIONS.md` 4.13).
void _expectRows(WidgetTester tester, List<String> wanted, String filter) {
  final List<String> seen = <String>[
    for (final HistoryRow row in tester.widgetList<HistoryRow>(
      find.byType(HistoryRow),
    ))
      '${row.flow.host} ${row.flow.path}',
  ]..sort();
  expect(
    seen,
    hasLength(wanted.length),
    reason: 'the rows "$filter" matched: $seen',
  );
  for (final String want in wanted) {
    expect(
      seen.where((String line) => line.startsWith(want)),
      hasLength(1),
      reason: 'exactly one row "$want" for "$filter", saw $seen',
    );
    // Und der Host steht auch lesbar in der Zeile, nicht nur im Widget.
    expect(
      find.descendant(
        of: find.byType(HistoryRow),
        matching: find.text(want.split(' ').first),
      ),
      findsWidgets,
      reason: 'the host of "$want" is drawn for "$filter"',
    );
  }
}

/// Schreibt [query] in das Filterfeld der Historie und wartet auf die Antwort.
Future<void> _filter(
  WidgetTester tester,
  ProviderContainer container,
  String query,
) async {
  final Finder field = find.byKey(const Key('history-filter-input'));
  // Erst den Fokus, dann der Text, und der Fokus ausdrücklich über den Knoten
  // des Feldes. `enterText` holt ihn nur beim **ersten** Mal: `showKeyboard`
  // merkt sich das zuletzt bediente `EditableText` und tut beim zweiten Aufruf
  // auf dasselbe Feld gar nichts mehr. Dazwischen nimmt der History-Bildschirm
  // die Tastatur für seine Tabelle (`_claimFocusOnceVisible`), und das zweite
  // `enterText` schriebe dann ins Leere — genau so ist der Lauf am 2026-09-12
  // gescheitert. Ein blankes `EditableText` hat auch keinen eigenen
  // Tipp-Erkenner, ein Klick darauf hülfe also nicht.
  final EditableText input = tester.widget<EditableText>(field);
  if (!input.focusNode.hasFocus) {
    input.focusNode.requestFocus();
    await pumpUntil(
      tester,
      () => input.focusNode.hasFocus,
      timeout: const Duration(seconds: 5),
      what: 'the filter field took the keyboard',
    );
  }
  await tester.enterText(field, query);
  await tester.pump();
  expect(
    tester.widget<EditableText>(field).controller.text,
    query,
    reason: 'the filter field took the text before it was submitted',
  );
  await tester.testTextInput.receiveAction(TextInputAction.done);
  await tester.pump();
  await pumpUntil(
    tester,
    () =>
        container.read(historyQueryProvider).filter == query &&
        !container.read(historyPageProvider).loading,
    timeout: const Duration(seconds: 20),
    what: 'the history answered the filter "$query"',
  );
}

/// Drückt [key] mit den genannten Zusatztasten.
Future<void> _chord(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool control = false,
  bool shift = false,
}) async {
  if (control) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  }
  if (shift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyEvent(key);
  if (shift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  if (control) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  }
  await tester.pump();
}

/// Pumpt, bis [ready] wahr ist, und bricht sonst mit [what] ab.
///
/// Der einzige Weg zu warten, den dieser Test kennt. `pumpAndSettle` liefe in
/// einer Oberfläche mit laufenden Animationen nie aus, und ein festes `sleep`
/// wäre entweder zu kurz oder verschenkte das Budget der Haltefrist.
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

/// Schreibt das Bild des Bildschirms nach [directory].
///
/// Nur bei einem Fehlschlag, und nie so, dass ein Fehler hier den echten
/// Fehler verdeckt: Das Bild ist ein Artefakt, kein Beweis.
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
    // Ein Bild, das nicht entsteht, darf den Bericht des Laufs nicht ersetzen.
    // ignore: avoid_print
    print('m2: could not write the screenshot: $error');
  }
}
