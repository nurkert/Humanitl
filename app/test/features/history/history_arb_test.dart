// Was in den Sprachdateien steht, ohne einen Baum zu bauen: die deutschen
// Wörter dieses Bildschirms, und dass beide Dateien dieselben Schlüssel
// tragen. Ein Wort, das nur in einer der beiden gepflegt wird, fällt sonst
// erst einem Leser auf.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> _arb(String name) =>
    jsonDecode(File('l10n/$name').readAsStringSync()) as Map<String, Object?>;

void main() {
  final Map<String, Object?> en = _arb('app_en.arb');
  final Map<String, Object?> de = _arb('app_de.arb');

  test('both languages carry the same history keys', () {
    Set<String> historyKeys(Map<String, Object?> arb) =>
        arb.keys.where((String key) => key.startsWith('history')).toSet();
    expect(historyKeys(de), historyKeys(en));
    expect(historyKeys(en), isNotEmpty);
  });

  test('the German of this screen says Funde, not Findings', () {
    // The code says "Fund"; a language file that says "Finding" in the same
    // column teaches two words for one thing.
    for (final String key in <String>[
      'historyChipFindings',
      'historyColumnFindings',
      'historyDetailFindings',
      'historyFindingsSemantics',
    ]) {
      final String value = de[key]! as String;
      expect(value, isNot(contains('Finding')), reason: key);
      expect(value.toLowerCase(), contains('fund'), reason: key);
    }
  });

  test('the filter hint uses a key that the recorder can answer', () {
    // `state:` compares against the seven states of the automaton, and
    // `blocked` is not one of them; an example that matches nothing teaches
    // the opposite of what it is for.
    for (final Map<String, Object?> arb in <Map<String, Object?>>[en, de]) {
      final String hint = arb['historyFilterHint']! as String;
      expect(hint, isNot(contains('state:blocked')));
      expect(hint, contains('decision:block'));
    }
  });

  test('the export names what the file carries, in both languages', () {
    expect(en['historyExportContents'], contains('clear text'));
    expect(de['historyExportContents'], contains('Klartext'));
    for (final Map<String, Object?> arb in <Map<String, Object?>>[en, de]) {
      final String sentence = arb['historyExportContents']! as String;
      // Headers and bodies are the two that carry secrets.
      expect(
        sentence.toLowerCase(),
        anyOf(contains('header'), contains('kopf')),
      );
      expect(
        sentence.toLowerCase(),
        anyOf(contains('bodies'), contains('rümpfe')),
      );
    }
  });

  test('no history test is defined twice', () {
    // Two identical blocks in one test file are an editing accident, not a
    // pair of tests: they run twice and prove the same thing once. On
    // 2026-09-04 one hundred and eighty-nine lines stood twice in
    // `history_screen_test.dart`.
    for (final FileSystemEntity entity in Directory(
      'test/features/history',
    ).listSync()) {
      if (!entity.path.endsWith('_test.dart')) {
        continue;
      }
      final List<String> names = <String>[];
      for (final String line in File(entity.path).readAsLinesSync()) {
        final RegExpMatch? match = RegExp(
          r"^\s*(?:test|testWidgets)\(\s*'([^']+)'",
        ).firstMatch(line);
        if (match != null) {
          names.add(match.group(1)!);
        }
      }
      expect(
        names.toSet(),
        hasLength(names.length),
        reason: '${entity.path} defines a test twice',
      );
    }
  });

  test('no history key is defined twice in a file', () {
    for (final String name in <String>['app_en.arb', 'app_de.arb']) {
      final List<String> raw = File('l10n/$name')
          .readAsLinesSync()
          .map((String line) => line.trim())
          .where((String line) => line.startsWith('"history'))
          .map((String line) => line.split('"')[1])
          .toList();
      expect(raw.toSet(), hasLength(raw.length), reason: name);
    }
  });
}
