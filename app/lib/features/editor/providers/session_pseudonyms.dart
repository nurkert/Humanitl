/// Die Pseudonyme dieser Sitzung, über alle Entwürfe hinweg (HUM-047).
///
/// Der Zähler gehört der Sitzung und nicht der einzelnen Anfrage. Stünde er am
/// [Draft], begänne jede gehaltene Anfrage wieder bei `<EMAIL_1>` — und
/// derselbe Name bezeichnete zuverlässig verschiedene Menschen. Wer zwei
/// Prompts nebeneinander liest, müsste dann raten, ob `<EMAIL_1>` zweimal
/// dieselbe Adresse meint. Die Spezifikation sagt es ausdrücklich: „Zähler pro
/// Typ und Session" (`backlog/sprint-4.md`, HUM-047).
///
/// # Was hier steht, und was nie
///
/// Gespeichert wird die Zuordnung von **Wert-Hash** auf Pseudonym und der
/// höchste vergebene Zähler je Kürzel. Ein Wert-Hash ist SHA-256 des Daemons
/// über den Fund; der Wert selbst steht nicht darin.
///
/// Eine Auswahl aus `Ctrl+R` hat keinen solchen Hash: Der Daemon hat sie nie
/// gesehen, und ihr Schlüssel im Entwurf (`manual:<LABEL>:<Text>`) **ist** der
/// Text. Solche Schlüssel nimmt [SessionPseudonyms.remember] deshalb nicht an.
/// Sie bleiben im Entwurf, der mit seinem Fluss verschwindet
/// (`DraftNotifier`, `Recorded`); ein Stand der Sitzung, der sie hielte, hielte
/// jedes markierte Geheimnis bis zum Ende der Anwendung im Klartext.
///
/// # Wann der Stand von vorn beginnt
///
/// Mit jeder neuen Sitzung. Einen Strom-Eintrag „Sitzung beendet" gibt es
/// nicht; die Flüsse tragen aber ihre `SessionId`, und sobald ein Entwurf eine
/// andere nennt als der Stand, beginnt der Stand leer. Zähler und Zuordnungen
/// wandern so nie von einer Sitzung in die nächste.
///
/// Dauerhaft und verschlüsselt wird die Zuordnung in HUM-048; ab da vergibt der
/// Daemon die Namen, und dieser Provider wird der Rückfall für den Fake-Daemon.
library;

import 'package:flutter/foundation.dart' show immutable, mapEquals;
import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../model/draft_ops.dart' show manualKeyPrefix;
import '../model/pseudonym_naming.dart';

part 'session_pseudonyms.g.dart';

/// Was die Sitzung über vergebene Pseudonyme weiß.
@immutable
class PseudonymLedger {
  /// Baut einen Stand.
  const PseudonymLedger({
    this.session,
    this.assigned = const <String, String>{},
    this.counters = const <String, int>{},
  });

  /// Die Sitzung, zu der dieser Stand gehört, oder null, solange keine
  /// bekannt ist.
  final SessionId? session;

  /// Wert-Hash auf Pseudonym. Nie ein Schlüssel mit [manualKeyPrefix].
  final Map<String, String> assigned;

  /// Der höchste vergebene Zähler je Kürzel.
  final Map<String, int> counters;

  @override
  bool operator ==(Object other) =>
      other is PseudonymLedger &&
      other.session == session &&
      mapEquals(other.assigned, assigned) &&
      mapEquals(other.counters, counters);

  @override
  int get hashCode => Object.hash(
    session,
    Object.hashAllUnordered(
      assigned.entries.map(
        (MapEntry<String, String> e) => Object.hash(e.key, e.value),
      ),
    ),
    Object.hashAllUnordered(
      counters.entries.map(
        (MapEntry<String, int> e) => Object.hash(e.key, e.value),
      ),
    ),
  );
}

/// Der Stand der Sitzung.
@Riverpod(keepAlive: true)
class SessionPseudonyms extends _$SessionPseudonyms {
  @override
  PseudonymLedger build() => const PseudonymLedger();

  /// Ein Namensgeber über dem Stand der Sitzung [session].
  ///
  /// Nennt [session] eine **neuere** Sitzung als der Stand, beginnt der Stand
  /// vorher leer. [local] ist die eigene Zuordnung des Entwurfs; sie trägt die
  /// Schlüssel aus `Ctrl+R`, die die Sitzung nicht hält, und gilt nur für
  /// diesen einen Namensgeber.
  ///
  /// Nennt [session] eine **ältere** Sitzung — ein Entwurf von vor einem
  /// Neustart des Daemons, der noch offen ist —, bleibt der Stand unberührt:
  /// Der Namensgeber kennt dann nur die eigene Zuordnung des Entwurfs und
  /// zählt von deren höchstem Namen weiter, und [remember] verwirft ihn.
  /// Setzte der alte Entwurf den Stand zurück, finge der nächste Entwurf der
  /// laufenden Sitzung wieder bei `<EMAIL_1>` an, und derselbe Name stünde in
  /// ihr für zwei verschiedene Werte.
  ///
  /// Der Aufrufer gibt ihn nach der Vergabe an [remember] zurück; erst dann
  /// gilt, was er vergeben hat. So bleibt [PseudonymNaming] eine reine,
  /// testbare Klasse ohne Provider darin.
  PseudonymNaming naming({
    required SessionId? session,
    Map<String, String> aliases = const <String, String>{},
    Map<String, String> local = const <String, String>{},
  }) {
    if (_isStale(session)) {
      return PseudonymNaming(
        existing: local,
        counters: countersOf(local.values),
        aliases: aliases,
      );
    }
    if (session != state.session) {
      state = PseudonymLedger(session: session);
    }
    return PseudonymNaming(
      existing: <String, String>{...state.assigned, ...local},
      counters: state.counters,
      aliases: aliases,
    );
  }

  /// Wahr, wenn [session] älter ist als die Sitzung des Stands.
  ///
  /// Eine `SessionId` ist eine UUIDv7 (`typed_id!` in
  /// `daemon/crates/core-types/src/ids.rs`, `Uuid::now_v7`): Ihre ersten 48 Bit
  /// sind die Startzeit, und in der kanonischen Hex-Schreibweise ordnet der
  /// Textvergleich deshalb nach der Zeit. Ein Entwurf ohne Sitzung gegen einen
  /// Stand mit Sitzung gilt als alt.
  bool _isStale(SessionId? session) {
    final SessionId? current = state.session;
    if (current == null || session == current) {
      return false;
    }
    return session == null || session.value.compareTo(current.value) < 0;
  }

  /// Übernimmt, was [naming] für [session] vergeben hat.
  ///
  /// Schlüssel einer Auswahl aus `Ctrl+R` ([manualKeyPrefix]) werden **nicht**
  /// übernommen: Sie sind der ausgewählte Text selbst. Der Zähler ihres Kürzels
  /// dagegen schon — er verrät nichts, und die nächste Auswahl soll
  /// weiterzählen.
  void remember(SessionId? session, PseudonymNaming naming) {
    if (session != state.session) {
      return;
    }
    state = PseudonymLedger(
      session: session,
      assigned: <String, String>{
        for (final MapEntry<String, String> entry in naming.assigned.entries)
          if (!entry.key.startsWith(manualKeyPrefix)) entry.key: entry.value,
      },
      counters: Map<String, int>.of(naming.counters),
    );
  }
}

/// Der höchste Zähler je Kürzel unter [names].
///
/// Liest Namen der Form `<KÜRZEL_N>`; Aliasse und alles andere zählen nicht.
/// Gebraucht für den Namensgeber eines alten Entwurfs, der die Zähler der
/// Sitzung nicht bekommt, aber keinen eigenen Namen doppelt vergeben darf.
Map<String, int> countersOf(Iterable<String> names) {
  final Map<String, int> counters = <String, int>{};
  for (final String name in names) {
    final RegExpMatch? match = _numbered.firstMatch(name);
    if (match == null) {
      continue;
    }
    final String label = match.group(1)!;
    final int number = int.parse(match.group(2)!);
    if (number > (counters[label] ?? 0)) {
      counters[label] = number;
    }
  }
  return counters;
}

final RegExp _numbered = RegExp(r'^<([A-Z][A-Z0-9_]*)_([0-9]+)>$');
