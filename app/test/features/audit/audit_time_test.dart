// Wie das Zeitraumfeld des Audit-Screens eine Grenze liest (HUM-051).
//
// Sie halten den Vertrag des Feldes fest: Ein Versatz wird umgerechnet, ohne
// Zone gilt UTC, ein Datum allein ist Mitternacht UTC, und was keine Zeit ist,
// bleibt ungelesen. Die Ergebnisse hängen nicht an der Zeitzone des Prozesses;
// sie sind in UTC und unter `TZ=Europe/Berlin` gelaufen.

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/audit/widgets/audit_table.dart';

void main() {
  test('an explicit offset is converted, never read as local fields', () {
    expect(
      auditParseUtc('2026-09-18T10:00:00+02:00'),
      DateTime.utc(2026, 9, 18, 8),
    );
    expect(
      auditParseUtc('2026-09-18T10:00:00-04:00'),
      DateTime.utc(2026, 9, 18, 14),
    );
    expect(
      auditParseUtc('2026-09-18 10:00 +0200'),
      DateTime.utc(2026, 9, 18, 8),
    );
  });

  test('Z and a missing zone both mean UTC', () {
    expect(
      auditParseUtc('2026-09-18T10:00:00Z'),
      DateTime.utc(2026, 9, 18, 10),
    );
    expect(auditParseUtc('2026-09-18 10:00'), DateTime.utc(2026, 9, 18, 10));
    expect(auditParseUtc('2026-09-18'), DateTime.utc(2026, 9, 18));
  });

  test('the day of a date is not mistaken for an offset', () {
    // `-11` am Ende von `2026-09-11` ist ein Tag, kein Versatz von elf
    // Stunden.
    expect(auditParseUtc('2026-09-11'), DateTime.utc(2026, 9, 11));
  });

  test('what is not a time stays unread', () {
    expect(auditParseUtc('11.09.2026 08:05'), isNull);
    expect(auditParseUtc(''), isNull);
    expect(auditParseUtc('yesterday'), isNull);
  });

  test('every result is UTC and round-trips through the field', () {
    for (final String text in <String>[
      '2026-09-18T10:00:00+02:00',
      '2026-09-18 10:00',
      '2026-09-18T23:30:00-04:00',
    ]) {
      final DateTime? at = auditParseUtc(text);
      expect(at?.isUtc, isTrue, reason: text);
      expect(auditParseUtc(auditFormatUtc(at!)), at, reason: text);
    }
  });
}
