// Was aus einem Fehlertext verschwindet, bevor er im Protokoll steht
// (HUM-136).
//
// `docs/SECURITY.md` 8 nennt, was nie in einem Protokoll steht: Bodies,
// Header, Klartext-Werte von Funden, die Notiz einer Blockierung, und der Pfad
// einer Anfrage nur als Streuwert. Der Text einer Ausnahme ist die Stelle, an
// der so etwas hineingerät, und diese Tests sind die Messung dafür.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/diagnostics/redaction.dart';

void main() {
  group('URLs', () {
    test('der Host bleibt, der Pfad geht', () {
      expect(
        redactForLog('failed: https://api.example.com/v1/chat/completions'),
        'failed: https://api.example.com/$removedPathMarker',
      );
    });

    test('die Abfrage geht mit dem Pfad', () {
      final String out = redactForLog(
        'GET http://host.example/search?q=my+secret+query&token=abc',
      );
      expect(out, contains('http://host.example/'));
      expect(out, isNot(contains('my+secret+query')));
      expect(out, isNot(contains('token=abc')));
    });

    test('eine Anmeldung in der URL geht', () {
      expect(
        redactForLog('https://alice:s3cr3t@api.example.com/v1/chat'),
        'https://$redactedMarker@api.example.com/$removedPathMarker',
      );
    });

    test('eine URL ohne Pfad bleibt, wie sie ist', () {
      expect(
        redactForLog('cannot reach https://example.com'),
        'cannot reach https://example.com',
      );
      expect(
        redactForLog('cannot reach https://example.com/'),
        'cannot reach https://example.com/',
      );
    });

    test('ein Stapelabzug behält seine Pfade', () {
      const String frame =
          '#0      main (package:humanitl/core/diagnostics/app_log.dart:42:3)';
      expect(redactForLog(frame), frame);
    });
  });

  group('Anmeldungen und Geheimnisse', () {
    test('ein Bearer-Token geht', () {
      final String out = redactForLog(
        'authorization: Bearer eyJhbGciOi.JIUzI1NiIs',
      );
      expect(out, startsWith('authorization=$redactedMarker'));
      expect(out, isNot(contains('eyJhbGciOi')));
    });

    test('ein Bearer ohne Kopfzeilenname geht auch', () {
      expect(
        redactForLog('retrying with Bearer abc.def-ghi'),
        'retrying with Bearer $redactedMarker',
      );
    });

    test('eine Zuweisung an einen verdächtigen Namen geht', () {
      expect(
        redactForLog('api_key=sk-live-123 and password: hunter2'),
        'api_key=$redactedMarker and password=$redactedMarker',
      );
    });

    test('JSON mit Anführungszeichen um den Namen geht auch', () {
      // Der Fall, wegen dem dieses Modul existiert: eine `FormatException`
      // über einem Rumpf. Ein Muster, das den Doppelpunkt direkt hinter dem
      // Namen erwartet, findet ihn nicht.
      final String json = redactForLog(
        'FormatException on {"password": "hunter2", "user": "bob"}',
      );
      expect(json, isNot(contains('hunter2')));
      expect(json, contains(redactedMarker));

      final String dartMap = redactForLog(
        "bad body {'token': 'abc123', 'x': 1}",
      );
      expect(dartMap, isNot(contains('abc123')));
      expect(dartMap, contains(redactedMarker));
    });

    test('ein Wert mit Leerzeichen geht ganz, nicht bis zum ersten', () {
      final String out = redactForLog('{"secret": "the launch code is 4711"}');
      expect(out, isNot(contains('launch')));
      expect(out, isNot(contains('4711')));
      expect(out, contains('secret=$redactedMarker'));
    });

    test('auch in einfachen Anführungszeichen', () {
      final String out = redactForLog("{'password': 'two words here'}");
      // Nicht nur der ganze Satz: Die nackte Formträfe `'two` und ließe den
      // Rest stehen, und dann wäre diese Zusicherung grün, ohne die
      // eingefasste Form zu messen.
      expect(out, isNot(contains('words')));
      expect(out, isNot(contains('here')));
      expect(out, contains('password=$redactedMarker'));
    });

    test('ein Wert, dessen Anführungszeichen nie schließt, geht trotzdem', () {
      // Abgeschnittener Text, eine Meldung, die nach dem Wert endet: Die
      // eingefassten Alternativen greifen hier nicht, und ohne das führende
      // Anführungszeichen an der nackten Form ginge gar nichts.
      for (final String open in <String>[
        '{"password": "hunter2',
        "{'password': 'hunter2",
        r'{"password": "hunter2\"}',
      ]) {
        final String out = redactForLog(open);
        expect(out, isNot(contains('hunter2')), reason: open);
        expect(out, contains('password=$redactedMarker'), reason: open);
      }
    });

    test('ein Umbruch im Wert beendet ihn, statt ihn zu verschlucken', () {
      // In `AppLog` kommt so etwas nie an -- `_oneLine` macht vorher aus allem
      // eine Zeile --, aber dieses Modul steht für sich. Der Wert geht bis zum
      // Ende **seiner** Zeile, ganz: nicht nur das erste Wort. Was auf der
      // nächsten Zeile steht, ist mit Absicht nicht mehr Teil des Werts; eine
      // Alternative, die über die Zeile hinausläse, verlöre den Anker dort.
      final String out = redactForLog('{"password": "line one\nhunter2"}');
      expect(out, isNot(contains('line')));
      expect(out, isNot(contains('one')));
      expect(out, contains('password=$redactedMarker'));
    });

    test('ein geheimer Name innerhalb eines JSON-Strings geht auch', () {
      // JSON in JSON, aber diesmal mit dem Geheimnis innen: Name und Wert
      // stehen in maskierten Anführungszeichen. Keiner dieser drei Fälle
      // wurde vor dem 2026-09-18 gefunden.
      for (final String escaped in <String>[
        r'{"body":"{\"password\":\"hunter2\"}"}',
        r'{\"password\": \"hunter2 two\"}',
        r'password=\"hunter2\"',
      ]) {
        final String out = redactForLog(escaped);
        expect(out, isNot(contains('hunter2')), reason: escaped);
        expect(out, isNot(contains('two')), reason: escaped);
        expect(out, contains('password=$redactedMarker'), reason: escaped);
      }
    });

    test('ein offenes Anführungszeichen nimmt den Rest der Zeile mit', () {
      // Die nackte Form endet am Leerzeichen; ein Wert, der ein
      // Anführungszeichen öffnet und nie schließt, darf deshalb nicht bei ihr
      // landen. In `AppLog` ist das der Normalfall: `_oneLine` und der
      // Stapelabzug dahinter lassen oft kein schließendes Zeichen übrig.
      for (final String open in <String>[
        '"password" : "hunter2 with space',
        "password: 'abc def",
      ]) {
        final String out = redactForLog(open);
        expect(out, isNot(contains('hunter2')), reason: open);
        expect(out, isNot(contains('with space')), reason: open);
        expect(out, isNot(contains('abc')), reason: open);
        expect(out, isNot(contains('def')), reason: open);
      }
    });

    test('ein maskiertes Anführungszeichen beendet den Wert nicht', () {
      final String out = redactForLog(r'{"password": "foo\"bar"}');
      expect(
        out,
        isNot(contains('bar')),
        reason: 'ein Muster, das am ersten \\" abbricht, lässt den Rest stehen',
      );
      expect(out, contains('password=$redactedMarker'));
    });

    test('JSON in JSON geht ganz', () {
      // Der teuerste Fall: Die eingefasste Form steht vor der nackten, also
      // wäre ein Abbruch am ersten `\"` schlechter als gar keine Alternative.
      final String out = redactForLog(r'{"secret": "{\"nested\": \"value\"}"}');
      expect(out, isNot(contains('nested')));
      expect(out, isNot(contains('value')));
    });

    test('ein Name mit Unterstrich rutscht nicht durch', () {
      // `\btoken\b` trifft `access_token` nicht: Der Unterstrich ist ein
      // Wortzeichen, also fängt dort kein Wort an.
      final String out = redactForLog('/v1/messages?access_token=sk-live-1');
      expect(out, isNot(contains('sk-live-1')));
      expect(out, contains(redactedMarker));
    });

    test('ein eingefasster Kopfzeilen-Wert geht mit allen Paaren', () {
      final String out = redactForLog("{'cookie': 'sid=abc; theme=dark'}");
      expect(out, isNot(contains('sid=abc')));
      expect(out, isNot(contains('theme=dark')));
    });

    test('ein langer Lauf geht, ein kurzes Wort bleibt', () {
      final String out = redactForLog('body: ${'A' * secretRunLength} ok');
      expect(out, 'body: $redactedMarker ok');
      expect(redactForLog('body: kurz ok'), 'body: kurz ok');
    });
  });

  group('der Stapelabzug bleibt', () {
    test('ein offener Wert endet vor dem Stapel, den AppLog anhängt', () {
      // `AppLog` schreibt `Meldung | #0 …` in eine Zeile. Ein Wert, dessen
      // Anführungszeichen nie schließt, nähme sonst jeden Rahmen mit, und
      // übrig bliebe nur der Typ der Ausnahme.
      const String line =
          'Exception: Unexpected response {"token": "eyJhbGciOi '
          '| #0 Client.login (package:humanitl/core/ipc/client.dart:88:7) '
          '#1 main (package:humanitl/main.dart:12:3)';
      final String out = redactForLog(line);
      expect(out, isNot(contains('eyJhbGciOi')));
      expect(
        out,
        contains(
          '#0 Client.login (package:humanitl/core/ipc/client.dart:88:7)',
        ),
      );
      expect(out, contains('#1 main (package:humanitl/main.dart:12:3)'));
    });

    test('ein geschlossener maskierter Wert endet nicht am Trennzeichen', () {
      // Der Halt vor ` | #` und einer Ziffer gilt nur offenen Werten. Ein
      // maskierter Wert, der schließt, geht ganz -- auch wenn diese Folge
      // mitten darin steht.
      for (final String closed in <String>[
        r'{"body":"{\"password\":\"hunter2 | #1 leakme\"}"}',
        r"{'body':'{\'password\':\'hunter2 | #1 leakme\'}'}",
      ]) {
        final String out = redactForLog(closed);
        expect(out, isNot(contains('hunter2')), reason: closed);
        expect(out, isNot(contains('leakme')), reason: closed);
      }
    });

    test('ein Setter im Stapel behält Datei und Zeile', () {
      // `token=` ist hier der Name einer Methode, kein Wert: Die öffnende
      // Klammer beendet die nackte Form, bevor sie anfängt.
      const String frame =
          '#0 AuthStore.token= (package:humanitl/core/auth/store.dart:41:12)';
      expect(redactForLog(frame), frame);
    });
  });

  group('Laufzeit', () {
    // Die Redaktion läuft synchron im Fehler-Handler, also im Fenster, das
    // gerade einen Fehler aufschreibt. Ein Ausdruck, der auf einer langen
    // Zeile zurückspringt, hält es an: gemessen am 2026-09-18 bis zu 60 s
    // für 100 KB.
    //
    // Gemessen wird auf 16 KiB und nicht auf der vollen Obergrenze: Dort
    // braucht die richtige Fassung unter Last bis zu 250 ms, und ein Test,
    // der auf einer belasteten Maschine rot wird, misst die Maschine. Auf
    // 16 KiB bleibt sie weit darunter, und ein quadratischer Ausdruck
    // braucht für dieselbe Eingabe immer noch Sekunden -- die Grenze trennt
    // beide weiterhin um mehr als das Zehnfache.
    const Duration budget = Duration(milliseconds: 250);

    // Einmal vorab, damit das Übersetzen der Ausdrücke nicht mitzählt.
    setUpAll(() => redactForLog('warm up https://example.com/x token=a'));

    Duration timed(String input) {
      final Stopwatch watch = Stopwatch()..start();
      redactForLog(input);
      watch.stop();
      return watch.elapsed;
    }

    const int size = 16 * 1024;

    test('16 KiB Hex', () {
      final String hex = List<String>.filled(size ~/ 8, 'a1b2c3d4').join();
      expect(timed(hex), lessThan(budget));
    }, timeout: const Timeout(Duration(minutes: 3)));

    test('16 KiB Zeichen eines Schemas, ohne ://', () {
      final String run = List<String>.filled(size ~/ 2, 'a.').join();
      expect(timed(run), lessThan(budget));
    }, timeout: const Timeout(Duration(minutes: 3)));

    test('16 KiB offener, maskierter Wert', () {
      final String open = 'password=\\"${'a' * size}';
      expect(timed(open), lessThan(budget));
    }, timeout: const Timeout(Duration(minutes: 3)));
  });

  group('die Eingabe ist begrenzt', () {
    test('mehr als maxRedactedInput wird vorher abgeschnitten', () {
      // Die zweite Grenze, für jedes künftige Muster: Kein Ausdruck läuft
      // auf einer unbegrenzten Eingabe.
      final String long = List<String>.filled(maxRedactedInput, 'ab').join(' ');
      expect(long.length, greaterThan(maxRedactedInput));
      final String out = redactForLog(long);
      expect(out.length, lessThanOrEqualTo(maxRedactedInput));
      expect(out, startsWith('ab ab ab'));
    });

    test('der Schnitt trennt nie die zwei Hälften eines Zeichens', () {
      // Ein Zeichen außerhalb der Grundebene sind in Dart zwei Einheiten. Liegt
      // die erste genau auf der letzten Stelle vor der Grenze, bliebe ein
      // einzelnes hohes Surrogat stehen -- kein Text mehr, sondern ein
      // kaputtes Zeichen.
      final String head =
          '${List<String>.filled((maxRedactedInput - 1) ~/ 2, 'a ').join()}a';
      expect(head.length, maxRedactedInput - 1);
      final String out = redactForLog('$head😀 und danach mehr');
      final int last = out.codeUnitAt(out.length - 1);
      expect(
        last >= 0xD800 && last <= 0xDBFF,
        isFalse,
        reason: 'letzte Einheit 0x${last.toRadixString(16)}',
      );
      expect(out.length, maxRedactedInput - 1);
    });
  });

  group('wie eine Ausnahme heißt', () {
    test('eine FormatException nennt Meldung und Versatz, nie die Quelle', () {
      expect(
        describeErrorForLog(
          const FormatException('Unexpected character', '{"body": 1}', 7),
        ),
        'FormatException: Unexpected character (offset 7)',
      );
    });

    test('jede andere Ausnahme bleibt ihr eigener Text', () {
      expect(
        describeErrorForLog(StateError('the view is gone')),
        'Bad state: the view is gone',
      );
    });
  });
}
