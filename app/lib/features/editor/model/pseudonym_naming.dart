/// Wie ein Pseudonym heißt (HUM-047).
///
/// Das Format ist genau `<TYPE_n>` mit ASCII-Spitzklammern, `n` beginnt bei 1,
/// und gezählt wird je Typ und Sitzung. Der Zweck ist nicht Schönheit, sondern
/// Lesbarkeit auf der Gegenseite: Wer `<EMAIL_1>` und `<EMAIL_2>` in einem
/// Prompt liest, sieht, dass dort zwei verschiedene Adressen standen, ohne eine
/// davon zu erfahren. Derselbe Wert bekommt deshalb immer dasselbe Pseudonym —
/// an jedem Ort der Anfrage, in jedem Feld, über die ganze Sitzung.
///
/// Ein Begriff des Nutzers mit hinterlegtem Alias (`findings.user_terms =
/// [{term, alias}]`, HUM-025) bekommt den Alias statt einer Nummer: Wer
/// „Müller GmbH" durch `Client-A` ersetzt, will genau das lesen, und ein
/// `<TERM_1>` nähme ihm die Information, die er selbst hinterlegt hat.
///
/// Die Zuordnung lebt hier nur im Speicher. Dauerhaft und verschlüsselt wird
/// sie in HUM-048; ab da holt der Editor die Namen über
/// `daemonClient.resolvePseudonyms` beim Daemon, und diese Klasse bleibt der
/// Rückfall für den Fake-Daemon (`backlog/sprint-4.md`, HUM-048).
library;

import '../../../core/domain/domain.dart';

/// Die Typ-Kürzel, die in einem Pseudonym stehen können.
///
/// Abgeleitet aus `FindingKind` des Kerns (`backlog/CONVENTIONS.md` 3.2). Der
/// Parameter eines Fundes (`api_key:github`, `user_term:Müller GmbH`) steht nie
/// im Pseudonym: Er ist selbst Information über den Wert.
const Map<String, String> pseudonymTypeLabels = <String, String>{
  'email': 'EMAIL',
  'iban': 'IBAN',
  'credit_card': 'CARD',
  'phone': 'PHONE',
  'ipv4': 'IPV4',
  'jwt': 'JWT',
  'api_key': 'API_KEY',
  'user_term': 'TERM',
  'custom': 'CUSTOM',
};

/// Das Kürzel für einen unbekannten Fund.
const String pseudonymFallbackLabel = 'CUSTOM';

/// Vergibt Pseudonyme und merkt sich, was schon vergeben wurde.
///
/// Die Klasse ist veränderlich, weil sie einen Zähler führt; sie ist aber
/// ohne Provider, ohne Flutter und ohne IO, also vollständig als Wert testbar.
/// Ein Aufrufer, der den Stand behalten will, liest [assigned] und [counters]
/// und baut damit die nächste Instanz.
class PseudonymNaming {
  /// Baut einen Namensgeber über einer bestehenden Zuordnung.
  ///
  /// [existing] bildet Wert-Hash (Hex, Kleinbuchstaben) auf das schon
  /// vergebene Pseudonym ab, [counters] hält den höchsten vergebenen Zähler je
  /// Kürzel, [aliases] die Aliasse aus `findings.user_terms`, nach Begriff.
  PseudonymNaming({
    Map<String, String> existing = const <String, String>{},
    Map<String, int> counters = const <String, int>{},
    Map<String, String> aliases = const <String, String>{},
  }) : _assigned = Map<String, String>.of(existing),
       _counters = Map<String, int>.of(counters),
       _aliases = Map<String, String>.of(aliases);

  final Map<String, String> _assigned;
  final Map<String, int> _counters;
  final Map<String, String> _aliases;

  /// Was bisher vergeben wurde: Wert-Hash auf Pseudonym.
  Map<String, String> get assigned =>
      Map<String, String>.unmodifiable(_assigned);

  /// Der höchste vergebene Zähler je Kürzel.
  Map<String, int> get counters => Map<String, int>.unmodifiable(_counters);

  /// Das Pseudonym für [valueHashHex], neu vergeben oder wiederverwendet.
  ///
  /// [kind] ist der `kind` eines Fundes, wie der Daemon ihn schreibt, also
  /// samt Parameter (`api_key:github`). Ein leerer [valueHashHex] bekommt
  /// trotzdem ein Pseudonym, aber keinen Eintrag in [assigned]: Ohne Hash gibt
  /// es keinen Wert, über den zwei Stellen dasselbe sagen könnten, und ein
  /// leerer Schlüssel fasste sie alle zusammen.
  String nameFor(String kind, String valueHashHex) {
    final String? known = _assigned[valueHashHex];
    if (valueHashHex.isNotEmpty && known != null) {
      return known;
    }
    final String alias = _aliasFor(kind);
    final String pseudonym = alias.isNotEmpty ? alias : _next(typeLabel(kind));
    if (valueHashHex.isNotEmpty) {
      _assigned[valueHashHex] = pseudonym;
    }
    return pseudonym;
  }

  /// Der Alias eines Nutzerbegriffs, oder ein leerer String.
  String _aliasFor(String kind) {
    if (!kind.startsWith('user_term:')) {
      return '';
    }
    return _aliases[kind.substring('user_term:'.length)] ?? '';
  }

  /// `<LABEL_n>` mit dem nächsten Zähler dieses Kürzels.
  String _next(String label) {
    final int n = (_counters[label] ?? 0) + 1;
    _counters[label] = n;
    return '<${label}_$n>';
  }

  /// Das Kürzel zu einem `kind` des Daemons.
  ///
  /// Der Parameter hinter dem Doppelpunkt fällt weg, und was das Register
  /// nicht kennt, wird [pseudonymFallbackLabel]: Ein Pseudonym, das den
  /// Bezeichner eines unbekannten Detektors trüge, verriete, wonach gesucht
  /// wurde, und lehrte niemanden etwas (`docs/UX.md` 4.2).
  ///
  /// **`custom:` ist die eine Ausnahme, und das ist der Sinn von `Ctrl+R`.**
  /// Dort hat kein Detektor etwas gefunden, sondern ein Mensch hat selbst
  /// gesagt, was die Stelle ist: `custom:PROJECT` wird `PROJECT`, nicht
  /// `CUSTOM`. Sein Wort steht im Pseudonym, sonst hätte das Tippen des
  /// Labels keine Wirkung.
  static String typeLabel(String kind) {
    final int colon = kind.indexOf(':');
    final String base = colon < 0 ? kind : kind.substring(0, colon);
    final String parameter = colon < 0 ? '' : kind.substring(colon + 1);
    if (base == 'custom' && parameter.isNotEmpty) {
      return labelFrom(parameter);
    }
    return pseudonymTypeLabels[base] ?? pseudonymFallbackLabel;
  }

  /// Das Kürzel eines freien Labels aus `Ctrl+R`.
  ///
  /// Ein Mensch tippt „project", „Kunde" oder „ACME GmbH"; daraus wird
  /// `PROJECT`, `KUNDE`, `ACME_GMBH`. Alles, was kein ASCII-Buchstabe und
  /// keine Ziffer ist, wird ein Unterstrich, und was danach leer bliebe, wird
  /// [pseudonymFallbackLabel] — ein Pseudonym `<_1>` sagte nichts.
  static String labelFrom(String text) {
    final StringBuffer out = StringBuffer();
    bool lastWasFill = true;
    for (final int unit in text.toUpperCase().codeUnits) {
      final bool keep =
          (unit >= 0x41 && unit <= 0x5A) || (unit >= 0x30 && unit <= 0x39);
      if (keep) {
        out.writeCharCode(unit);
        lastWasFill = false;
      } else if (!lastWasFill) {
        out.write('_');
        lastWasFill = true;
      }
    }
    final String label = out.toString().replaceAll(RegExp(r'_+$'), '');
    return label.isEmpty ? pseudonymFallbackLabel : label;
  }
}

/// Der Wert-Hash eines Fundes als Hex in Kleinbuchstaben.
///
/// Der eine Weg vom Byte-Hash zum Schlüssel der Zuordnung. Zwei Funde mit
/// demselben Wert tragen denselben Hash und bekommen deshalb dasselbe
/// Pseudonym, an welchem Ort der Anfrage sie auch stehen.
String valueHashHex(Finding finding) {
  final StringBuffer out = StringBuffer();
  for (final int byte in finding.valueHash) {
    out.write((byte & 0xFF).toRadixString(16).padLeft(2, '0'));
  }
  return out.toString();
}
