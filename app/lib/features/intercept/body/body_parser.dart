/// Aus Bytes wird ein Modell, aus dem eine Ansicht zeichnen kann.
///
/// Alles hier ist einfaches Dart ohne Flutter-Typ: über [bodyIsolateThreshold]
/// läuft das Zerlegen in `Isolate.run`, und nur einfache Werte kommen zurück
/// (`docs/UX.md` 7). Alles hier rechnet außerdem mit einem feindlichen Rumpf.
/// Drei Grenzen halten ihn davon ab, die Oberfläche anzuhalten:
///
/// * [jsonMaxDepth] — unterhalb dieser Tiefe wird nicht mehr aufgebaut,
///   sondern ein Knoten gesetzt, der sagt, dass es weitergeht.
/// * [jsonMaxNodes] — ein Baum aus einer Million Knoten wird nicht vollständig
///   gebaut, sondern beschnitten, und die Ansicht sagt es.
/// * [bodyMaxRows] und [bodyRowChars] — eine einzige Zeile von acht Mebibyte
///   wird in Zeilen bekannter Länge zerlegt, statt in einem `Text` zu landen,
///   dessen Layout Sekunden kostet. Die Zerlegung verschiebt keinen Versatz:
///   jede Zeile trägt ihren eigenen Anfang.
///
/// Und mit einer Unterscheidung, die dieses Modell überall durchhält: **leer
/// und nicht lesbar sind zwei verschiedene Aussagen.** [BodyKind.empty] gibt
/// es nur bei Größe null; alles andere, was nicht zerfällt, kommt mit einem
/// [BodyProblem] zurück und nie als leeres Ergebnis.
library;

import 'dart:convert';
import 'dart:isolate';
import 'dart:typed_data';

import '../../../core/domain/domain.dart';
import 'body_kind.dart';
import 'body_span.dart';

/// Ab dieser Größe wird auf einem anderen Isolate zerlegt.
const int bodyIsolateThreshold = 64 * 1024;

/// Wie viele Zeichen eine Zeile der Rohansicht höchstens trägt.
///
/// Kein Umbruch im Sinne des Fallstricks von HUM-030: der Schnitt fällt an
/// einer bekannten Spalte, und jede Zeile trägt ihren Zeichenversatz, also
/// bleibt jede Markierung dort, wo sie hingehört.
const int bodyRowChars = 2000;

/// Wie viele Zeilen die Rohansicht höchstens führt.
const int bodyMaxRows = 200000;

/// Wie tief der JSON-Baum aufgebaut wird.
const int jsonMaxDepth = 64;

/// Wie viele Knoten der JSON-Baum höchstens trägt.
const int jsonMaxNodes = 200000;

/// Wie lang ein Wert im Baum angezeigt wird, bevor gekürzt wird.
const int jsonValueChars = 200;

/// Wie lang ein Schlüssel im Baum angezeigt wird, bevor gekürzt wird.
///
/// Beide Deckel stehen im Modell und nicht in der Ansicht: die Ansicht
/// zeichnet, was hier steht, und die Fundsuche rechnet gegen dieselbe Grenze.
/// Auseinander gelaufen hieße: ein Unterstrich unter etwas, das gar nicht
/// gezeigt wird.
const int jsonKeyChars = 200;

/// Wie viel Arbeit die Fundsuche im Baum höchstens kostet: Knoten mal Funde.
const int jsonFindingBudget = 4000000;

/// Warum eine Ansicht weniger zeigt, als der Rumpf hergibt.
enum BodyProblem {
  /// Der Rumpf wurde als JSON angekündigt und ist keins.
  notJson,

  /// Der Rumpf wurde als Formular angekündigt und ist keins.
  notForm,

  /// Der Rumpf ist größer als [bodyMaxBytes]; nichts davon wurde zerlegt.
  tooLarge,

  /// Der Daemon hat weniger Bytes geliefert, als der Verweis nennt.
  incomplete,

  /// Der gepackte Strom endet nicht dort, wo sein Abschluss es sagt.
  ///
  /// Nicht dasselbe wie [incomplete]: dort brach der Transport ab, hier ist
  /// der Inhalt selbst abgeschnitten oder mehrgliedrig, und der Daemon hat
  /// dann etwas anderes gesehen als diese Ansicht.
  truncatedStream,

  /// Die Anfrage kündigt eine Kodierung an, die diese Ansicht nicht auspackt.
  undecodedEncoding,
}

/// Was für ein Wert an einem Knoten des Baums steht.
enum JsonNodeType {
  /// Ein Objekt.
  object,

  /// Ein Feld.
  array,

  /// Eine Zeichenkette.
  string,

  /// Eine Zahl.
  number,

  /// Wahr oder falsch.
  boolean,

  /// `null`.
  nul,

  /// Hier ginge es weiter, aber [jsonMaxDepth] ist erreicht.
  elided,
}

/// Wo ein Fund an einem Knoten steht.
///
/// Der Baum unterstreicht nicht den ganzen Wert, sondern genau die Stelle, und
/// er unterscheidet Schlüssel von Wert: ein Fund im Schlüssel gehört an den
/// Schlüssel, nicht an den Wert daneben.
class JsonMark {
  /// Creates a mark.
  const JsonMark({
    required this.finding,
    required this.inKey,
    required this.start,
    required this.end,
  });

  /// Die Stelle in `FlowDetail.findings`.
  final int finding;

  /// Wahr, wenn der Treffer im Schlüssel steht.
  final bool inKey;

  /// Erste Code-Unit des Treffers im gezeichneten Text.
  final int start;

  /// Erste Code-Unit dahinter.
  final int end;
}

/// Ein Knoten des JSON-Baums.
///
/// Ein flaches Feld statt eines Zeigergeflechts: das Modell reist damit in
/// einem Stück durch `Isolate.run`, und die Ansicht kommt ohne Rekursion aus.
class JsonNode {
  /// Creates a node.
  JsonNode({
    required this.parent,
    required this.depth,
    required this.key,
    required this.index,
    required this.type,
    required this.display,
    required this.full,
    required this.childCount,
  });

  /// Der Elternknoten, oder -1 an der Wurzel.
  final int parent;

  /// Wie tief der Knoten steht; die Wurzel steht auf null.
  final int depth;

  /// Der Schlüssel, unter dem der Knoten in seinem Objekt steht.
  final String key;

  /// Der Platz im Feld, oder -1 außerhalb eines Feldes.
  final int index;

  /// Was hier steht.
  final JsonNodeType type;

  /// Der angezeigte Wert, auf [jsonValueChars] gekürzt.
  final String display;

  /// Der ganze Wert, für Kopieren und Tooltip.
  final String full;

  /// Wie viele Kinder der Knoten hat.
  final int childCount;

  /// Das erste Kind, oder -1.
  int firstChild = -1;

  /// Das nächste Geschwister, oder -1.
  int nextSibling = -1;

  /// Wahr für einen Knoten, unter dem etwas liegt.
  bool get isContainer =>
      type == JsonNodeType.object || type == JsonNodeType.array;
}

/// Der ganze Baum.
class JsonDocument {
  /// Creates a document.
  JsonDocument({
    required this.nodes,
    required this.capped,
    required this.depthCapped,
    required this.duplicateKeys,
    required this.findingsByNode,
    required this.markedAncestors,
    required this.unlocatedFindings,
  });

  /// Die Knoten in Vorordnung; die Wurzel steht auf null.
  final List<JsonNode> nodes;

  /// Wahr, wenn [jsonMaxNodes] erreicht wurde.
  final bool capped;

  /// Wahr, wenn irgendwo [jsonMaxDepth] erreicht wurde.
  final bool depthCapped;

  /// Wahr, wenn die Quelle einen Schlüssel zweimal im selben Objekt führt.
  ///
  /// `jsonDecode` behält den letzten; die Ansicht sagt, dass etwas
  /// verschwunden ist, statt den Verlust zu verschweigen.
  final bool duplicateKeys;

  /// Welche Funde an welchem Knoten stehen, und wo genau.
  final Map<int, List<JsonMark>> findingsByNode;

  /// Knoten auf dem Weg zu einem Fund; sie tragen den Punkt.
  final Set<int> markedAncestors;

  /// Funde, die in diesem Baum nicht zu finden waren.
  ///
  /// Sie verschwinden nicht: die Ansicht nennt sie und verweist auf die
  /// Rohansicht, in der jeder Fund über seinen Versatz sitzt.
  final Set<int> unlocatedFindings;
}

/// Eine Zeile der Rohansicht.
class BodyRow {
  /// Creates a row.
  const BodyRow({
    required this.charStart,
    required this.charEnd,
    required this.line,
    required this.continued,
  });

  /// Erste Code-Unit der Zeile im bereinigten Text.
  final int charStart;

  /// Erste Code-Unit hinter der Zeile.
  final int charEnd;

  /// Die Nummer der Quellzeile, eins-basiert.
  final int line;

  /// Wahr für die Fortsetzung einer Zeile, die länger als [bodyRowChars] war.
  final bool continued;

  /// Wie viele Zeichen die Zeile trägt.
  int get length => charEnd - charStart;
}

/// Der Rumpf als Text.
class BodyText {
  /// Creates the text model.
  const BodyText({
    required this.text,
    required this.rows,
    required this.rowsCapped,
    required this.longestRow,
  });

  /// Der ganze bereinigte Text.
  final String text;

  /// Die Zeilen.
  final List<BodyRow> rows;

  /// Wahr, wenn [bodyMaxRows] erreicht wurde.
  final bool rowsCapped;

  /// Die Länge der längsten Zeile, für die Breite des Inhalts.
  final int longestRow;

  /// Der Ausschnitt von [row].
  String slice(BodyRow row) => text.substring(row.charStart, row.charEnd);
}

/// Ein Paar eines Formularrumpfs.
class FormPair {
  /// Creates a pair.
  const FormPair({
    required this.name,
    required this.value,
    required this.nameByteOfChar,
    required this.byteOfChar,
  });

  /// Der dekodierte Name.
  final String name;

  /// Zu jeder Code-Unit von [name] das Byte, an dem sie beginnt, plus einem
  /// Abschluss am Ende.
  ///
  /// Ein Eintrag mehr als Zeichen, damit jede Code-Unit einen **Bereich** hat
  /// und nicht nur einen Punkt: `%40` sind drei Bytes und ein Zeichen, und ein
  /// Fund, der nur die beiden Hex-Ziffern trifft, gehört trotzdem auf dieses
  /// Zeichen. Ein Fund kann auch im Namen stehen; ohne diese Tabelle wäre er
  /// in dieser Ansicht unsichtbar.
  final List<int> nameByteOfChar;

  /// Der dekodierte Wert.
  final String value;

  /// Zu jeder Code-Unit von [value] das Byte, an dem sie beginnt, plus einem
  /// Abschluss am Ende. Aufbau wie [nameByteOfChar].
  final List<int> byteOfChar;
}

/// Alles, was eine Rumpf-Ansicht braucht.
class ParsedBody {
  /// Creates a parsed body.
  const ParsedBody({
    required this.kind,
    required this.findings,
    required this.bytes,
    this.encodingLabel = '',
    this.text,
    this.json,
    this.form,
    this.problem,
    this.disputedType = false,
    this.findingsPlaced = true,
  });

  /// Wie der Rumpf angezeigt wird.
  final BodyKind kind;

  /// Die Funde, auf Zeichen umgerechnet.
  final List<BodyFinding> findings;

  /// Die Bytes, in denen die Fundstellen liegen.
  ///
  /// Nicht dasselbe wie die Bytes des Transports: war die Anfrage gepackt und
  /// ließ sie sich auspacken, sind das die ausgepackten. Die Hex-Ansicht liest
  /// sie, damit sie denselben Byteraum zeigt, in dem der Daemon gesucht hat.
  final Uint8List bytes;

  /// Der `Content-Encoding` der Anfrage, für den Satz dazu.
  final String encodingLabel;

  /// Der Rumpf als Text, oder null für Binärdaten.
  final BodyText? text;

  /// Der Baum, oder null.
  final JsonDocument? json;

  /// Die Paare, oder null.
  final List<FormPair>? form;

  /// Warum eine Ansicht fehlt, oder null.
  final BodyProblem? problem;

  /// Wahr, wenn der `Content-Type` etwas anderes sagte als die Bytes zeigen.
  final bool disputedType;

  /// Wahr, wenn diese Bytes die sind, auf denen der Daemon gesucht hat.
  ///
  /// Ist sie falsch, behalten die Funde ihren Namen und verlieren ihre Stelle:
  /// eine Markierung im falschen Byteraum zeigt auf einen Wert, der dort nie
  /// stand, und das ist schlimmer als gar keine Markierung.
  final bool findingsPlaced;

  /// Die Funde, die eine Ansicht zeichnen darf.
  List<BodyFinding> get placedFindings =>
      findingsPlaced ? findings : const <BodyFinding>[];
}

/// Zerlegt [bytes] als [kind].
///
/// Oberste Ebene und ohne Flutter, damit `Isolate.run` das Ergebnis
/// zurückgeben kann.
ParsedBody parseBody(
  Uint8List bytes,
  BodyKind kind,
  List<Finding> findings, {
  bool disputedType = false,
  BodyProblem? problem,
  bool placeFindings = true,
  String encodingLabel = '',
}) {
  final List<BodyFinding> mapped = mapBodyFindings(
    bytes,
    findings,
    place: placeFindings,
  );
  if (kind == BodyKind.empty) {
    return ParsedBody(
      kind: kind,
      findings: mapped,
      bytes: bytes,
      encodingLabel: encodingLabel,
      problem: problem,
      findingsPlaced: placeFindings,
    );
  }
  if (kind == BodyKind.binary) {
    return ParsedBody(
      kind: kind,
      findings: mapped,
      bytes: bytes,
      encodingLabel: encodingLabel,
      problem: problem,
      disputedType: disputedType,
      findingsPlaced: placeFindings,
    );
  }
  final BodyText text = buildBodyText(bytes);
  if (kind == BodyKind.json) {
    final Object? decoded = _tryDecodeJson(text.text);
    if (decoded == null) {
      return ParsedBody(
        kind: kind,
        findings: mapped,
        bytes: bytes,
        encodingLabel: encodingLabel,
        text: text,
        problem: problem ?? BodyProblem.notJson,
        disputedType: disputedType,
        findingsPlaced: placeFindings,
      );
    }
    return ParsedBody(
      kind: kind,
      findings: mapped,
      bytes: bytes,
      encodingLabel: encodingLabel,
      text: text,
      json: buildJsonDocument(decoded, text.text, mapped),
      problem: problem,
      disputedType: disputedType,
      findingsPlaced: placeFindings,
    );
  }
  if (kind == BodyKind.form) {
    final List<FormPair> pairs = parseFormPairs(bytes);
    return ParsedBody(
      kind: kind,
      findings: mapped,
      bytes: bytes,
      encodingLabel: encodingLabel,
      text: text,
      form: pairs,
      problem: pairs.isEmpty ? (problem ?? BodyProblem.notForm) : problem,
      disputedType: disputedType,
      findingsPlaced: placeFindings,
    );
  }
  return ParsedBody(
    kind: kind,
    findings: mapped,
    bytes: bytes,
    encodingLabel: encodingLabel,
    text: text,
    problem: problem,
    disputedType: disputedType,
    findingsPlaced: placeFindings,
  );
}

/// Zerlegt [bytes], über [bodyIsolateThreshold] auf einem anderen Isolate.
Future<ParsedBody> parseBodyAsync(
  Uint8List bytes,
  BodyKind kind,
  List<Finding> findings, {
  bool disputedType = false,
  BodyProblem? problem,
  bool placeFindings = true,
  String encodingLabel = '',
}) async {
  if (bytes.length <= bodyIsolateThreshold) {
    return parseBody(
      bytes,
      kind,
      findings,
      disputedType: disputedType,
      problem: problem,
      placeFindings: placeFindings,
      encodingLabel: encodingLabel,
    );
  }
  return Isolate.run(
    () => parseBody(
      bytes,
      kind,
      findings,
      disputedType: disputedType,
      problem: problem,
      placeFindings: placeFindings,
      encodingLabel: encodingLabel,
    ),
  );
}

/// Der Text von [bytes], bereinigt und in Zeilen zerlegt.
BodyText buildBodyText(Uint8List bytes) {
  final String text = sanitizeBodyText(
    const Utf8Decoder(allowMalformed: true).convert(bytes),
  );
  final List<BodyRow> rows = <BodyRow>[];
  int longest = 0;
  int line = 1;
  int start = 0;
  bool capped = false;
  while (start <= text.length) {
    int end = text.indexOf('\n', start);
    if (end < 0) {
      end = text.length;
    }
    int from = start;
    bool continued = false;
    do {
      final int to = end - from > bodyRowChars ? from + bodyRowChars : end;
      if (rows.length >= bodyMaxRows) {
        capped = true;
        break;
      }
      rows.add(
        BodyRow(charStart: from, charEnd: to, line: line, continued: continued),
      );
      if (to - from > longest) {
        longest = to - from;
      }
      from = to;
      continued = true;
    } while (from < end);
    if (capped) {
      break;
    }
    if (end >= text.length) {
      break;
    }
    start = end + 1;
    line++;
  }
  return BodyText(
    text: text,
    rows: rows,
    rowsCapped: capped,
    longestRow: longest,
  );
}

/// Die Paare eines Formularrumpfs, prozentdekodiert, `+` als Leerzeichen.
List<FormPair> parseFormPairs(Uint8List bytes) {
  final List<FormPair> pairs = <FormPair>[];
  int start = 0;
  while (start <= bytes.length) {
    int end = start;
    while (end < bytes.length && bytes[end] != 0x26) {
      end++;
    }
    if (end > start) {
      int split = start;
      while (split < end && bytes[split] != 0x3D) {
        split++;
      }
      final _DecodedField name = _decodeField(bytes, start, split);
      final _DecodedField value = split < end
          ? _decodeField(bytes, split + 1, end)
          : _DecodedField('', <int>[end]);
      pairs.add(
        FormPair(
          name: sanitizeBodyText(name.text),
          value: sanitizeBodyText(value.text),
          nameByteOfChar: name.byteOfChar,
          byteOfChar: value.byteOfChar,
        ),
      );
    }
    if (end >= bytes.length) {
      break;
    }
    start = end + 1;
  }
  return pairs;
}

/// Baut den Baum aus [decoded].
///
/// Iterativ über einen eigenen Stapel: ein Rumpf, der tausend Ebenen tief
/// verschachtelt ist, darf nicht den Aufrufstapel sprengen, und er tut es hier
/// auch nicht, weil ab [jsonMaxDepth] ein [JsonNodeType.elided] steht.
JsonDocument buildJsonDocument(
  Object? decoded,
  String source,
  List<BodyFinding> findings,
) {
  final List<JsonNode> nodes = <JsonNode>[
    _nodeFor(decoded, parent: -1, depth: 0, key: '', index: -1),
  ];
  bool capped = false;
  bool depthCapped = false;
  final List<_Frame> stack = <_Frame>[];
  if (nodes.first.isContainer) {
    stack.add(_Frame(decoded, 0, 0));
  }
  while (stack.isNotEmpty) {
    final _Frame frame = stack.last;
    if (frame.done) {
      stack.removeLast();
      continue;
    }
    if (nodes.length >= jsonMaxNodes) {
      capped = true;
      break;
    }
    final _Child child = frame.next();
    final int depth = frame.depth + 1;
    final bool tooDeep =
        depth >= jsonMaxDepth &&
        (child.value is Map<String, Object?> || child.value is List<Object?>);
    if (tooDeep) {
      depthCapped = true;
    }
    final JsonNode node = tooDeep
        ? JsonNode(
            parent: frame.node,
            depth: depth,
            key: child.key,
            index: child.index,
            type: JsonNodeType.elided,
            display: '',
            full: '',
            childCount: 0,
          )
        : _nodeFor(
            child.value,
            parent: frame.node,
            depth: depth,
            key: child.key,
            index: child.index,
          );
    final int at = nodes.length;
    nodes.add(node);
    if (frame.previous < 0) {
      nodes[frame.node].firstChild = at;
    } else {
      nodes[frame.previous].nextSibling = at;
    }
    frame.previous = at;
    if (!tooDeep && node.isContainer && node.childCount > 0) {
      stack.add(_Frame(child.value, at, depth));
    }
  }
  final _FindingMap map = _locateFindings(nodes, findings);
  return JsonDocument(
    nodes: nodes,
    capped: capped,
    depthCapped: depthCapped,
    duplicateKeys: hasDuplicateJsonKeys(source),
    findingsByNode: map.byNode,
    markedAncestors: map.ancestors,
    unlocatedFindings: map.unlocated,
  );
}

/// Wahr, wenn in [source] ein Objekt denselben Schlüssel zweimal führt.
///
/// `jsonDecode` behält stillschweigend den letzten. Ein Rumpf, der zweimal
/// `"amount"` schreibt und darauf baut, dass Mensch und Empfänger
/// verschiedene davon sehen, ist genau der Fall, für den dieser Durchgang da
/// ist.
bool hasDuplicateJsonKeys(String source) {
  final List<Set<String>> levels = <Set<String>>[];
  int i = 0;
  while (i < source.length) {
    final int unit = source.codeUnitAt(i);
    if (unit == 0x7B) {
      levels.add(<String>{});
      i++;
      continue;
    }
    if (unit == 0x7D) {
      if (levels.isNotEmpty) {
        levels.removeLast();
      }
      i++;
      continue;
    }
    if (unit != 0x22) {
      i++;
      continue;
    }
    final int from = i + 1;
    int at = from;
    while (at < source.length) {
      final int c = source.codeUnitAt(at);
      if (c == 0x5C) {
        at += 2;
        continue;
      }
      if (c == 0x22) {
        break;
      }
      at++;
    }
    final String literal = source.substring(
      from,
      at.clamp(from, source.length),
    );
    i = at + 1;
    int after = i;
    while (after < source.length && _isJsonSpace(source.codeUnitAt(after))) {
      after++;
    }
    final bool isKey =
        after < source.length && source.codeUnitAt(after) == 0x3A;
    if (isKey && levels.isNotEmpty && !levels.last.add(literal)) {
      return true;
    }
  }
  return false;
}

bool _isJsonSpace(int unit) =>
    unit == 0x20 || unit == 0x09 || unit == 0x0A || unit == 0x0D;

/// Der dekodierte JSON-Wert, oder null, wenn es keiner ist.
Object? _tryDecodeJson(String text) {
  try {
    final Object? value = jsonDecode(text);
    return value ?? const <String, Object?>{};
  } on FormatException {
    return null;
  }
}

JsonNode _nodeFor(
  Object? value, {
  required int parent,
  required int depth,
  required String key,
  required int index,
}) {
  if (value is Map<String, Object?>) {
    return JsonNode(
      parent: parent,
      depth: depth,
      key: key,
      index: index,
      type: JsonNodeType.object,
      display: '',
      full: '',
      childCount: value.length,
    );
  }
  if (value is List<Object?>) {
    return JsonNode(
      parent: parent,
      depth: depth,
      key: key,
      index: index,
      type: JsonNodeType.array,
      display: '',
      full: '',
      childCount: value.length,
    );
  }
  final (JsonNodeType type, String full) = switch (value) {
    null => (JsonNodeType.nul, 'null'),
    final bool b => (JsonNodeType.boolean, b ? 'true' : 'false'),
    final num n => (JsonNodeType.number, n.toString()),
    _ => (JsonNodeType.string, sanitizeBodyText(value.toString())),
  };
  return JsonNode(
    parent: parent,
    depth: depth,
    key: key,
    index: index,
    type: type,
    display: full.length > jsonValueChars
        ? '${full.substring(0, jsonValueChars)}…'
        : full,
    full: full,
    childCount: 0,
  );
}

/// Wo die Funde im Baum stehen.
///
/// Der Baum kennt keine Byte-Versätze mehr — `jsonDecode` hat sie verloren —,
/// also wird der Treffertext gesucht. Gesucht wird in dem Text, der auch
/// **gezeichnet** wird: im Schlüssel bis [jsonKeyChars], im Wert bis
/// [jsonValueChars]. Was dahinter liegt, gilt als nicht verortet und wird von
/// der Ansicht genannt; ein Unterstrich unter dem sichtbaren Anfang behauptete
/// sonst eine Fundstelle, die dort nicht steht.
_FindingMap _locateFindings(List<JsonNode> nodes, List<BodyFinding> findings) {
  final Map<int, List<JsonMark>> byNode = <int, List<JsonMark>>{};
  final Set<int> ancestors = <int>{};
  final Set<int> unlocated = <int>{
    for (final BodyFinding finding in findings) finding.index,
  };
  if (findings.isEmpty) {
    return _FindingMap(byNode, ancestors, unlocated);
  }
  if (nodes.length * findings.length > jsonFindingBudget) {
    return _FindingMap(byNode, ancestors, unlocated);
  }
  for (int n = 0; n < nodes.length; n++) {
    final JsonNode node = nodes[n];
    if (node.isContainer) {
      continue;
    }
    for (final BodyFinding finding in findings) {
      if (finding.needle.isEmpty) {
        continue;
      }
      final int inKey = _drawnIndexOf(node.key, finding.needle, jsonKeyChars);
      final int inValue = inKey >= 0
          ? -1
          : _drawnIndexOf(node.display, finding.needle, jsonValueChars);
      if (inKey < 0 && inValue < 0) {
        continue;
      }
      final int at = inKey >= 0 ? inKey : inValue;
      (byNode[n] ??= <JsonMark>[]).add(
        JsonMark(
          finding: finding.index,
          inKey: inKey >= 0,
          start: at,
          end: at + finding.needle.length,
        ),
      );
      unlocated.remove(finding.index);
      int parent = node.parent;
      while (parent >= 0 && ancestors.add(parent)) {
        parent = nodes[parent].parent;
      }
    }
  }
  return _FindingMap(byNode, ancestors, unlocated);
}

/// Der Platz von [needle] in [text], aber nur, wenn er ganz in den ersten
/// [cap] Zeichen liegt. Sonst -1.
int _drawnIndexOf(String text, String needle, int cap) {
  final int at = text.indexOf(needle);
  return at >= 0 && at + needle.length <= cap ? at : -1;
}

class _FindingMap {
  const _FindingMap(this.byNode, this.ancestors, this.unlocated);

  final Map<int, List<JsonMark>> byNode;
  final Set<int> ancestors;
  final Set<int> unlocated;
}

/// Ein Container, dessen Kinder noch nicht alle im Baum stehen.
class _Frame {
  _Frame(Object? value, this.node, this.depth)
    : _entries = value is Map<String, Object?> ? value.keys.toList() : null,
      _map = value is Map<String, Object?> ? value : null,
      _list = value is List<Object?> ? value : null;

  final List<String>? _entries;
  final Map<String, Object?>? _map;
  final List<Object?>? _list;
  final int node;
  final int depth;
  int previous = -1;
  int _at = 0;

  bool get done => _at >= (_entries?.length ?? _list?.length ?? 0);

  _Child next() {
    final List<String>? entries = _entries;
    if (entries != null) {
      final String key = entries[_at++];
      return _Child(sanitizeBodyText(key), -1, _map![key]);
    }
    final int index = _at++;
    return _Child('', index, _list![index]);
  }
}

class _Child {
  const _Child(this.key, this.index, this.value);

  final String key;
  final int index;
  final Object? value;
}

class _DecodedField {
  const _DecodedField(this.text, this.byteOfChar);

  final String text;
  final List<int> byteOfChar;
}

/// Dekodiert `bytes[from, to)` als Formularfeld und merkt sich je Zeichen das
/// Byte, aus dem es stammt.
_DecodedField _decodeField(Uint8List bytes, int from, int to) {
  final List<int> raw = <int>[];
  final List<int> origin = <int>[];
  int i = from;
  while (i < to) {
    final int byte = bytes[i];
    if (byte == 0x2B) {
      raw.add(0x20);
      origin.add(i);
      i++;
      continue;
    }
    if (byte == 0x25 && i + 2 < to) {
      final int? high = _hexDigit(bytes[i + 1]);
      final int? low = _hexDigit(bytes[i + 2]);
      if (high != null && low != null) {
        raw.add(high * 16 + low);
        origin.add(i);
        i += 3;
        continue;
      }
    }
    raw.add(byte);
    origin.add(i);
    i++;
  }
  // Ein Zeichen kann aus mehreren dekodierten Bytes entstehen; die Tabelle
  // wird deshalb beim Dekodieren nachgezogen, statt sie zu erraten.
  final List<int> byteOfChar = <int>[];
  final StringBuffer text = StringBuffer();
  int at = 0;
  while (at < raw.length) {
    final int length = _utf8Length(raw, at);
    final String piece = const Utf8Decoder(allowMalformed: true)
        .convert(raw, at, at + length);
    text.write(piece);
    for (int u = 0; u < piece.length; u++) {
      byteOfChar.add(origin[at]);
    }
    at += length;
  }
  // Der Abschluss: das Byte hinter dem letzten Zeichen. Damit hat jede
  // Code-Unit einen halboffenen Bereich `[byteOfChar[i], byteOfChar[i + 1])`.
  byteOfChar.add(to);
  return _DecodedField(text.toString(), byteOfChar);
}

int _utf8Length(List<int> bytes, int at) {
  final int lead = bytes[at];
  final int length = lead >= 0xF0
      ? 4
      : lead >= 0xE0
      ? 3
      : lead >= 0xC0
      ? 2
      : 1;
  return at + length > bytes.length ? 1 : length;
}

int? _hexDigit(int byte) {
  if (byte >= 0x30 && byte <= 0x39) {
    return byte - 0x30;
  }
  if (byte >= 0x41 && byte <= 0x46) {
    return byte - 0x37;
  }
  if (byte >= 0x61 && byte <= 0x66) {
    return byte - 0x57;
  }
  return null;
}
