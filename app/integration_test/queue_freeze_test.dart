// Die eingefrorene Warteschlange gegen den echten Fake-Daemon (HUM-029,
// HUM-144).
//
// **Warum dieser Test neben `test/features/intercept/queue_freeze_test.dart`
// steht.** Dort ist das Einfrieren mit zwei Ankünften gemessen, gegen den
// Fake-Client im selben Prozess. Das Akzeptanzkriterium von HUM-029 verlangt
// mehr: das mitgelieferte Szenario `fixtures/sessions/npm-install.jsonl` mit
// fünfzehn Flüssen, gespielt vom echten `humanitld --fake`, und die Frage, ob
// unter dem Zeiger wirklich nichts wandert, während sie eintreffen.
//
// Der Lauf braucht einen Bildschirm; `Xvfb :99` genügt (`make
// flutter-test-integration`). Er braucht **kein** Netz und keine Sandbox: Der
// Daemon spielt eine Aufzeichnung ab.
//
// **Gemessen wird mit aufgeklappter Gruppe.** Alle fünfzehn Anfragen des
// Szenarios gehen an denselben Host, und ab drei Flüssen zeichnet die
// Warteschlange sie als **eine** eingeklappte Gruppe (`collapseFrom`,
// `held_groups.dart`). Zählte dieser Test die Einträge der eingeklappten
// Ansicht, bliebe die Zahl bei eins -- auch dann, wenn das Einfrieren ganz
// abgeschaltet wäre. Der Test tippt deshalb erst den Gruppenkopf auf, und
// danach ist jede Zeile der Gruppe eine eigene Zeile des Bildschirms.
//
// **Was der Mutationsprobe zuliebe anders aussieht als sonst.** Die
// Messpunkte laufen über [_Marks.check] und brechen nicht beim ersten roten
// Punkt ab; erst der `expect` darüber lässt den Test fallen. So zeigt ein
// einziger Lauf mit `_frozen = false` in `queue_pane.dart`, welche
// Zusicherungen Zähne haben, statt nur die erste zu nennen. Die Wartepunkte
// davor hängen bewusst an der Zahl der Flüsse, die der **Daemon** hält, und
// nicht an der Pille: Unter der Mutation gäbe es keine Pille, und der Test
// stürbe im Warten, bevor er eine einzige Zeile gemessen hätte. In [_Marks]
// steht nur, was unter dieser Mutation rot wird; eine Zusicherung, die auch
// ohne Einfrieren grün bliebe, steht ausdrücklich daneben und nicht darin.
//
// **Kein `--loop`.** Das Szenario ist unter Zeitraffer in gut zehn Sekunden
// durch, die Anwendung braucht länger bis zum ersten Bild. Gelöst wird das
// über die Reihenfolge -- erst die Anwendung, dann der Daemon auf denselben
// Socket --, nicht über eine Schleife, die das Zeitproblem zudeckt.

import 'dart:async';
import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ipc/launch_options.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/held_groups.dart';
import 'package:humanitl/features/intercept/widgets/group_header_row.dart';
import 'package:humanitl/features/intercept/widgets/new_arrivals_pill.dart';
import 'package:humanitl/features/intercept/widgets/queue_row.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:integration_test/integration_test.dart';

/// Die Auflösung, in der die Selektoren dieses Tests gelten.
///
/// Unter 1400×900 greift das schmale Layout, und die Warteschlange steht dann
/// nicht mehr dort, wo dieser Test sie sucht.
const Size screenSize = Size(1600, 1000);

/// Der Zeitraffer des Szenarios: keiner.
///
/// Das Szenario dauert 20,6 s und stellt seine fünfzehn Anfragen zwischen
/// 0,1 s und 19,4 s. Der Test braucht Ankünfte **hinter** dem Zeiger, und
/// zwischen dem Start des Daemons und dem Einfrieren vergehen der
/// Verbindungsversuch, der Weg über die Rail, der Tipp auf den Gruppenkopf und
/// eine halbe Sekunde Ruhe. Mit Zeitraffer 2 wären das am 2026-09-12 auf
/// diesem Rechner 10 von 15 Anfragen vor dem Einfrieren gewesen, also fünf
/// Sekunden Luft bis zur zwölften -- auf einem langsamen oder belasteten
/// Läufer zu wenig, und der Lauf stürbe in einer Frist statt an einer
/// Zusicherung (Review vom 2026-09-12). Ohne Zeitraffer ist dieselbe Luft
/// mehr als doppelt so groß; was der Lauf wirklich hatte, steht in der
/// Zeile, die er am Ende schreibt.
const int scenarioSpeed = 1;

/// So viele Anfragen hält das Szenario am Ende.
const int scenarioFlows = 15;

/// So viele Ankünfte müssen nach dem Einfrieren noch kommen.
///
/// Zwei wären zu wenig: Die Pille soll wachsen, und dafür braucht es zwei
/// Messungen **und** eine Ankunft dazwischen.
const int arrivalsBehindTheFreeze = 3;

/// Der Daemon dieses Laufs: `humanitld --fake <szenario>`.
class FakeDaemon {
  FakeDaemon._(this.root, this.process, this.socket);

  /// Startet ihn über der Aufzeichnung [scenario] und wartet auf den Socket.
  static Future<FakeDaemon> start(
    String scenario, {
    required String socket,
    int speed = scenarioSpeed,
  }) async {
    final String repo = Directory.current.path.endsWith('/app')
        ? Directory.current.parent.path
        : Directory.current.path;
    final File binary = File('$repo/daemon/target/debug/humanitld');
    if (!binary.existsSync()) {
      fail('daemon/target/debug/humanitld is missing; run cargo build first');
    }
    final Directory root = Directory.systemTemp.createTempSync('hum-fake-');
    try {
      return await _startIn(root, binary, scenario, repo, speed, socket);
    } catch (_) {
      // Was zwischen `createTempSync` und dem laufenden Daemon schiefgeht,
      // ließe sonst einen Baum in `/tmp` stehen: `addTearDown` steht erst
      // beim Aufrufer, und der sieht diesen Fehler nie.
      if (root.existsSync()) {
        root.deleteSync(recursive: true);
      }
      rethrow;
    }
  }

  static Future<FakeDaemon> _startIn(
    Directory root,
    File binary,
    String scenario,
    String repo,
    int speed,
    String socket,
  ) async {
    for (final String path in <String>[
      'runtime/humanitl',
      'data',
      'config',
      'state',
    ]) {
      Directory('${root.path}/$path').createSync(recursive: true);
    }
    // Der Daemon besteht auf 0700 für das Verzeichnis von Socket und Token,
    // und zwar für beide Ebenen (`DAEMON_004`).
    await Process.run('chmod', <String>['700', '${root.path}/runtime']);
    await Process.run('chmod', <String>[
      '700',
      '${root.path}/runtime/humanitl',
    ]);
    // Der Socket liegt dort, wo die Anwendung ihn schon erwartet; sein
    // Verzeichnis gehört zum Aufräumen dieses Laufs.
    await Process.run('chmod', <String>['700', File(socket).parent.path]);
    final Process process = await Process.start(
      binary.path,
      <String>[
        '--socket',
        socket,
        '--fake',
        '$repo/$scenario',
        // Zeitraffer: Die Reihenfolge der Ereignisse bleibt, nur die Abstände
        // schrumpfen. Die Haltefristen skalieren nicht mit (kein
        // `--scale-timeouts`), die fünfzehn Flüsse bleiben also gehalten,
        // solange dieser Test misst.
        '--speed',
        '$speed',
      ],
      environment: <String, String>{
        'XDG_RUNTIME_DIR': '${root.path}/runtime',
        'XDG_DATA_HOME': '${root.path}/data',
        'XDG_CONFIG_HOME': '${root.path}/config',
        'XDG_STATE_HOME': '${root.path}/state',
        'HOME': root.path,
        'PATH': Platform.environment['PATH'] ?? '/usr/local/bin:/usr/bin:/bin',
      },
    );
    final StringBuffer said = StringBuffer();
    process.stdout.listen((List<int> b) => said.write(String.fromCharCodes(b)));
    process.stderr.listen((List<int> b) => said.write(String.fromCharCodes(b)));
    bool exited = false;
    unawaited(process.exitCode.then((int _) => exited = true));
    final DateTime until = DateTime.now().add(const Duration(seconds: 20));
    while (!File(socket).existsSync() &&
        !exited &&
        DateTime.now().isBefore(until)) {
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    if (!File(socket).existsSync()) {
      process.kill(ProcessSignal.sigkill);
      root.deleteSync(recursive: true);
      fail('the fake daemon never opened $socket; it said: $said');
    }
    return FakeDaemon._(root, process, socket);
  }

  final Directory root;
  final Process process;
  final String socket;

  /// Beendet den Daemon und räumt sein Verzeichnis weg.
  ///
  /// **`SIGKILL` und nicht `SIGTERM`.** Der Daemon wartet beim geordneten
  /// Abschied auf die offenen Ströme seiner Clients, und einer davon ist die
  /// Anwendung dieses Tests, die noch steht: Gemessen am 2026-09-12 lief ein
  /// `SIGTERM` in die Frist von fünfzehn Sekunden und endete danach doch im
  /// `SIGKILL`. Gewartet wird auf das Ende des Prozesses, bevor das
  /// Verzeichnis fällt -- sonst löscht der Test unter einem laufenden Prozess
  /// weg. Wer zusieht, wenn er fällt, steht unter „Das Ende" im Rumpf.
  Future<void> stop() async {
    process.kill(ProcessSignal.sigkill);
    await process.exitCode;
    if (root.existsSync()) {
      root.deleteSync(recursive: true);
    }
  }
}

/// Was in der Warteschlange steht: einzelne Zeilen und Gruppenköpfe.
int entries() =>
    find.byType(QueueRow).evaluate().length +
    find.byType(GroupHeaderRow).evaluate().length;

/// Jede gezeichnete Zeile mit ihrem Rechteck, geschlüsselt über den Wert
/// ihres [ValueKey] (`queue_items.dart` baut ihn aus der Fluss-Id).
///
/// Die Karte ist die Zusicherung „nichts wandert unter dem Auge" in einer
/// Zahl: Sie ändert sich, sobald eine Zeile dazukommt, verschwindet oder sich
/// verschiebt.
Map<String, Rect> drawnRows(WidgetTester tester) {
  final Map<String, Rect> rows = <String, Rect>{};
  for (final Element element in find.byType(QueueRow).evaluate()) {
    final QueueRow row = element.widget as QueueRow;
    rows[(row.key! as ValueKey<String>).value] = tester.getRect(
      find.byWidget(row),
    );
  }
  return rows;
}

/// Was die Pille zählt, oder null, wenn keine da ist.
///
/// Gesucht wird über `Key('intercept-new-pill')` und nicht über den Text
/// `+{count} weitere`: Dasselbe Muster steht im Einrichten-Bildschirm, der im
/// `IndexedStack` daneben liegt und mitgezeichnet wird.
int? pillCount(WidgetTester tester) {
  final Finder pill = find.byKey(const Key('intercept-new-pill'));
  if (pill.evaluate().isEmpty) {
    return null;
  }
  return tester
      .widget<NewArrivalsPill>(
        find.ancestor(of: pill, matching: find.byType(NewArrivalsPill)),
      )
      .count;
}

/// Der Kopf der Gruppe [apex] und sein Faltdreieck.
Finder chevronOf(String apex) => find.descendant(
  of: find.byKey(Key('queue-group-$apex')),
  matching: find.byWidgetPredicate(
    (Widget widget) =>
        widget is HGlyphIcon && widget.glyph == HGlyph.chevronRight,
  ),
);

/// Rote Punkte einer Messung, gesammelt statt beim ersten geworfen.
class _Marks {
  final List<String> red = <String>[];

  /// Prüft [actual] gegen [matcher] und merkt sich den Fehlschlag.
  void check(Object? actual, Object? matcher, {required String reason}) {
    try {
      expect(actual, matcher, reason: reason);
    } on TestFailure catch (failure) {
      red.add('$reason -- ${failure.message}');
    }
  }
}

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('no_row_moves_under_the_pointer_while_fifteen_flows_arrive', (
    WidgetTester tester,
  ) async {
    tester.view.physicalSize = screenSize;
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);

    // **Erst die Anwendung, dann der Daemon.** Startete der Daemon zuerst,
    // wäre unter Zeitraffer ein Teil des Szenarios abgespielt, bevor jemand
    // hinsieht, und der Test maße fertige Flüsse statt Ankünfte. Die
    // Anwendung verträgt das -- sie verbindet sich alle zwei Sekunden erneut,
    // sobald der Socket da ist (`connection.dart`).
    final Directory socketRoot = Directory.systemTemp.createTempSync(
      'hum-freeze-',
    );
    addTearDown(() {
      if (socketRoot.existsSync()) {
        socketRoot.deleteSync(recursive: true);
      }
    });
    final String socket = '${socketRoot.path}/daemon.sock';

    await tester.pumpWidget(
      ProviderScope(
        overrides: <Override>[
          launchOptionsProvider.overrideWithValue(
            LaunchOptions(socketPath: socket),
          ),
        ],
        child: const HumanitlApp(),
      ),
    );
    await tester.pump();
    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(HumanitlApp)),
    );
    // Ohne Daemon gibt es keine Shell und keine Rail: `ConnectionGate` zeigt
    // den Einrichten-Bildschirm, bis einer geantwortet hat. Gewartet wird
    // deshalb darauf, dass die Anwendung steht und einmal vergeblich gesucht
    // hat -- erst danach hat der Daemon einen Zuschauer.
    await pumpUntil(
      tester,
      () => container.read(connectionStateProvider) is! ConnectionConnecting,
      timeout: const Duration(seconds: 30),
      what: 'the application is up and looked for a daemon',
    );

    // Und jetzt der Daemon, der auf genau diesen Socket hört.
    final FakeDaemon daemon = await FakeDaemon.start(
      'fixtures/sessions/npm-install.jsonl',
      socket: socket,
    );
    // Das Netz für den Fall, dass eine Messung unten fällt: Der Daemon stirbt
    // dann hier, und kein Prozess und kein Verzeichnis bleibt stehen. Im
    // grünen Lauf ist er längst tot -- der letzte Schritt des Rumpfes legt
    // ihn hin, und das muss er dort tun (siehe „Das Ende" unten).
    addTearDown(daemon.stop);

    // Und sofort nachfragen, statt den Takt von zwei Sekunden abzuwarten
    // (`connectionReconnectProvider`): Das ist derselbe Weg, den „Erneut
    // verbinden" nimmt, und die zwei Sekunden gehören zur Luft, die der Test
    // für Ankünfte hinter dem Zeiger braucht.
    container.read(connectionStateProvider.notifier).retry();
    await pumpUntil(
      tester,
      () => container.read(linkLiveProvider),
      timeout: const Duration(seconds: 30),
      what: 'the daemon answered',
    );

    await pumpUntil(
      tester,
      () => find.byType(IconRail).evaluate().isNotEmpty,
      what: 'the shell drew its rail',
    );
    await goToQueue(tester, container);

    // **Die Gruppe aufklappen.** Ab drei Anfragen an denselben Apex steht die
    // Gruppe eingeklappt (`collapseFrom`); erst der Tipp auf das Faltdreieck
    // macht aus fünfzehn Flüssen fünfzehn Zeilen. Ohne ihn bliebe die Zahl der
    // Einträge bei eins, ganz gleich, ob eingefroren wird oder nicht.
    await pumpUntil(
      tester,
      () => (_group(container)?.length ?? 0) >= 3,
      timeout: const Duration(seconds: 30),
      what: 'the scenario reaches the screen',
    );
    final HeldGroup group = _group(container)!;
    expect(
      group.apex,
      'npmjs.org',
      reason: 'the scenario sends every request to one host',
    );
    expect(
      find.byKey(Key('queue-group-${group.apex}')),
      findsOneWidget,
      reason: 'the burst of the package registry is drawn as a group',
    );
    expect(
      find.byType(QueueRow),
      findsNothing,
      reason: 'a burst of three or more opens folded',
    );
    await tester.tap(chevronOf(group.apex));
    await tester.pump();
    await pumpUntil(
      tester,
      () => find.byType(QueueRow).evaluate().length >= 3,
      what: 'the tap on the group head opened its rows',
    );
    expect(
      container.read(expandedGroupsProvider.notifier).isOpen(group),
      isTrue,
      reason: 'and the group stays open while the rest arrives',
    );

    // --- Der Zeiger steht in der Warteschlange --------------------------------

    final TestGesture pointer = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await pointer.addPointer(location: Offset.zero);
    addTearDown(pointer.removePointer);
    await pointer.moveTo(tester.getCenter(find.byType(QueueRow).first));
    await tester.pump();

    final _Marks marks = _Marks();
    // **Das Fenster zwischen Eintritt und Grundlinie ist selbst eine
    // Messung.** Die Rechtecke werden nicht sofort abgelesen: Zeilen, die
    // kurz vor dem Zeiger hereinkamen, gleiten noch in ihre Lage
    // (`_relayout`, gestaffelt), und ein Rechteck, das mitten in dieser
    // Bewegung abgelesen wird, ist um Bruchteile eines Pixels daneben --
    // gemessen am 2026-09-12: 0,2 px. Gewartet wird deshalb, bis der Daemon
    // eine weitere Anfrage hält, und erst dann wird die Grundlinie
    // genommen. Damit steht fest, dass in diesem Fenster eine Ankunft lag:
    // Was jetzt noch dieselben Zeilen zeigt, hat sie wirklich
    // zurückgehalten, und ohne Einfrieren ist dieser Punkt rot statt
    // zufällig grün.
    final Set<String> keysAtEntry = drawnRows(tester).keys.toSet();
    final int heldAtEntry = container.read(heldFlowsProvider).length;
    await pumpUntil(
      tester,
      () => container.read(heldFlowsProvider).length >= heldAtEntry + 1,
      timeout: const Duration(seconds: 30),
      what: 'a request arrives while the pointer stands in the queue',
    );
    await pumpFor(tester, const Duration(milliseconds: 500));
    marks.check(
      drawnRows(tester).keys.toSet(),
      keysAtEntry,
      reason: 'no row joined while the picture settled under the pointer',
    );

    final int frozenEntries = entries();
    final Map<String, Rect> frozenRows = drawnRows(tester);
    final int heldAtFreeze = container.read(heldFlowsProvider).length;
    // Gemessen werden kann nur, was noch kommt: Ohne Ankünfte hinter dem
    // Zeiger sagt der Lauf nichts über das Einfrieren. Steht hier ein
    // `expect` und kein Messpunkt, weil ein Lauf ohne Rest kein rotes
    // Ergebnis ist, sondern ein ungültiges -- der Rechner war zu langsam, und
    // der Test sagt das, statt dreißig Sekunden in einer Frist zu stehen.
    expect(
      heldAtFreeze,
      lessThanOrEqualTo(scenarioFlows - arrivalsBehindTheFreeze),
      reason:
          'the pointer entered while at least $arrivalsBehindTheFreeze of the '
          '$scenarioFlows requests were still to come',
    );

    // Gewartet wird auf den Daemon, nicht auf die Pille: Unter der
    // Mutationsprobe (`_frozen = false`) gäbe es keine Pille, und der Test
    // stürbe hier, statt die Zeilen zu messen.
    await pumpUntil(
      tester,
      () => container.read(heldFlowsProvider).length >= heldAtFreeze + 1,
      timeout: const Duration(seconds: 30),
      what: 'the first request arrives behind the frozen queue',
    );
    final int? pillAfterFirst = pillCount(tester);
    await pumpUntil(
      tester,
      () =>
          container.read(heldFlowsProvider).length >=
          heldAtFreeze + arrivalsBehindTheFreeze,
      timeout: const Duration(seconds: 30),
      what:
          '$arrivalsBehindTheFreeze more requests arrive behind the frozen '
          'queue',
    );
    final int? pillAfterThree = pillCount(tester);
    // **Bis das Szenario durch ist, und mit reichlich Frist.** Erst wenn
    // nichts mehr kommt, hat der letzte Messpunkt Zähne: Käme nach dem
    // Verlassen des Zeigers noch eine Anfrage, wüchse die Liste auch ohne
    // Zusammenführen. Das Szenario dauert 20,6 s; die Frist ist dreimal so
    // lang, weil dieser Test unter `nice` neben Bauten laufen kann und ein
    // ausgehungerter Fake-Daemon langsamer abspielt (am 2026-09-12 lief ein
    // Lauf unter Last in eine Frist von 40 s).
    final bool played = await pumpWhile(
      tester,
      () => container.read(heldFlowsProvider).length >= scenarioFlows,
      timeout: const Duration(seconds: 60),
    );
    expect(
      played,
      isTrue,
      reason:
          'the scenario played its $scenarioFlows requests; the daemon held '
          '${container.read(heldFlowsProvider).length}',
    );
    final int held = container.read(heldFlowsProvider).length;
    final int entriesAtEnd = entries();
    final int? pillHeldBack = pillCount(tester);

    // --- Was unter dem Zeiger gilt -------------------------------------------

    marks
      ..check(
        entriesAtEnd,
        frozenEntries,
        reason: 'no line joined the queue under the pointer',
      )
      ..check(
        drawnRows(tester),
        frozenRows,
        reason: 'and every row that was drawn stands where it stood',
      )
      ..check(
        pillAfterFirst,
        isNotNull,
        reason: 'the pill appears with the first arrival nobody has seen',
      )
      ..check(
        pillAfterThree,
        greaterThan(pillAfterFirst ?? 0),
        reason: 'and it counts up while they pile up',
      )
      ..check(
        pillHeldBack,
        held - frozenRows.length,
        reason: 'the pill names every request held back from the frozen queue',
      );

    // --- Zeiger weg: Was gewartet hat, kommt herein ---------------------------

    // `Offset(5, 5)` liegt in der Rail und damit außerhalb der Warteschlange;
    // bei kleineren Fenstern träfe die Stelle die Kopfzeile, deshalb steht die
    // Auflösung dieses Tests fest ([screenSize]).
    await pointer.moveTo(const Offset(5, 5));
    await tester.pump();
    final bool merged = await pumpWhile(
      tester,
      () =>
          entries() > entriesAtEnd &&
          find.byKey(const Key('intercept-new-pill')).evaluate().isEmpty,
      timeout: const Duration(seconds: 15),
    );
    marks
      ..check(
        merged,
        isTrue,
        reason: 'the queue merges once the pointer leaves',
      )
      // Gegen [entriesAtEnd] und nicht gegen [frozenEntries]: Unter der
      // Mutationsprobe sind die Ankünfte längst in der Liste, und ein
      // Vergleich mit dem Stand vor ihnen wäre auch dann grün.
      ..check(
        entries(),
        greaterThan(entriesAtEnd),
        reason: 'the arrivals came in once the pointer left',
      )
      // Gegen die vorher gemessene Pille und nicht gegen ihr blosses Fehlen:
      // Ohne Einfrieren erscheint sie nie, und „sie ist fort" wäre dann grün,
      // ohne etwas zu bewachen (Review vom 2026-09-12).
      ..check(
        pillAfterFirst != null && pillCount(tester) == null,
        isTrue,
        reason: 'the pill that counted the arrivals is gone once they came in',
      );
    final Set<String> keysAfterMerge = drawnRows(tester).keys.toSet();

    // Die Zahlen dieses Laufs, damit der Bericht sie trägt und nicht nur ein
    // grüner Haken dasteht. Sie sind keine Konstanten: Wie viele Anfragen beim
    // Einfrieren schon da waren, hängt daran, wie schnell die Anwendung stand.
    // ignore: avoid_print
    print(
      'queue_freeze: froze at $heldAtFreeze of $scenarioFlows requests, '
      '$frozenEntries entries frozen '
      '(${frozenRows.length} rows under one head), $held flows held, '
      'pill ${pillAfterFirst ?? 0} to ${pillAfterThree ?? 0} to '
      '${pillHeldBack ?? 0}, ${entries()} entries after the merge, '
      'pill ${pillCount(tester) == null ? 'gone' : 'still there'}',
    );

    // --- Das Ende ------------------------------------------------------------

    // **Der Daemon fällt hier und nicht in einem `addTearDown`.** Nach dem
    // letzten Bild des Rumpfes räumt das Testgerüst den Baum selbst ab
    // (gemessen am 2026-09-12: `flowEventsProvider` wird abgeräumt, bevor der
    // erste `addTearDown` läuft). Der abbestellte Ereignisstrom ist damit ein
    // `async*`-Erzeuger, der in seinem `await for` hängt: Dart beendet ihn
    // erst beim nächsten `yield`, er hält also weiter eine offene
    // gRPC-Antwort. Stirbt der Daemon danach, wirft dieser Erzeuger
    // `DAEMON_001` ins Leere, und das Testgerüst zählt die Ausnahme als
    // Fehlschlag -- gleich, in welcher Reihenfolge die `addTearDown` stehen;
    // gemessen in fünf Läufen am 2026-09-12, auch mit `SIGTERM` (der Daemon
    // wartet dann auf genau diesen Strom und läuft in die Frist).
    //
    // Solange die Anwendung steht, nimmt **sie** den Verlust entgegen: Ihr
    // Hörer hat einen `onError`, das Banner sagt es, und der Bericht bleibt
    // sauber. Also stirbt der Daemon, während sie zusieht, und der Test
    // behauptet, dass sie es gemerkt hat.
    await daemon.stop();
    await pumpUntil(
      tester,
      () => !container.read(linkLiveProvider),
      timeout: const Duration(seconds: 15),
      what: 'the application noticed that the daemon is gone',
    );
    // Und noch einen Moment stehenbleiben, damit der Wiederverbinder seinen
    // ersten vergeblichen Versuch hinter sich bringt, solange sein Hörer noch
    // da ist.
    await pumpFor(tester, const Duration(seconds: 1));

    expect(
      marks.red,
      isEmpty,
      reason:
          'the frozen queue was measured; every line above is a measurement '
          'that did not hold',
    );

    // Und zum Schluss eine Bedingung, die **nicht** zur Messung des
    // Einfrierens gehört und deshalb nicht in [_Marks] steht: Sie behauptet
    // eine Obermenge und bleibt auch dann grün, wenn gar nicht eingefroren
    // wird. Bewacht wird das Zusammenführen -- keine Zeile, die eingefroren
    // stand, geht dabei verloren. Sie steht hinter dem `expect` oben, damit
    // ein roter Lauf zuerst seine Messpunkte nennt.
    expect(
      keysAfterMerge,
      containsAll(frozenRows.keys),
      reason: 'no row that was frozen was lost in the merge',
    );
  }, timeout: const Timeout(Duration(minutes: 3)));
}

/// Klickt die Warteschlange auf den Schirm, so wie ein Mensch es täte.
///
/// Der Tipp wird bis zu fünfmal wiederholt und danach eine Sekunde gehalten,
/// weil `_offerSetup` (`shell_screen.dart`) die Weiche einmal auf den
/// Einrichten-Bildschirm stellen kann, sobald seine vier Prüfungen antworten
/// -- und das kann nach dem ersten Tipp geschehen. Gefragt wird
/// [navigationProvider] und nicht der Baum: Die Abschnitte stehen in einem
/// `IndexedStack`, der sie alle baut und nur einen zeigt.
Future<void> goToQueue(WidgetTester tester, ProviderContainer container) async {
  for (int attempt = 0; attempt < 5; attempt++) {
    await tester.tap(
      find.byWidgetPredicate(
        (Widget widget) =>
            widget is RailEntry && widget.section == Section.intercept,
      ),
    );
    await pumpFor(tester, const Duration(seconds: 1));
    if (container.read(navigationProvider) == Section.intercept) {
      return;
    }
  }
  fail('the rail did not keep the queue on screen');
}

/// Die eine Gruppe des Szenarios, oder null, solange nichts gehalten wird.
HeldGroup? _group(ProviderContainer container) {
  final List<HeldGroup> groups = container.read(heldGroupsProvider).groups;
  return groups.isEmpty ? null : groups.first;
}

/// Pumpt, bis [ready] wahr ist, und sagt, ob es dazu kam.
///
/// `pumpAndSettle` liefe in dieser Oberfläche nie aus: Die Warteschlange
/// animiert dauernd (Countdown, Puls des Zählers).
Future<bool> pumpWhile(
  WidgetTester tester,
  bool Function() ready, {
  Duration timeout = const Duration(seconds: 20),
  Duration step = const Duration(milliseconds: 100),
}) async {
  final Stopwatch clock = Stopwatch()..start();
  while (clock.elapsed < timeout) {
    if (ready()) {
      return true;
    }
    await tester.pump(step);
  }
  return ready();
}

/// Pumpt [duration] lang weiter, ohne auf etwas zu warten.
Future<void> pumpFor(WidgetTester tester, Duration duration) async {
  final Stopwatch clock = Stopwatch()..start();
  while (clock.elapsed < duration) {
    await tester.pump(const Duration(milliseconds: 100));
  }
}

/// Pumpt, bis [ready] wahr ist, und bricht sonst mit [what] ab.
Future<void> pumpUntil(
  WidgetTester tester,
  bool Function() ready, {
  required String what,
  Duration timeout = const Duration(seconds: 20),
  Duration step = const Duration(milliseconds: 100),
}) async {
  if (!await pumpWhile(tester, ready, timeout: timeout, step: step)) {
    fail('$what did not happen within ${timeout.inSeconds}s');
  }
}
