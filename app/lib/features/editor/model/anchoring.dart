/// Wo die Funde eines Entwurfs nach einer Änderung stehen (HUM-161).
///
/// Ein Fund des Daemons steht auf Offsets im Text seines Ortes. Jede Eingabe
/// und jede Ersetzung verschiebt Text, und ein Fund auf veralteten Offsets
/// ersetzte beim nächsten „Alle ersetzen" fremden Text, hieße danach ersetzt,
/// und der Wert ginge ohne Pause hinaus. Deshalb trägt jeder Fund seinen Wert
/// als Anker ([FindingView.value]), und nach jeder Änderung ordnet
/// [reanchorFindings] die Funde den Stellen zu, an denen ihr Wert jetzt steht.
///
/// # Die Regeln
///
/// 1. **Eins zu eins.** Eine Stelle gehört höchstens einem Fund. Zwei Funde
///    mit demselben Wert landen nie auf derselben Kopie; sonst ersetzte
///    „Alle ersetzen" die eine zweimal und ließe die andere stehen.
/// 2. **Wer steht, bleibt.** Ein offener Fund, an dessen Stelle sein Wert
///    noch steht, behält sie. Die übrigen offenen Funde bekommen die freien
///    Kopien an ihrem Ort, beide Listen nach Position geordnet.
/// 3. **Offen, solange der Wert hinausgeht.** Findet ein offener Fund an
///    seinem Ort keine freie Kopie, steht der Wert aber noch irgendwo in dem,
///    was hinausginge (Rumpf, jede Kopfzeile, Pfad, Query), bleibt er offen,
///    ohne Stelle: Er zählt für Knopf und Pause und wird nie ersetzt.
///    [FindingStatus.removed] ist er nur, wenn der Wert nirgends mehr steht.
/// 4. **Zurück zu offen.** Ein ersetzter oder entfernter Fund wird wieder
///    offen, sobald sein Wert an einer Stelle steht, die kein offener Fund
///    hält: nach dem Zurücktippen oder nach `Ctrl+Z` hinter „Alle ersetzen".
///    Die Ersetzung, deren Pseudonym dann nicht mehr dasteht, fällt weg.
///    Eine eigene Auswahl aus `Ctrl+R` (`custom:`) gilt dabei nur an ihrem
///    Ort: Sie ist eine Entscheidung über genau diese Stelle, oft ein kurzes
///    Wort, und an jedem anderen Ort, an dem es zufällig auch steht, wäre sie
///    weder gemeint noch ersetzbar.
/// 5. **Nie ignoriert.** Nur eine Entscheidung des Menschen
///    (`ignoreFinding`) macht einen Fund [FindingStatus.ignored].
library;

import 'dart:convert';

import '../../../core/domain/domain.dart';
import 'draft.dart';

/// Wahr, wenn an der Stelle von [view] noch genau sein Wert steht.
///
/// Ein Fund ohne [FindingView.value] (eine Auswahl aus `Ctrl+R`) hat keinen
/// Anker und gilt als verankert. Ein Fund ohne Stelle
/// ([FindingView.unplaced], oder ein negativer Anfang nach Regel 3) ist es nie.
bool isAnchored(Draft draft, FindingView view) {
  if (view.unplaced || view.start < 0) {
    return false;
  }
  if (view.value.isEmpty) {
    return true;
  }
  final String text = draft.textAt(view.location);
  return view.end <= text.length &&
      view.start <= view.end &&
      text.substring(view.start, view.end) == view.value;
}

/// Wahr, wenn [view] eine Stelle im Text hat, die sich markieren lässt.
bool hasPlace(FindingView view) =>
    !view.unplaced && view.start >= 0 && view.end > view.start;

/// Ordnet jeden verfolgten Fund der Stelle zu, an der sein Wert jetzt steht.
///
/// Die Regeln stehen im Kopf dieser Datei.
Draft reanchorFindings(Draft draft) {
  final _Occurrences where = _Occurrences(draft);
  final List<FindingView> views = List<FindingView>.of(draft.findings);
  final List<int> pending = <int>[];
  for (int i = 0; i < views.length; i++) {
    final FindingView view = views[i];
    if (!_tracked(view) || !view.isOpen) {
      continue;
    }
    if (!(isAnchored(draft, view) &&
        where.claim(where.keyOf(view.location), view.start))) {
      pending.add(i);
    }
  }
  pending.sort((int a, int b) => views[a].start.compareTo(views[b].start));
  for (final int i in pending) {
    views[i] = _placeOpen(views[i], where);
  }
  final Set<int> reopened = _reopenAll(views, where);
  _settleUnplaced(views, where);
  return draft.copyWith(
    findings: views,
    replacements: reopened.isEmpty
        ? draft.replacements
        : _withoutStale(draft, <FindingView>[
            for (final int i in reopened) draft.findings[i],
          ]),
  );
}

/// Regel 4 über alle Funde; gibt die Stellen der wieder geöffneten zurück.
Set<int> _reopenAll(List<FindingView> views, _Occurrences where) {
  final Set<int> reopened = <int>{};
  for (int i = 0; i < views.length; i++) {
    final FindingView view = views[i];
    if (_tracked(view) &&
        (view.status == FindingStatus.replaced ||
            view.status == FindingStatus.removed)) {
      final FindingView? again = _reopen(view, where);
      if (again != null) {
        views[i] = again;
        reopened.add(i);
      }
    }
  }
  return reopened;
}

/// Ein Fund ohne Stelle, aber mit Wert oder möglichen Werten
/// ([FindingView.alternatives]), folgt nur der Frage, ob einer davon noch
/// hinausginge (HUM-161).
///
/// Ohne das bliebe er offen, auch wenn der Mensch den Wert gelöscht hat, und
/// unter der harten Sperre käme „Trotzdem senden" nie zurück. Steht der Wert
/// wieder da, ist er wieder offen. Ein Fund ohne Wert lässt sich nicht
/// prüfen und bleibt offen.
void _settleUnplaced(List<FindingView> views, _Occurrences where) {
  for (int i = 0; i < views.length; i++) {
    final FindingView view = views[i];
    final List<String> values = view.value.isNotEmpty
        ? <String>[view.value]
        : view.alternatives;
    if (!view.unplaced ||
        values.isEmpty ||
        !(view.isOpen || view.status == FindingStatus.removed)) {
      continue;
    }
    views[i] = view.copyWith(
      status: values.any(where.anywhere)
          ? FindingStatus.open
          : FindingStatus.removed,
    );
  }
}

/// Wahr für einen Fund, den der Anker verfolgt: einer mit Wert und Stelle,
/// den kein Mensch ignoriert hat.
bool _tracked(FindingView view) =>
    !view.unplaced &&
    view.value.isNotEmpty &&
    view.status != FindingStatus.ignored;

/// Regeln 2 und 3 für einen offenen Fund, der seine Stelle verloren hat.
FindingView _placeOpen(FindingView view, _Occurrences where) {
  final String own = where.keyOf(view.location);
  final int? at = where.claimFirst(own, view.value);
  if (at != null) {
    return view.copyWith(start: at, end: at + view.value.length);
  }
  if (_isManual(view)) {
    return view.copyWith(status: FindingStatus.removed);
  }
  if (where.claimElsewhere(view.value) || where.anywhere(view.value)) {
    return view.copyWith(start: -1, end: -1);
  }
  return view.copyWith(status: FindingStatus.removed);
}

/// Regel 4: ein ersetzter oder entfernter Fund, dessen Wert wieder frei
/// dasteht, oder null.
FindingView? _reopen(FindingView view, _Occurrences where) {
  final int? at = where.claimFirst(where.keyOf(view.location), view.value);
  if (at != null) {
    return view.copyWith(
      status: FindingStatus.open,
      pseudonym: '',
      start: at,
      end: at + view.value.length,
    );
  }
  if (_isManual(view)) {
    return null;
  }
  // Anderswo, wörtlich oder nur in Pfad und Query ausgeschrieben: offen ohne
  // Stelle. Die dekodierte Form allein genügt, denn eine wörtliche Kopie hat
  // `claimElsewhere` schon vergeben.
  if (where.claimElsewhere(view.value) || where.decodedAnywhere(view.value)) {
    return view.copyWith(
      status: FindingStatus.open,
      pseudonym: '',
      start: -1,
      end: -1,
    );
  }
  return null;
}

/// Wahr für eine eigene Auswahl aus `Ctrl+R`; sie gilt nur an ihrem Ort.
bool _isManual(FindingView view) => view.kind.startsWith('custom:');

/// Die Ersetzungen ohne die der wieder geöffneten Funde, deren Pseudonym
/// nicht mehr an seiner Stelle steht.
List<Replacement> _withoutStale(Draft draft, List<FindingView> reopened) =>
    <Replacement>[
      for (final Replacement done in draft.replacements)
        if (!_isStaleOf(draft, done, reopened)) done,
    ];

bool _isStaleOf(Draft draft, Replacement done, List<FindingView> reopened) {
  final String text = draft.textAt(done.location);
  final bool stands =
      done.start >= 0 &&
      done.end <= text.length &&
      text.substring(done.start, done.end) == done.pseudonym;
  return !stands &&
      reopened.any(
        (FindingView view) =>
            view.pseudonym == done.pseudonym &&
            view.valueHash == done.valueHash,
      );
}

/// Jede Stelle, an der ein Wert in dem steht, was hinausginge, und welche
/// davon schon einem Fund gehören.
class _Occurrences {
  _Occurrences(this._draft)
    : _texts = <String, String>{
        _body: _draft.body,
        _query: _draft.query,
        _path: _draft.path,
        for (int i = 0; i < _draft.headers.length; i++)
          '$_header$i': _draft.headers[i].value,
        // Name und Methode gehen ebenso hinaus; ein Wert, der dorthin
        // gewandert ist, ist nicht weg.
        for (int i = 0; i < _draft.headers.length; i++)
          '$_headerName$i': _draft.headers[i].name,
        _method: _draft.method,
      };

  static const String _body = 'body';
  static const String _query = 'query';
  static const String _path = 'path';
  static const String _header = 'header:';
  static const String _headerName = 'header-name:';
  static const String _method = 'method';

  final Draft _draft;

  /// Jeder Text, der hinausginge, unter seinem Schlüssel.
  final Map<String, String> _texts;

  /// Die vergebenen Stellen als `schlüssel@anfang`.
  final Set<String> _claimed = <String>{};

  /// Der Schlüssel des Textes, den [location] meint.
  String keyOf(DraftLocation location) => switch (location.kind) {
    FindingLocation.body => _body,
    FindingLocation.query => _query,
    FindingLocation.header => '$_header${location.indexIn(_draft.headers)}',
  };

  /// Vergibt die Stelle [start] in [key]; falsch, wenn sie schon vergeben ist.
  bool claim(String key, int start) => _claimed.add('$key@$start');

  /// Vergibt die erste freie Kopie von [value] in [key], nach Position.
  ///
  /// Eine Kopie mitten in einem Pseudonym, das noch dasteht, ist keine: Ein
  /// Wert wie `1` stünde sonst in `<EMAIL_1>` und öffnete den Fund wieder.
  int? claimFirst(String key, String value) {
    for (final int at in _positions(_texts[key] ?? '', value)) {
      if (!_insidePseudonym(key, at, at + value.length) && claim(key, at)) {
        return at;
      }
    }
    return null;
  }

  bool _insidePseudonym(String key, int start, int end) =>
      _draft.replacements.any((Replacement done) {
        if (keyOf(done.location) != key) {
          return false;
        }
        final String text = _texts[key] ?? '';
        return done.start >= 0 &&
            done.end <= text.length &&
            text.substring(done.start, done.end) == done.pseudonym &&
            start < done.end &&
            end > done.start;
      });

  /// Vergibt eine freie Kopie von [value] in irgendeinem Text.
  bool claimElsewhere(String value) =>
      _texts.keys.any((String key) => claimFirst(key, value) != null);

  /// Wahr, wenn [value] irgendwo steht, vergeben oder nicht, außerhalb eines
  /// stehenden Pseudonyms.
  ///
  /// In Pfad und Query zählt auch die dekodierte Form: Ein Wert, den der
  /// Daemon als `a%40x.de` fand und den jemand zu `a@x.de` ausgeschrieben hat,
  /// geht weiter hinaus, nur anders geschrieben (HUM-161).
  bool anywhere(String value) =>
      _texts.keys.any(
        (String key) => _positions(
          _texts[key] ?? '',
          value,
        ).any((int at) => !_insidePseudonym(key, at, at + value.length)),
      ) ||
      decodedAnywhere(value);

  /// Wahr, wenn [value] in Pfad oder Query steht, beide dekodiert verglichen.
  bool decodedAnywhere(String value) {
    final String needle = _decoded(value);
    if (needle.isEmpty) {
      return false;
    }
    return <String>[
      _path,
      _query,
    ].any((String key) => _decoded(_texts[key] ?? '').contains(needle));
  }

  /// [text] mit jedem gültigen `%XX` als Byte und `+` als Leerzeichen, als
  /// UTF-8 gelesen.
  ///
  /// Nachsichtig, anders als `Uri.decodeQueryComponent`: Ein einzelnes `%`
  /// oder ein Umlaut neben einem `%XX` ließen jene werfen, und ein Text, der
  /// sich nicht lesen ließe, verlöre jeden Treffer darin. Ein `%` ohne zwei
  /// Hexziffern bleibt, wie es ist.
  static String _decoded(String text) {
    final List<int> bytes = <int>[];
    for (final Match token in _escapes.allMatches(text.replaceAll('+', ' '))) {
      final String part = token[0]!;
      bytes.addAll(
        part.length == 3 && part.startsWith('%')
            ? <int>[int.parse(part.substring(1), radix: 16)]
            : utf8.encode(part),
      );
    }
    return utf8.decode(bytes, allowMalformed: true);
  }

  /// Ein gültiges `%XX`, ein Stück ohne `%` oder ein einzelnes `%`.
  static final RegExp _escapes = RegExp('%[0-9A-Fa-f]{2}|[^%]+|%');

  static Iterable<int> _positions(String text, String value) sync* {
    for (
      int at = text.indexOf(value);
      at >= 0;
      at = text.indexOf(value, at + 1)
    ) {
      yield at;
    }
  }
}
