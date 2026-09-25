/// Was ein Mensch mit einem Entwurf tun kann, als reine Funktionen (HUM-047).
///
/// Jede Funktion nimmt einen [Draft] und gibt einen neuen zurück. Kein
/// Provider, kein Flutter, kein IO: Die ganze Fachlogik des Editors ist damit
/// ohne Widget-Baum prüfbar, und der Provider daneben ist nur noch eine
/// Ablage.
///
/// # Die eine Regel, an der alles hängt: Spans verschieben sich
///
/// Eine Ersetzung an `[s, e)` durch einen Text der Länge `n` verschiebt jede
/// spätere Stelle **am selben Ort** um `n - (e - s)`. Wer das vergisst,
/// ersetzt beim zweiten Fund ein paar Zeichen daneben — und beim dritten mitten
/// in einem Wort. Verschoben werden Funde **und** schon angewandte
/// Ersetzungen, denn der Diff-Glow steht auf denselben Offsets.
///
/// Ein Fund, der sich mit `[s, e)` überlappt, wird nie ignoriert (HUM-161).
/// Nach jeder Ersetzung ordnet `reanchorFindings` (`anchoring.dart`) die Funde
/// neu ihren Werten zu: Steht der Wert eines überlappten Fundes noch da, bleibt
/// er offen; ist er im Pseudonym aufgegangen, ist er
/// [FindingStatus.removed]. Eine Überlappung ist meist ein Regex-Artefakt
/// (eine IBAN, die auch als Telefonnummer durchgeht), aber nicht immer: Eine
/// Auswahl mit `Ctrl+R` kann einen Fund anschneiden, und dessen Wert ginge
/// sonst ungesehen hinaus.
library;

import 'dart:convert';

import '../../../core/domain/domain.dart';
import 'anchoring.dart';
import 'draft.dart';
import 'pseudonym_naming.dart';

export 'anchoring.dart' show hasPlace, isAnchored, reanchorFindings;

/// Das Präfix des Schlüssels einer Auswahl aus `Ctrl+R`.
///
/// Was dahinter steht, ist der ausgewählte Text selbst; siehe
/// [replaceSelection]. Ein Schlüssel mit diesem Präfix verlässt den Entwurf nie.
const String manualKeyPrefix = 'manual:';

/// Warum eine Kopfzeile nicht gesendet werden kann.
enum HeaderProblem {
  /// Der Name ist kein Token nach RFC 9110 5.1.
  name,

  /// Der Wert trägt ein Steuerzeichen, etwa einen Zeilenumbruch.
  value,

  /// Der Name gehört einer Kopfzeile, die Humanitl selbst setzt.
  ownedByDaemon,
}

/// Die erste freie Kopfzeile, die so nicht hinausgehen kann, oder null.
///
/// Geprüft wird hier, was der Daemon sonst still verwürfe: Einen Namen, der
/// kein Token ist, oder einen Wert mit Steuerzeichen lässt
/// `headers_from_proto` fallen, und eine Kopfzeile wie `Host` oder
/// `Connection` streicht `apply_edit` oder der Weg zum Ziel. Der Draht bleibt
/// dabei sauber — aber der Editor meldete Erfolg für eine Zeile, die nie
/// ankommt. Deshalb hält die Leiste das Senden an und nennt die Zeile.
///
/// Gesperrte Zeilen aus der Anfrage des Agenten werden nicht geprüft; sie
/// stehen so, wie sie kamen. Eine leere Zeile (Name und Wert leer) fällt beim
/// Senden weg und ist kein Fehler.
({int row, HeaderProblem problem})? checkHeaders(List<HeaderEntry> headers) {
  for (int i = 0; i < headers.length; i++) {
    final HeaderEntry entry = headers[i];
    if (entry.locked || (entry.name.isEmpty && entry.value.isEmpty)) {
      continue;
    }
    if (!_isToken(entry.name)) {
      return (row: i, problem: HeaderProblem.name);
    }
    if (isLockedHeader(entry.name)) {
      return (row: i, problem: HeaderProblem.ownedByDaemon);
    }
    if (!_isFieldValue(entry.value)) {
      return (row: i, problem: HeaderProblem.value);
    }
  }
  return null;
}

/// `token` nach RFC 9110 5.6.2: ein oder mehr `tchar`.
bool _isToken(String name) {
  if (name.isEmpty) {
    return false;
  }
  const String specials = "!#\$%&'*+-.^_`|~";
  for (final int unit in name.codeUnits) {
    final bool alpha =
        (unit >= 0x41 && unit <= 0x5A) || (unit >= 0x61 && unit <= 0x7A);
    final bool digit = unit >= 0x30 && unit <= 0x39;
    if (!alpha && !digit && !specials.codeUnits.contains(unit)) {
      return false;
    }
  }
  return true;
}

/// `field-value` nach RFC 9110 5.5: sichtbare Zeichen, Leerzeichen und
/// Tabulator; nichts unter 0x20 außer dem Tabulator, und kein DEL.
///
/// Alles ab 0x80 ist erlaubt: Es geht als UTF-8 hinaus, und jedes Byte davon
/// ist `obs-text`.
bool _isFieldValue(String value) {
  for (final int unit in value.codeUnits) {
    if ((unit < 0x20 && unit != 0x09) || unit == 0x7F) {
      return false;
    }
  }
  return true;
}

/// Wie viele Funde noch offen sind.
int openFindings(Draft draft) =>
    draft.findings.where((FindingView view) => view.isOpen).length;

/// Ersetzt genau einen Fund durch [pseudonym].
///
/// Ein Fund, den es nicht gibt oder der nicht mehr offen ist, lässt den
/// Entwurf unverändert: Ein zweiter Klick auf denselben Chip soll nichts
/// zerstören, und ein Aufruf mit einer alten Nummer ist kein Fehler des
/// Menschen. Dasselbe gilt für einen Fund, an dessen Stelle sein Wert nicht
/// mehr steht ([isAnchored]): Ersetzt würde fremder Text, der Fund hieße
/// ersetzt, und der Wert ginge ohne Pause hinaus (HUM-161). Er bleibt offen.
Draft replaceFinding(Draft draft, int findingIndex, String pseudonym) {
  final FindingView? view = draft.findings
      .where((FindingView view) => view.index == findingIndex && view.isOpen)
      .firstOrNull;
  if (view == null || !isAnchored(draft, view)) {
    return draft;
  }
  return _apply(
    draft,
    location: view.location,
    start: view.start,
    end: view.end,
    pseudonym: pseudonym,
    valueHash: view.valueHash,
    replacedIndex: view.index,
  );
}

/// Ersetzt jeden offenen Fund mit demselben Wert, an jedem Ort.
///
/// Der Wert selbst steht nirgends; verglichen wird sein Hash. Genau das macht
/// die Funktion möglich: Dieselbe Adresse in `authorization` und im Body ist
/// zweimal derselbe Wert, und beide bekommen dasselbe Pseudonym, ohne dass die
/// Oberfläche den Wert je gesehen hätte.
Draft replaceAllOfValue(Draft draft, String valueHash, String pseudonym) {
  if (valueHash.isEmpty) {
    return draft;
  }
  // Durchgänge bis nichts mehr ersetzt wird; siehe [replaceAllOpen].
  Draft out = draft;
  for (int pass = 0; pass <= _passBound(draft); pass++) {
    final Draft next = _replaceValueOnce(out, valueHash, pseudonym);
    if (identical(next, out)) {
      break;
    }
    out = next;
  }
  return out;
}

/// Ein Durchgang von [replaceAllOfValue]: jeder passende offene Fund
/// höchstens einmal; einer ohne Stelle bliebe sonst ewig gewählt.
Draft _replaceValueOnce(Draft draft, String valueHash, String pseudonym) {
  Draft out = draft;
  for (final FindingView view in draft.findings) {
    if (view.valueHash == valueHash) {
      out = replaceFinding(out, view.index, pseudonym);
    }
  }
  return out;
}

/// Die Sicherung für die Durchgänge beim Ersetzen.
///
/// Jeder Durchgang, der etwas ändert, nimmt eine Kopie eines Wertes weg, und
/// es kann nicht mehr Kopien geben als Zeichen in dem, was hinausginge. Die
/// Schleifen enden deshalb lange vorher; die Grenze fängt nur einen Fehler ab.
int _passBound(Draft draft) =>
    draft.body.length +
    draft.pathAndQuery.length +
    draft.headers.fold<int>(
      0,
      (int sum, HeaderEntry entry) =>
          sum + entry.name.length + entry.value.length,
    ) +
    1;

/// Ersetzt jeden offenen Fund; die Namen kommen aus [naming].
///
/// Die Reihenfolge ist die des `Analyzed`-Ereignisses und nicht die der
/// Textstellen: Die Zähler sollen von oben nach unten laufen, wie die
/// Findings-Leiste sie zeigt. Gleicher Wert heißt gleiches Pseudonym, weil
/// [PseudonymNaming] sich jeden vergebenen Namen merkt.
Draft replaceAllOpen(Draft draft, PseudonymNaming naming) {
  final List<int> order = <int>[
    for (final FindingView view in draft.findings) view.index,
  ]..sort();
  // Durchgänge, bis nichts mehr ersetzt wird: Eine Ersetzung kann einen Fund
  // wieder öffnen, dessen Wert ein weiteres Mal im Text steht
  // (`reanchorFindings`, Regel 4), und ein Fund mit fünf Kopien braucht fünf
  // Durchgänge. Kopien in stehenden Pseudonymen zählen nicht, also nimmt jede
  // Ersetzung eine Kopie weg; [_passBound] ist nur die Sicherung.
  Draft out = draft;
  for (int pass = 0; pass <= _passBound(draft); pass++) {
    final Draft next = _replaceAllOnce(out, order, naming);
    if (identical(next, out)) {
      break;
    }
    out = next;
  }
  return out.copyWith(counters: naming.counters);
}

/// Ein Durchgang von [replaceAllOpen]; gibt [draft] selbst zurück, wenn er
/// nichts ersetzt hat.
Draft _replaceAllOnce(Draft draft, List<int> order, PseudonymNaming naming) {
  Draft out = draft;
  for (final int index in order) {
    // Die eine Stelle, an der „offen" hier zählt. Ein ignorierter Fund darf
    // nicht einmal einen Namen bekommen: [PseudonymNaming] zählt je Typ, und
    // ein verbrannter Zähler ließe den nächsten echten Fund `<EMAIL_2>`
    // heißen, obwohl es keinen `<EMAIL_1>` gibt. Dass [replaceFinding] ihn
    // danach ohnehin stehen ließe, reicht deshalb nicht.
    final FindingView? view = out.findings
        .where((FindingView view) => view.index == index && view.isOpen)
        .firstOrNull;
    // Ein Fund ohne seinen Wert an der Stelle wird nicht ersetzt und bekommt
    // deshalb auch keinen Namen, aus demselben Grund.
    if (view == null || !isAnchored(out, view)) {
      continue;
    }
    out = replaceFinding(out, index, naming.nameFor(view.kind, view.valueHash));
  }
  return out;
}

/// Lässt einen Fund stehen; er zählt danach nicht mehr als offen.
Draft ignoreFinding(Draft draft, int findingIndex) => draft.copyWith(
  findings: <FindingView>[
    for (final FindingView view in draft.findings)
      if (view.index == findingIndex && view.isOpen)
        view.copyWith(status: FindingStatus.ignored)
      else
        view,
  ],
);

/// Pseudonymisiert eine Auswahl, die kein Detektor gefunden hat (`Ctrl+R`).
///
/// Es entsteht ein synthetischer Fund mit `kind: custom:<LABEL>` und Status
/// [FindingStatus.replaced], damit die Stelle im Mapping, in der Leiste und im
/// Diff-Glow auftaucht wie jede andere. Seine Nummer liegt hinter allen echten
/// Funden, damit `acknowledged_findings` (HUM-049) die Indizes des
/// `Analyzed`-Ereignisses behält.
///
/// Der Schlüssel der Zuordnung ist **kein** SHA-256: Der Wert ist nur hier
/// bekannt, der Daemon hat ihn nie gesehen, und die Oberfläche rechnet keine
/// Hashes. Er trägt deshalb das Präfix [manualKeyPrefix] und kann mit keinem
/// Hash des Daemons kollidieren, der aus 64 Hex-Zeichen besteht.
///
/// **Der Schlüssel enthält den ausgewählten Text im Klartext.** Er bleibt
/// deshalb im Entwurf, der mit seinem Fluss verschwindet, und geht nie in den
/// Stand der Sitzung (`SessionPseudonyms.remember` verwirft ihn). Dieselbe
/// Auswahl bekommt so innerhalb einer Anfrage denselben Namen, über Anfragen
/// hinweg nicht. HUM-048 löst das endgültig: Dort vergibt der Daemon die Namen,
/// und er hasht selbst.
Draft replaceSelection(
  Draft draft,
  DraftLocation location,
  int start,
  int end,
  String kindLabel,
  PseudonymNaming naming,
) {
  final String text = draft.textAt(location);
  if (start < 0 || end > text.length || start >= end) {
    return draft;
  }
  final String label = PseudonymNaming.labelFrom(kindLabel);
  final String original = text.substring(start, end);
  final String key = '$manualKeyPrefix$label:$original';
  final String pseudonym = naming.nameFor('custom:$label', key);
  final int index = _nextIndex(draft);
  final Draft seeded = draft.copyWith(
    findings: <FindingView>[
      ...draft.findings,
      FindingView(
        index: index,
        finding: Finding(
          kind: 'custom:$label',
          location: location.kind,
          headerName: location.headerName,
          spanStart: start,
          spanEnd: end,
          tier: FindingTier.userTerm,
          displayPrefix: '',
        ),
        location: location,
        start: start,
        end: end,
        valueHash: key,
        // Der ausgewählte Text ist der Anker: Nach `Ctrl+Z` steht er wieder
        // da, und der Fund wird wieder offen, statt als ersetzt ohne Pause
        // hinauszugehen (HUM-161). Er bleibt wie der Schlüssel im Entwurf.
        value: original,
      ),
    ],
  );
  return _apply(
    seeded,
    location: location,
    start: start,
    end: end,
    pseudonym: pseudonym,
    valueHash: key,
    replacedIndex: index,
  ).copyWith(counters: naming.counters);
}

/// Der Body, wie er hinausginge, und ob er noch gültiges JSON ist.
///
/// Die Prüfung hält nichts auf. Ein Mensch darf einen Body schicken, der kein
/// JSON ist — vielleicht war er vorher schon keins. Sie steht da, weil eine
/// Ersetzung mitten in einem Anführungszeichen ein Versehen ist, das man
/// bemerken will, bevor der Server mit `400` antwortet (`docs/UX.md` 4.4).
({String body, String? jsonError}) renderBody(Draft draft) {
  if (draft.bodyKind != BodyKind.json || draft.body.trim().isEmpty) {
    return (body: draft.body, jsonError: null);
  }
  try {
    jsonDecode(draft.body);
    return (body: draft.body, jsonError: null);
  } on FormatException catch (error) {
    return (body: draft.body, jsonError: error.message);
  }
}

/// Die nächste freie Fund-Nummer.
int _nextIndex(Draft draft) => draft.findings.fold<int>(
  0,
  (int highest, FindingView view) =>
      view.index >= highest ? view.index + 1 : highest,
);

/// Setzt [pseudonym] an `[start, end)` und zieht alles Spätere nach.
Draft _apply(
  Draft draft, {
  required DraftLocation location,
  required int start,
  required int end,
  required String pseudonym,
  required String valueHash,
  required int replacedIndex,
}) {
  final String text = draft.textAt(location);
  if (start < 0 || end > text.length || start > end) {
    return draft;
  }
  final String original = text.substring(start, end);
  final int delta = pseudonym.length - (end - start);
  final String next =
      text.substring(0, start) + pseudonym + text.substring(end);

  final List<FindingView> findings = <FindingView>[
    for (final FindingView view in draft.findings)
      if (view.index == replacedIndex)
        view.copyWith(
          status: FindingStatus.replaced,
          pseudonym: pseudonym,
          start: start,
          end: start + pseudonym.length,
        )
      else if (view.location != location)
        view
      else if (view.isOpen &&
          !view.unplaced &&
          view.value.isEmpty &&
          view.start < end &&
          view.end > start)
        // Überlappt und ohne Anker: Wo er stand, steht jetzt das Pseudonym.
        // Ein Fund mit Anker bleibt hier stehen; `reanchorFindings` unten
        // entscheidet über ihn nach seinem Wert. Ein Fund ohne Stelle
        // ([FindingView.unplaced]) trägt noch den Bereich des Daemons, der
        // nichts über den Text sagt; er bleibt offen (HUM-161).
        view.copyWith(status: FindingStatus.removed)
      else if (view.start >= end)
        view.copyWith(start: view.start + delta, end: view.end + delta)
      else
        view,
  ];

  final List<Replacement> replacements = <Replacement>[
    for (final Replacement done in draft.replacements)
      if (done.location == location && done.start >= end)
        done.copyWith(start: done.start + delta, end: done.end + delta)
      else
        done,
    Replacement(
      location: location,
      start: start,
      end: start + pseudonym.length,
      original: original,
      pseudonym: pseudonym,
      valueHash: valueHash,
    ),
  ];

  return reanchorFindings(
    draft
        .withTextAt(location, next)
        .copyWith(
          findings: findings,
          replacements: replacements,
          pseudonyms: <String, String>{
            ...draft.pseudonyms,
            if (valueHash.isNotEmpty) valueHash: pseudonym,
          },
          dirty: true,
        ),
  );
}
