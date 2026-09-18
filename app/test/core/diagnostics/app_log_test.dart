// Das Protokoll, in dem die Anwendung ihr eigenes Ende aufschreibt
// (HUM-136). Geprüft wird, was nach einem verschwundenen Fenster in der Datei
// stehen muss: eine Startzeile, eine Ausnahme mit ihrem Text, und beides in
// einer Datei, die nicht wächst, bis die Platte voll ist.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/diagnostics/app_log.dart';

void main() {
  late Directory home;
  late AppLog log;

  setUp(() {
    home = Directory.systemTemp.createTempSync('humanitl-hum136-');
    log = AppLog('${home.path}/humanitl/${AppLog.fileName}');
  });

  tearDown(() {
    if (home.existsSync()) {
      home.deleteSync(recursive: true);
    }
  });

  group('der Ort der Datei', () {
    test('folgt XDG_STATE_HOME, wenn es gesetzt ist', () {
      final AppLog resolved = AppLog.resolve(
        environment: const <String, String>{
          'XDG_STATE_HOME': '/state',
          'HOME': '/home/someone',
        },
      );
      expect(resolved.path, '/state/humanitl/app.log');
    });

    test('fällt auf ~/.local/state zurück, nie auf /tmp', () {
      final AppLog resolved = AppLog.resolve(
        environment: const <String, String>{'HOME': '/home/someone'},
      );
      expect(resolved.path, '/home/someone/.local/state/humanitl/app.log');
      expect(resolved.path, isNot(startsWith('/tmp')));
    });

    test('die Rotation liegt neben der Datei', () {
      expect(log.rotatedPath, endsWith(AppLog.rotatedFileName));
      expect(
        log.rotatedPath,
        '${log.path.substring(0, log.path.length - AppLog.fileName.length)}'
        '${AppLog.rotatedFileName}',
      );
    });
  });

  group('was in der Datei steht', () {
    test('der Start nennt Zeitpunkt, Version und Prozesskennung', () {
      log.started(version: '1.2.3');

      final List<String> lines = log.readLines();
      expect(lines, hasLength(1));
      expect(lines.single, contains(' ${AppLog.kindStart} '));
      expect(lines.single, contains('version=1.2.3'));
      expect(lines.single, contains('pid=$pid'));
      expect(
        DateTime.tryParse(lines.single.split(' ').first),
        isNotNull,
        reason: 'die Zeile beginnt mit einem lesbaren Zeitpunkt',
      );
    });

    test('das geordnete Ende ist als solches erkennbar', () {
      log
        ..started(version: '1.2.3')
        ..stopped();

      final List<String> lines = log.readLines();
      expect(lines, hasLength(2));
      expect(lines.last, contains(' ${AppLog.kindStop} '));
    });

    test('eine Ausnahme trägt ihren Text und ihre Herkunft', () {
      log.started(version: '1.2.3');
      log.exception(
        StateError('the view is gone'),
        stack: StackTrace.fromString('frame one\nframe two'),
        source: 'test',
      );

      final List<String> lines = log.readLines();
      expect(lines, hasLength(2));
      expect(lines.last, contains(' ${AppLog.kindException} '));
      expect(lines.last, contains('source=test'));
      expect(lines.last, contains('the view is gone'));
      expect(lines.last, contains('frame one frame two'));
    });

    test('ein Umbruch im Text macht nie eine zweite Zeile', () {
      log.exception(
        Exception('erste Zeile\nzweite Zeile\r\ndritte'),
        source: 'test',
      );

      expect(log.readLines(), hasLength(1));
      expect(log.readLines().single, contains('erste Zeile zweite Zeile'));
    });

    test('eine sehr lange Zeile wird gekürzt, nicht angehängt', () {
      // Wörter mit Leerzeichen, keine lange Kette: Eine Kette entfernt die
      // Redaktion, und dann misst dieser Test sie statt der Kürzung.
      final String long = List<String>.filled(
        AppLog.maxLineBytes,
        'word',
      ).join(' ');
      log.exception(Exception(long), source: 'test');

      final List<String> lines = log.readLines();
      expect(lines, hasLength(1));
      expect(
        utf8.encode(lines.single).length,
        lessThanOrEqualTo(AppLog.maxLineBytes),
      );
      expect(lines.single, endsWith(AppLog.cutMarker));
    });

    test('dieselbe Ausnahme wird gezählt, nicht wiederholt', () {
      for (int i = 0; i < 60; i++) {
        log.exception(StateError('every frame'), source: 'test');
      }
      expect(log.readLines(), hasLength(1));

      log.exception(StateError('something else'), source: 'test');

      final List<String> lines = log.readLines();
      expect(lines, hasLength(3));
      expect(lines[1], contains('repeated=59'));
      expect(lines[1], contains('every frame'));
      expect(lines[2], contains('something else'));
    });
  });

  group('die Obergrenze', () {
    test('300 KiB hinterlassen zwei Dateien, die neuere unter 256 KiB', () {
      // Jede Zeile trägt eine andere Zahl, damit die Zusammenfassung gleicher
      // Ausnahmen hier nichts misst, was sie nicht messen soll, und Wörter
      // statt einer Kette, damit die Redaktion die Füllung stehen lässt.
      final String padding = List<String>.filled(100, 'pad').join(' ');
      int written = 0;
      int index = 0;
      while (written < 300 * 1024) {
        final String text = '$index $padding';
        log.exception(Exception(text), source: 'test');
        written += text.length + 40;
        index++;
      }

      final File current = File(log.path);
      final File rotated = File(log.rotatedPath);
      expect(current.existsSync(), isTrue);
      expect(rotated.existsSync(), isTrue);
      expect(current.lengthSync(), lessThan(AppLog.maxBytes));
      expect(rotated.lengthSync(), lessThanOrEqualTo(AppLog.maxBytes));
      expect(
        current.lengthSync() + rotated.lengthSync(),
        lessThan(2 * AppLog.maxBytes),
        reason: 'das Protokoll kostet nie mehr als 512 KiB',
      );
      expect(
        Directory('${home.path}/humanitl').listSync(),
        hasLength(2),
        reason: 'genau eine Rotation, nicht mehr',
      );
      expect(
        log.readLines().last,
        contains('${index - 1} '),
        reason: 'die jüngste Zeile steht in der jüngeren Datei',
      );
    });
  });

  group('wer sie lesen darf', () {
    test('Datei 0600, Verzeichnis 0700', () {
      log.started(version: '1.2.3');

      expect(File(log.path).statSync().modeString(), 'rw-------');
      expect(
        Directory('${home.path}/humanitl').statSync().modeString(),
        'rwx------',
        reason: 'ein anderes Konto kommt nicht einmal in das Verzeichnis',
      );
    });

    test('auch die Datei nach einer Rotation steht auf 0600', () {
      // Der teuerste der drei Fälle: `_prepare` läuft **vor** der Rotation,
      // und danach ist die Datei fort. Die nächste Zeile legt sie über
      // `FileMode.append` neu an -- mit der Maske des Prozesses. Rotierte die
      // letzte Zeile vor einem Absturz, läge das Protokoll bis zum nächsten
      // Start offen da, und der Runner repariert es nicht.
      // **Gemessen genau nach der rotierenden Zeile und keiner weiteren.**
      // Die nächste repariert den Modus beim Vorbereiten wieder, und dann
      // misst der Test die Reparatur statt der Lücke -- die Lücke aber ist
      // der Fall: Rotiert die letzte Zeile vor einem Absturz, kommt keine
      // nächste mehr.
      final String padding = List<String>.filled(100, 'pad').join(' ');
      final File rotated = File(log.rotatedPath);
      int index = 0;
      while (!rotated.existsSync()) {
        log.exception(Exception('$index $padding'), source: 'test');
        index++;
        if (index > 10000) {
          fail('keine Rotation nach $index Zeilen');
        }
      }

      expect(File(log.path).statSync().modeString(), 'rw-------');
      expect(rotated.statSync().modeString(), 'rw-------');
    });

    test('eine offene Datei aus einer älteren Fassung wird zugezogen', () {
      Directory('${home.path}/humanitl').createSync(recursive: true);
      File(log.path).writeAsStringSync('alte Zeile\n');
      Process.runSync('chmod', <String>['644', log.path]);
      expect(File(log.path).statSync().modeString(), 'rw-r--r--');

      log.started(version: '1.2.3');

      expect(File(log.path).statSync().modeString(), 'rw-------');
    });

    test('ein Verzeichnis, das nicht uns gehört, kostet die Datei nichts', () {
      // Das Verzeichnis ist `/tmp`: Es gehört dem Systemverwalter und trägt
      // fremde Bits, also scheitert `chmod` daran mit `EPERM` -- und darf der
      // Datei darin trotzdem nichts wegnehmen. Genau das war der Fehler
      // hinter einer gemeinsamen Marke für beide Ziele: Ein Verzeichnis, an
      // dem `chmod` scheitert, ließ das Protokoll für den ganzen Lauf offen
      // lesbar da.
      //
      // Als `root` würde dieses `chmod` gelingen und `/tmp` zunageln; dann
      // misst der Test nichts und sagt das.
      final ProcessResult id = Process.runSync('id', <String>['-u']);
      if ((id.stdout as String).trim() == '0') {
        markTestSkipped('als root würde chmod auf /tmp gelingen');
        return;
      }
      final Directory system = Directory.systemTemp;
      expect(
        system.statSync().mode & 0x3f,
        isNot(0),
        reason: 'ohne fremde Bits gäbe es hier nichts zu versuchen',
      );

      final AppLog inSystemTemp = AppLog(
        '${system.path}/humanitl-hum136-$pid.log',
      );
      addTearDown(() {
        final File file = File(inSystemTemp.path);
        if (file.existsSync()) {
          file.deleteSync();
        }
      });
      final File before = File(inSystemTemp.path);
      if (before.existsSync()) {
        before.deleteSync();
      }

      inSystemTemp.started(version: '1.2.3');

      expect(File(inSystemTemp.path).statSync().modeString(), 'rw-------');
    }, skip: !Platform.isLinux);

    test('der Pfad einer Anfrage steht nicht in der Zeile', () {
      log.exception(
        Exception(
          'GET https://api.example.com/v1/messages?key=sk-abcdef failed',
        ),
        source: 'test',
      );

      final String line = log.readLines().single;
      expect(line, contains('https://api.example.com/'));
      expect(line, isNot(contains('/v1/messages')));
      expect(line, isNot(contains('sk-abcdef')));
    });

    test('die Quelle einer FormatException bleibt draußen', () {
      log.exception(
        const FormatException(
          'Unexpected character',
          '{"authorization": "Bearer super-secret-value"}',
          5,
        ),
        source: 'test',
      );

      final String line = log.readLines().single;
      expect(line, contains('FormatException: Unexpected character'));
      expect(line, contains('offset 5'));
      expect(line, isNot(contains('super-secret-value')));
    });
  });

  group('die Rotation ersetzt, statt zu löschen', () {
    test('eine vorhandene app.log.1 wird überschrieben', () {
      // `renameSync` ersetzt das Ziel in einem Schritt. Der Test misst das
      // Ergebnis; das Fenster, das ein Löschen davor aufmachte, ist von einem
      // Prozess aus nicht zu sehen -- es zählt gegen den Runner, der dieselbe
      // Datei rotiert.
      Directory('${home.path}/humanitl').createSync(recursive: true);
      File(log.rotatedPath).writeAsStringSync('alte Rotation\n');
      final String padding = List<String>.filled(100, 'pad').join(' ');
      int written = 0;
      while (written < AppLog.maxBytes) {
        final String text = '$written $padding';
        log.exception(Exception(text), source: 'test');
        written += text.length + 40;
      }

      expect(File(log.rotatedPath).existsSync(), isTrue);
      expect(
        File(log.rotatedPath).readAsStringSync(),
        isNot(contains('alte Rotation')),
      );
      expect(File(log.path).existsSync(), isTrue);
    });

    test('kein Löschen vor dem Umbenennen, auf beiden Seiten', () {
      // Das Ergebnis eines Löschens mit anschließendem Umbenennen ist
      // dasselbe; verschieden ist nur der Augenblick dazwischen, in dem es
      // keine Rotation gibt -- und den sieht nur ein zweiter Prozess, der
      // genau dann `stat` ruft. Messbar ist stattdessen die Quelle, mit
      // demselben Griff wie bei der Obergrenze: Zwei Rotierer teilen diese
      // Datei, und beide müssen in einem Schritt umbenennen.
      final String dart = File('lib/core/diagnostics/app_log.dart')
          .readAsStringSync();
      final int start = dart.indexOf('void _rotate(File file)');
      expect(start, greaterThan(0), reason: '_rotate steht nicht mehr da');
      final String body = dart.substring(start, dart.indexOf('\n  }', start));
      expect(
        body,
        isNot(contains('deleteSync')),
        reason: 'rename ersetzt das Ziel in einem Schritt',
      );
      expect(body, contains('renameSync'));

      final String runner = File('linux/runner/exit_log.cc').readAsStringSync();
      expect(
        runner,
        isNot(contains('unlink(g_rotated_path)')),
        reason: 'dieselbe Regel im Runner, und er rotiert dieselbe Datei',
      );
      expect(runner, contains('rename(g_path, g_rotated_path)'));
    });
  });

  group('zwei Schreiber, eine Grenze', () {
    test('der Runner kennt dieselbe Obergrenze wie AppLog', () {
      // Die Datei hat zwei Schreiber: diese Klasse und
      // `linux/runner/exit_log.cc`. Die Zusicherung „nie mehr als 512 KiB"
      // gilt nur, solange beide dieselbe Zahl kennen, und eine Zahl, die an
      // zwei Stellen steht, geht auseinander. Der C++-Test prüft die Richtung
      // C -> Dart, indem er die Zahl ausschreibt; dieser Test prüft die
      // andere Richtung und läuft im selben `flutter test` wie alles andere.
      final File runner = File('linux/runner/exit_log.cc');
      expect(
        runner.existsSync(),
        isTrue,
        reason: 'relativ zu app/, dem Arbeitsverzeichnis von flutter test',
      );
      // Zwei Schreibweisen, damit eine Umformatierung den Test nicht rot
      // macht und dabei etwas anderes behauptet, als er misst: ein Produkt
      // (`256L * 1024L`, auch ohne Suffixe und ohne Leerzeichen) und eine
      // einzelne Zahl (`262144`).
      final String source = runner.readAsStringSync();
      final RegExpMatch? product = RegExp(
        r'kMaxBytes\s*=\s*(\d+)L?\s*\*\s*(\d+)L?',
      ).firstMatch(source);
      final RegExpMatch? single = RegExp(r'kMaxBytes\s*=\s*(\d+)L?\s*;')
          .firstMatch(source);
      expect(
        product ?? single,
        isNotNull,
        reason: 'kMaxBytes steht nicht mehr in einer Form, die hier ankommt',
      );
      final int fromRunner = product != null
          ? int.parse(product.group(1)!) * int.parse(product.group(2)!)
          : int.parse(single!.group(1)!);
      expect(fromRunner, AppLog.maxBytes);
    });
  });

  group('ein Protokoll wirft nie', () {
    test('ein Fehler, dessen toString wirft, kommt nicht heraus', () {
      expect(
        () => log.exception(_Unprintable(), source: 'test'),
        returnsNormally,
        reason: 'sonst wäre das Protokoll der Grund für das nächste Ende',
      );
    });

    test('ein unbeschreibbarer Ort kostet den Lauf nichts', () {
      final AppLog broken = AppLog('${home.path}/file/humanitl/app.log');
      File('${home.path}/file').writeAsStringSync('not a directory');

      expect(() => broken.started(version: '1.2.3'), returnsNormally);
      expect(broken.readLines(), isEmpty);
    });
  });
}

/// Ein Fehler, dessen `toString()` selbst wirft. Es gibt ihn: ein Objekt, das
/// seine Meldung aus einem Zustand baut, den der Fehler gerade zerstört hat.
class _Unprintable implements Exception {
  @override
  String toString() => throw StateError('even the message is broken');
}
