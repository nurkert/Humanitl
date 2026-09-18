// Was in den Sprachdateien über dem Audit-Bildschirm steht, ohne einen Baum zu
// bauen. Dieselbe Begründung wie in `history_arb_test.dart`: Ein Wort, das nur
// in einer der beiden Dateien gepflegt wird, fällt sonst erst einem Leser auf.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> _arb(String name) =>
    jsonDecode(File('l10n/$name').readAsStringSync()) as Map<String, Object?>;

Set<String> _auditKeys(Map<String, Object?> arb) => arb.keys
    .where((String key) => key.startsWith('audit') && !key.startsWith('@'))
    .toSet();

void main() {
  final Map<String, Object?> en = _arb('app_en.arb');
  final Map<String, Object?> de = _arb('app_de.arb');

  test('both languages carry the same audit keys', () {
    expect(_auditKeys(de), _auditKeys(en));
    expect(_auditKeys(en), isNotEmpty);
  });

  test('the retention section says what the wireframe says', () {
    // Die Sätze wortgenau, nach der Skizze in HUM-051. 180 ist die Vorgabe von
    // `recorder.retention_days` und steht als Vorgabe da, neben dem
    // Schlüssel, der sie ändert: Der Client kann die geltende Zahl nicht lesen
    // (kein `GetConfig`, HUM-069). Die Konfiguration nimmt 0 bis 3650, und 0
    // heißt nie (`daemon/crates/config/src/validate.rs`).
    expect(
      en['auditRetentionRecordings'],
      'Recordings (requests, responses, bodies) are deleted after 180 days '
      'unless {key} sets another number; 0 means never.',
    );
    expect(
      de['auditRetentionRecordings'],
      'Aufzeichnungen (Anfragen, Antworten, Bodies) werden nach 180 Tagen '
      'gelöscht, sofern {key} keine andere Zahl nennt; 0 heißt nie.',
    );
    expect(
      en['auditRetentionChain'],
      'The audit chain is never deleted; this version does not act on {key}.',
    );
    expect(
      de['auditRetentionChain'],
      'Die Audit-Kette wird nie gelöscht; {key} wirkt in dieser Fassung nicht.',
    );
  });

  test('the status of a chain nobody has read is neither whole nor broken', () {
    for (final Map<String, Object?> arb in <Map<String, Object?>>[en, de]) {
      final String checking = arb['auditStatusChecking']! as String;
      final String unknown = arb['auditStatusUnknown']! as String;
      expect(checking, isNot(equals(arb['auditStatusOk'])));
      expect(unknown, isNot(equals(arb['auditStatusOk'])));
    }
    expect(en['auditStatusUnknown'], 'Not checked');
    expect(de['auditStatusUnknown'], 'Nicht geprüft');
  });

  test('the two warnings of the check say what stays unproven', () {
    expect(en['auditWarningNoHmacKey'], contains('unchecked'));
    expect(de['auditWarningNoHmacKey'], contains('ungeprüft'));
    for (final Map<String, Object?> arb in <Map<String, Object?>>[en, de]) {
      expect(arb['auditWarningUnanchoredTail'], contains('{count'));
    }
  });
}
