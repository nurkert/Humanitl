// Die Sprachprüfung der Oberfläche (HUM-052). Aufruf aus `app/`:
//
//   dart run tool/l10n_lint.dart [--root <Repository>]
//
// Fünf Prüfungen, jede mit eigenem Namen in der Ausgabe:
//
// 1. `parity`: `app_de.arb` hat genau die Schlüssel von `app_en.arb`, und jede
//    Nachricht benutzt in beiden Sprachen dieselben Platzhalter.
// 2. `description`: Jeder Schlüssel in `app_en.arb` hat einen `@`-Eintrag mit
//    `description`.
// 3. `literal`: Kein String-Literal mit einem Wort in dem, was angezeigt
//    wird: das erste Argument von `Text`, `SelectableText`, `RichText` und
//    ihren `.rich`-Formen, und jedes benannte Argument aus `textArguments`
//    oder auf `Label` (`text:`, `title:`, `hint:`, `body:`, `message:`,
//    `tooltip:`, `semanticsLabel:` ...), jeweils der ganze Ausdruck bis zum
//    nächsten `,` (Bedingung, Verkettung, verschachtelte Literale in einer
//    Interpolation). Gilt in `app/lib/features/**` und
//    `app/packages/ui/lib/**`; ausgenommen sind Dateien mit `// l10n-exempt`
//    in der ersten Zeile (Galerie). Ein Literal ohne Buchstaben außerhalb
//    seiner Interpolationen ist kein Wort (`'${a} · ${b}'`) und zählt nicht.
//    Listen sichtbarer Texte (`labels:`, `columns:`, `options:`, `items:`
//    und alles auf `Labels`) zählen mit ihren Elementen. Eine Konstante
//    derselben Datei (`const`, `static const`, `final` mit String- oder
//    Listen- oder Map-Initialisierer) zählt an einer Textposition wie ihr
//    Literal, auch in einer Interpolation. Flutters `...Text`-Parameter
//    (`hintText`, `labelText` ...) und `Semantics(value:)` zählen mit.
//    Grenze: Konstanten aus anderen Dateien löst die Prüfung nicht auf; dafür
//    bräuchte sie den Analyzer (CONVENTIONS 4.36).
// 4. `diagnostic`: Jeder Code des Registers
//    (`daemon/crates/core-types/src/diagnostics/codes.rs`) hat
//    `diag<CODE>Title` und `diag<CODE>Why`, der Code ohne Unterstrich.
// 5. `glossary`: Jede Zeile der Tabelle „Begriffe“ in `docs/GLOSSARY.md` nennt
//    einen Schlüssel, der in beiden Dateien steht und den Begriff der Sprache
//    als ganzes Wort enthält; kein deutscher Text enthält ein Wort der Tabelle
//    „Verbotene Wörter“.
//
// Exit-Code 0 ohne Befund, 1 mit Befund, 2 bei einem Fehler des Aufrufs.

import 'dart:convert';
import 'dart:io';

/// Ein Verstoß: die Prüfung, die ihn fand, und wo.
class L10nFinding {
  /// Erzeugt einen Befund.
  const L10nFinding(this.check, this.message);

  /// Der Name der Prüfung (`parity`, `literal`, ...).
  final String check;

  /// Was falsch ist und wo.
  final String message;

  @override
  String toString() => '$check: $message';
}

/// Die Nachrichten einer ARB-Datei: jeder Schlüssel, der nicht mit `@`
/// beginnt.
Map<String, String> arbMessages(Map<String, Object?> arb) => <String, String>{
  for (final MapEntry<String, Object?> entry in arb.entries)
    if (!entry.key.startsWith('@') && entry.value is String)
      entry.key: entry.value! as String,
};

/// Die Platzhalter, die [message] benutzt, gelesen als ICU MessageFormat mit
/// `use-escaping: true` (ein Apostroph leitet einen wörtlichen Abschnitt ein,
/// zwei Apostrophe sind einer).
///
/// Die Zweige von `plural` und `select` sind wieder Nachrichten; ihr Text ist
/// kein Platzhalter, ihr `{name}` schon.
Set<String> placeholdersOf(String message) {
  final _IcuReader reader = _IcuReader(message);
  reader.message(nested: false);
  return reader.names;
}

class _IcuReader {
  _IcuReader(this.text);

  final String text;
  final Set<String> names = <String>{};
  int at = 0;

  bool get done => at >= text.length;
  String get char => text[at];

  void message({required bool nested}) {
    while (!done) {
      final String c = char;
      if (c == "'") {
        _quote();
      } else if (c == '{') {
        _argument();
      } else if (c == '}') {
        if (!nested) {
          throw FormatException('unbalanced "}"', text, at);
        }
        at++;
        return;
      } else {
        at++;
      }
    }
    if (nested) {
      throw FormatException('missing "}"', text, at);
    }
  }

  void _quote() {
    if (at + 1 < text.length && text[at + 1] == "'") {
      at += 2;
      return;
    }
    final int end = text.indexOf("'", at + 1);
    if (end < 0) {
      throw FormatException('unterminated quote', text, at);
    }
    at = end + 1;
  }

  void _spaces() {
    while (!done && char.trim().isEmpty) {
      at++;
    }
  }

  String _word() {
    final int start = at;
    while (!done && RegExp(r'[A-Za-z0-9_]').hasMatch(char)) {
      at++;
    }
    if (start == at) {
      throw FormatException('expected a name', text, at);
    }
    return text.substring(start, at);
  }

  void _expect(String c) {
    if (done || char != c) {
      throw FormatException('expected "$c"', text, at);
    }
    at++;
  }

  void _argument() {
    _expect('{');
    _spaces();
    names.add(_word());
    _spaces();
    if (!done && char == '}') {
      at++;
      return;
    }
    _expect(',');
    _spaces();
    final String kind = _word();
    _spaces();
    if (!done && char == '}') {
      at++;
      return;
    }
    _expect(',');
    if (kind != 'plural' && kind != 'select' && kind != 'selectordinal') {
      // Ein Stil wie `{n, number, integer}`: darin steht nichts Geschachteltes.
      final int end = text.indexOf('}', at);
      if (end < 0) {
        throw FormatException('missing "}"', text, at);
      }
      at = end + 1;
      return;
    }
    while (true) {
      _spaces();
      if (done) {
        throw FormatException('missing "}"', text, at);
      }
      if (char == '}') {
        at++;
        return;
      }
      final int start = at;
      while (!done && char != '{' && char.trim().isNotEmpty) {
        at++;
      }
      if (start == at) {
        throw FormatException('expected a selector', text, at);
      }
      _spaces();
      _expect('{');
      message(nested: true);
    }
  }
}

/// Prüfung 1 und 2: Beide ARB-Dateien stimmen überein, und Englisch
/// beschreibt jeden Schlüssel.
List<L10nFinding> checkArb(Map<String, Object?> en, Map<String, Object?> de) {
  final List<L10nFinding> found = <L10nFinding>[];
  final Map<String, String> source = arbMessages(en);
  final Map<String, String> german = arbMessages(de);
  for (final String key in source.keys) {
    final String? translated = german[key];
    if (translated == null) {
      found.add(L10nFinding('parity', '$key is missing in app_de.arb'));
      continue;
    }
    try {
      final Set<String> want = placeholdersOf(source[key]!);
      final Set<String> have = placeholdersOf(translated);
      if (want.length != have.length || !want.containsAll(have)) {
        found.add(
          L10nFinding(
            'parity',
            '$key uses {${want.join(', ')}} in en but '
                '{${have.join(', ')}} in de',
          ),
        );
      }
    } on FormatException catch (error) {
      found.add(L10nFinding('parity', '$key does not parse: ${error.message}'));
    }
    final Object? meta = en['@$key'];
    if (meta is! Map || meta['description'] is! String) {
      found.add(L10nFinding('description', '$key has no @$key.description'));
    }
  }
  for (final String key in german.keys) {
    if (!source.containsKey(key)) {
      found.add(L10nFinding('parity', '$key is in app_de.arb only'));
    }
  }
  return found;
}

/// Die Widgets, deren erstes Positionsargument angezeigter Text ist, auch als
/// `Text.rich(...)` und `SelectableText.rich(...)`.
const Set<String> textWidgets = <String>{'Text', 'SelectableText', 'RichText'};

/// Die benannten Argumente, die angezeigten oder gesprochenen Text tragen,
/// gesammelt aus den Konstruktoren von `packages/ui` und `core/ui` (HUM-052):
/// `text` eines `TextSpan`, `title` eines Fensters, einer Karte oder eines
/// Sheets, `hint` eines Feldes, `body` eines Modals, `message` und `tooltip`,
/// `why` und `detail` einer Diagnose-Karte und `label` selbst. Jeder Name auf
/// `Label` zählt ebenso ([isTextArgument]).
///
/// Benannte Argumente, die kein Mensch als Text liest, bleiben mit Absicht
/// draußen: `code` (ein Diagnose-Code wie `SANDBOX_001`), `method` (ein
/// HTTP-Verb), `docsUrl`, `workDir`, `shortcut` (eine Taste wie `Ctrl+K`) und
/// jeder Schlüssel und jede Id. Sie sind in jeder Sprache gleich.
const Set<String> textArguments = <String>{
  'label',
  'text',
  'title',
  'hint',
  'body',
  'message',
  'tooltip',
  'why',
  'detail',
  'semanticsValue',
};

/// Benannte Argumente, deren Name nach Text aussieht, die aber keiner sind:
/// `debugLabel` benennt einen `FocusNode` oder `ScrollController` für den
/// Debugger und erreicht nie den Bildschirm oder einen Screenreader.
const Set<String> notTextArguments = <String>{'debugLabel'};

/// Aufrufe, deren Argumente nie angezeigt werden, auch nicht in einem
/// angezeigten Ausdruck: Ein Widget-Schlüssel ist ein Bezeichner für Tests
/// und den Element-Baum (`Text(key: Key('audit-total'), ...)`).
const Set<String> keyCalls = <String>{
  'Key',
  'ValueKey',
  'ObjectKey',
  'GlobalKey',
  'PageStorageKey',
};

/// Aufrufe, deren Textargumente die Worte des Absenders sind und nicht die
/// der Anwendung: Ein `Diagnostic` trägt `title` und `why` so, wie der Daemon
/// sie schickt, und die Oberfläche übersetzt ihn am Code (`DiagnosticL10n`,
/// `diag<CODE>Title` und `diag<CODE>Why`), nie am Satz (`docs/UX.md` 4.4,
/// CONVENTIONS 4.36).
const Set<String> senderCalls = <String>{'Diagnostic'};

/// Ob ein benanntes Argument namens [name] angezeigten oder gesprochenen Text
/// trägt.
///
/// Neben [textArguments] jeder Name auf `Label` oder `Text`; letztere sind
/// die von Flutter (`hintText`, `labelText`, `errorText`, `helperText`).
/// `value:` zählt nur innerhalb von `Semantics` ([semanticsOnlyArgument]).
bool isTextArgument(String name) =>
    !notTextArguments.contains(name) &&
    (textArguments.contains(name) ||
        name.endsWith('Label') ||
        name.endsWith('Text'));

/// Der eine allgemeine Name, der nur in einem Aufruf Text ist:
/// `Semantics(value:)` sagt ein Screenreader vor; `value:` sonst ist Daten.
const String semanticsOnlyArgument = 'value';

/// Ob [at] ein Listenliteral öffnet und keinen Index: `<String>['a']` und
/// `['a']` sind Listen, `data['secret']` und `m[k]` sind Nachschlagen.
bool _isListLiteral(List<_Token> tokens, int at) {
  if (tokens[at].text != '[') {
    return false;
  }
  if (at == 0) {
    return true;
  }
  final _Token before = tokens[at - 1];
  if (before.kind == _Kind.identifier) {
    return before.text == 'const' || before.text == 'return';
  }
  return before.text != ')' &&
      before.text != ']' &&
      before.text != '!' &&
      before.kind != _Kind.string;
}

/// Ob [name] in dieser Datei auch ein Parameter oder ein Feld ohne
/// Initialisierer ist (`String title,`, `this.title`, `final String title;`),
/// sodass `title: title` diesen Wert weiterreichen kann und nicht eine
/// Konstante gleichen Namens.
bool _declaredWithoutInitializer(List<_Token> tokens, String name) {
  const Set<String> notTypes = <String>{
    'return',
    'await',
    'yield',
    'else',
    'case',
    'in',
    'is',
    'as',
    'throw',
  };
  for (int j = 1; j + 1 < tokens.length; j++) {
    if (tokens[j].text != name || tokens[j].kind != _Kind.identifier) {
      continue;
    }
    final String next = tokens[j + 1].text;
    if (next != ',' && next != ')' && next != ';' && next != '}') {
      continue;
    }
    final _Token before = tokens[j - 1];
    if (before.text == '.' && j > 1 && tokens[j - 2].text == 'this') {
      return true;
    }
    if ((before.kind == _Kind.identifier && !notTypes.contains(before.text)) ||
        before.text == '?' ||
        before.text == '>') {
      return true;
    }
  }
  return false;
}

/// Benannte Argumente, die eine Liste angezeigter Texte tragen, gesammelt aus
/// den Konstruktoren von `packages/ui` und den Features: `labels`, `columns`
/// (`SandboxTable`), `options` und `items` und jeder Name auf `Labels`
/// (`tabLabels`, `durationLabels`, `targetLabels`) ([isTextListArgument]).
///
/// Nur ein Element der Liste selbst zählt, nicht das Argument eines Aufrufs
/// darin: In `options: [HSegmentOption(value: 'allow', label: l10n.x)]` ist
/// `value` ein Bezeichner, und `label:` wird für sich geprüft.
const Set<String> textListArguments = <String>{
  'labels',
  'columns',
  'options',
  'items',
};

/// Ob ein benanntes Argument namens [name] eine Liste angezeigter Texte
/// trägt.
bool isTextListArgument(String name) =>
    textListArguments.contains(name) || name.endsWith('Labels');

/// Der Index der Klammer, die die Liste, den Aufruf oder den Block öffnet,
/// in dem das Token an [at] steht, oder `null` auf oberster Ebene der Datei.
int? _enclosingBracket(List<_Token> tokens, int at) {
  int depth = 0;
  for (int j = at - 1; j >= 0; j--) {
    final String t = tokens[j].text;
    if (tokens[j].kind != _Kind.symbol) {
      continue;
    }
    if (t == ')' || t == ']' || t == '}') {
      depth++;
    } else if (t == '(' || t == '[' || t == '{') {
      if (depth == 0) {
        return j;
      }
      depth--;
    }
  }
  return null;
}

/// Der Name des Aufrufs, in dessen Argumentliste das Token an [at] steht,
/// oder `null` in einer Liste, einem Block oder auf oberster Ebene.
/// `ValueKey<String>(` zählt als `ValueKey`.
String? _enclosingCall(List<_Token> tokens, int at) {
  final int? j = _enclosingBracket(tokens, at);
  if (j == null || tokens[j].text != '(') {
    return null;
  }
  int k = j - 1;
  if (k >= 0 && tokens[k].text == '>') {
    int angle = 0;
    for (; k >= 0; k--) {
      if (tokens[k].text == '>') {
        angle++;
      } else if (tokens[k].text == '<') {
        angle--;
        if (angle == 0) {
          break;
        }
      }
    }
    k--;
  }
  return k >= 0 && tokens[k].kind == _Kind.identifier ? tokens[k].text : null;
}

/// Das erste Positionsargument des Aufrufs, dessen Liste bei [from] beginnt;
/// benannte Argumente davor werden übersprungen (`Text(key: k, 'Hello')`).
/// `null`, wenn es keines gibt.
int? _firstPositional(List<_Token> tokens, int from) {
  int p = from;
  while (p + 1 < tokens.length &&
      tokens[p].kind == _Kind.identifier &&
      tokens[p + 1].text == ':') {
    final int end = _expressionEnd(tokens, p + 2);
    if (end >= tokens.length || tokens[end].text != ',') {
      return null;
    }
    p = end + 1;
  }
  return p < tokens.length && tokens[p].text != ')' ? p : null;
}

/// Der Index hinter dem Ausdruck, der bei [from] beginnt: das erste `,` oder
/// `;` auf Tiefe 0 oder die Klammer, die die umgebende schließt.
int _expressionEnd(List<_Token> tokens, int from) {
  int depth = 0;
  int j = from;
  for (; j < tokens.length; j++) {
    final _Token t = tokens[j];
    if (t.kind != _Kind.symbol) {
      continue;
    }
    if (t.text == '(' || t.text == '[' || t.text == '{') {
      depth++;
    } else if (t.text == ')' || t.text == ']' || t.text == '}') {
      if (depth == 0) {
        return j;
      }
      depth--;
    } else if ((t.text == ',' || t.text == ';') && depth == 0) {
      return j;
    }
  }
  return j;
}

/// Prüfung 3: String-Literale mit einem Wort darin, wo Text angezeigt wird.
///
/// Ein Literal zählt, wo immer es im angezeigten Ausdruck steht: hinter einer
/// Bedingung (`Text(on ? 'A' : 'B')`), in einer Verkettung (`x + 'more'`), in
/// einer Interpolation (`'${a ? "on" : "off"}'`) oder in einem `TextSpan`
/// unter `Text.rich`. [path] nennt nur die Datei im Befund.
List<L10nFinding> checkLiterals(String path, String source) {
  final String firstLine = source.split('\n').first.trim();
  if (firstLine.startsWith('// l10n-exempt')) {
    return const <L10nFinding>[];
  }
  final List<_Token> tokens = _DartScanner(source).tokens();
  final Map<String, List<String>> constants = _stringConstants(tokens);
  // Token-Index auf die Stelle, die es anzeigt; die erste Stelle gewinnt,
  // sodass ein Literal unter `Text.rich(TextSpan(text: ...))` einmal zählt.
  // Ein Token ist ein String-Literal oder der Name einer Konstante dieser
  // Datei.
  final Map<int, String> shown = <int, String>{};
  bool isConstant(int k) {
    if (tokens[k].kind != _Kind.identifier ||
        !constants.containsKey(tokens[k].text)) {
      return false;
    }
    // `widget.title` ist ein Feld, nicht die Konstante `title` dieser Datei;
    // `_Texts.title` nennt eine statische Konstante über ihre Klasse.
    if (k > 0 && tokens[k - 1].text == '.') {
      return k > 1 && RegExp(r'^_?[A-Z]').hasMatch(tokens[k - 2].text);
    }
    // Ein Aufruf dieses Namens ist nicht die Konstante, ebenso wenig ein
    // benanntes Argument dieses Namens: `name:` direkt nach `(` oder `,`. Das
    // `name :` einer Bedingung (`c ? _on : l10n.off`) ist die Konstante.
    if (k + 1 < tokens.length && tokens[k + 1].text == '(') {
      return false;
    }
    final bool namedArgument =
        k + 1 < tokens.length &&
        tokens[k + 1].text == ':' &&
        k > 0 &&
        (tokens[k - 1].text == '(' || tokens[k - 1].text == ',');
    return !namedArgument;
  }

  void mark(int from, String place, {int? onlyIn}) {
    final int end = _expressionEnd(tokens, from);
    for (int k = from; k < end; k++) {
      if (tokens[k].kind != _Kind.string && !isConstant(k)) {
        continue;
      }
      if (keyCalls.contains(_enclosingCall(tokens, k))) {
        continue;
      }
      // Ein Index ist ein Nachschlagen, kein Text: `data['secret']`,
      // `cfg['key']`, `data[_key]`. Ein Literal in einem geschachtelten
      // Aufruf (`Text(format('Hello'))`) zählt weiterhin.
      final int? index = _enclosingBracket(tokens, k);
      if (index != null &&
          index >= from &&
          tokens[index].text == '[' &&
          !_isListLiteral(tokens, index)) {
        continue;
      }
      if (tokens[k].kind == _Kind.identifier) {
        // `title: title` reicht einen Parameter oder ein Feld gleichen Namens
        // weiter, aber nur, wenn die Datei eines deklariert; sonst ist es die
        // Konstante.
        if (k == from &&
            from >= 2 &&
            tokens[from - 2].text == tokens[k].text &&
            _declaredWithoutInitializer(tokens, tokens[k].text)) {
          continue;
        }
        // Eine Konstante, die an eine ARB-Nachricht geht, ist ein Platzhalter,
        // nicht der Text: `l10n.auditRetentionChain(auditRetentionKey)` zeigt
        // den übersetzten Satz. Nur `l10n.<member>(...)`; jeder andere Aufruf
        // (`emphasize(_title)`, `Wrap(_g)`, `(_g)`) kann seine Eingabe zeigen.
        final int? bracket = _enclosingBracket(tokens, k);
        if (bracket != null &&
            bracket >= from &&
            tokens[bracket].text == '(' &&
            bracket >= 3 &&
            tokens[bracket - 1].kind == _Kind.identifier &&
            tokens[bracket - 2].text == '.' &&
            tokens[bracket - 3].text == 'l10n') {
          continue;
        }
      }
      if (onlyIn != null) {
        final int? bracket = _enclosingBracket(tokens, k);
        if (bracket != onlyIn &&
            (bracket == null || !_isListLiteral(tokens, bracket))) {
          continue;
        }
      }
      shown.putIfAbsent(k, () => place);
    }
  }

  for (int i = 0; i + 2 < tokens.length; i++) {
    final _Token head = tokens[i];
    if (head.kind != _Kind.identifier) {
      continue;
    }
    if (textWidgets.contains(head.text)) {
      final int? start = tokens[i + 1].text == '('
          ? i + 2
          : (i + 3 < tokens.length &&
                tokens[i + 1].text == '.' &&
                tokens[i + 2].text == 'rich' &&
                tokens[i + 3].text == '(')
          ? i + 4
          : null;
      final int? positional = start == null
          ? null
          : _firstPositional(tokens, start);
      if (positional != null) {
        mark(positional, start == i + 2 ? head.text : '${head.text}.rich');
      }
      continue;
    }
    // Ein benanntes Argument: `name:` direkt nach `(` oder `,`. Nicht das
    // `a ? name : b` einer Bedingung, kein Map-Eintrag, kein `case`.
    final bool named =
        (isTextArgument(head.text) ||
            (head.text == semanticsOnlyArgument &&
                _enclosingCall(tokens, i) == 'Semantics')) &&
        tokens[i + 1].text == ':' &&
        i > 0 &&
        (tokens[i - 1].text == '(' || tokens[i - 1].text == ',') &&
        !senderCalls.contains(_enclosingCall(tokens, i));
    if (named) {
      mark(i + 2, '${head.text}:');
    }
    final bool namedList =
        isTextListArgument(head.text) &&
        tokens[i + 1].text == ':' &&
        i > 0 &&
        (tokens[i - 1].text == '(' || tokens[i - 1].text == ',');
    if (namedList) {
      mark(i + 2, '${head.text}:', onlyIn: _enclosingBracket(tokens, i));
    }
  }

  final List<L10nFinding> found = <L10nFinding>[];
  final List<int> indices = shown.keys.toList()..sort();
  final RegExp word = RegExp(r'\p{L}', unicode: true);
  for (final int k in indices) {
    final _Token token = tokens[k];
    if (token.kind == _Kind.string) {
      // `'$_greeting'` und `'${_greeting}'` zeigen die Konstante.
      for (final String name in token.names) {
        for (final String text in constants[name] ?? const <String>[]) {
          if (word.hasMatch(text)) {
            found.add(
              L10nFinding(
                'literal',
                '$path:${token.line}: ${shown[k]} with the constant '
                    '$name = "${text.trim()}" in an interpolation; move it '
                    'to app/l10n/app_en.arb',
              ),
            );
          }
        }
      }
      if (word.hasMatch(token.text)) {
        found.add(
          L10nFinding(
            'literal',
            '$path:${token.line}: ${shown[k]} with the literal '
                '"${token.text.trim()}"; move it to app/l10n/app_en.arb',
          ),
        );
      }
      continue;
    }
    for (final String text in constants[token.text]!) {
      if (word.hasMatch(text)) {
        found.add(
          L10nFinding(
            'literal',
            '$path:${token.line}: ${shown[k]} with the constant '
                '${token.text} = "${text.trim()}"; move it to '
                'app/l10n/app_en.arb',
          ),
        );
      }
    }
  }
  return found;
}

/// Die String-Konstanten einer Datei: jede Deklaration mit `const`,
/// `static const` oder `final` und Initialisierer, abgebildet auf die
/// Literale dieses Initialisierers (eines für `'Hello'`, mehrere für
/// `<String>['A', 'B']` oder eine Bedingung). Literale in einem Aufruf des
/// Initialisierers zählen nicht (`final t = Key('x')`,
/// `final s = format('x')`).
///
/// **Nur diese Datei.** Eine aus einer anderen Datei importierte Konstante
/// wird nicht aufgelöst; dafür bräuchte die Prüfung den Analyzer, und eine
/// Textkonstante in einer geteilten Datei ist genau das, was ARB ersetzt.
/// So steht es in CONVENTIONS 4.36.
Map<String, List<String>> _stringConstants(List<_Token> tokens) {
  final Map<String, List<String>> out = <String, List<String>>{};
  // Die Konstanten, die jede interpoliert, für das Auflösen am Ende.
  final Map<String, List<String>> refs = <String, List<String>>{};
  for (int i = 1; i + 2 < tokens.length; i++) {
    if (tokens[i].kind != _Kind.identifier || tokens[i + 1].text != '=') {
      continue;
    }
    // Zurück zum Anfang der Anweisung: `const` oder `final` auf ihrer obersten
    // Ebene. Auch über ein `,` hinweg, sodass `const _a = 'x', _b = 'Hello'`
    // beide deklariert; ein offenes `(` oder `[` bedeutet eine Argumentliste,
    // keine Deklaration (`f(a = 1)`; `{String a = 'x'}` endet am `{`).
    bool declared = false;
    int depth = 0;
    for (int j = i - 1; j >= 0; j--) {
      final String t = tokens[j].text;
      if (tokens[j].kind == _Kind.symbol) {
        if (t == ')' || t == ']') {
          depth++;
          continue;
        }
        if (t == '(' || t == '[') {
          if (depth == 0) {
            break;
          }
          depth--;
          continue;
        }
        if (depth == 0 && (t == ';' || t == '{' || t == '}')) {
          break;
        }
        continue;
      }
      if (depth == 0 && (t == 'const' || t == 'final')) {
        declared = true;
        break;
      }
    }
    if (!declared) {
      continue;
    }
    final int? outer = _enclosingBracket(tokens, i);
    // Zuerst die Typargumente: Das Komma in `<K, String>{...}` beendet den
    // Initialisierer nicht.
    int start = i + 2;
    if (start < tokens.length && tokens[start].text == '<') {
      int angle = 0;
      for (; start < tokens.length; start++) {
        if (tokens[start].text == '<') {
          angle++;
        } else if (tokens[start].text == '>') {
          angle--;
          if (angle == 0) {
            start++;
            break;
          }
        }
      }
    }
    final int end = _expressionEnd(tokens, start);
    // Ein Literal des Initialisierers selbst, ein Element eines
    // Listenliterals oder der Wert eines Map-Eintrags (`{K.a: 'Allow'}`);
    // kein Argument eines Aufrufs (`format('x')`) und kein Index
    // (`data['secret']`).
    bool shownPart(int k) {
      final int? bracket = _enclosingBracket(tokens, k);
      if (bracket == outer) {
        return true;
      }
      if (bracket == null || bracket < i) {
        return false;
      }
      if (_isListLiteral(tokens, bracket)) {
        return true;
      }
      return tokens[bracket].text == '{' && tokens[k - 1].text == ':';
    }

    final List<String> texts = <String>[];
    final List<String> names = <String>[];
    for (int k = start; k < end; k++) {
      if (tokens[k].kind == _Kind.string && shownPart(k)) {
        texts.add(tokens[k].text);
        names.addAll(tokens[k].names);
      }
    }
    if (texts.isNotEmpty) {
      out[tokens[i].text] = texts;
      refs[tokens[i].text] = names;
    }
  }
  // Eine Konstante, die eine andere interpoliert (`const _s = '$_g'`), zeigt
  // auch deren Text; transitiv aufgelöst, jede Konstante einmal, sodass ein
  // Kreis (`_a = '$_b'`, `_b = '$_a'`) endet.
  List<String> expand(String name, Set<String> seen) {
    if (!seen.add(name) || !out.containsKey(name)) {
      return const <String>[];
    }
    return <String>[
      ...out[name]!,
      for (final String ref in refs[name] ?? const <String>[])
        ...expand(ref, seen),
    ];
  }

  return <String, List<String>>{
    for (final String name in out.keys) name: expand(name, <String>{}),
  };
}

enum _Kind { identifier, string, symbol }

class _Token {
  const _Token(this.kind, this.text, this.line, [this.names = const []]);

  final _Kind kind;

  /// Bei einem String: die Bezeichner, die er allein interpoliert (`$name`,
  /// `${name}`), zum Auflösen gegen die Konstanten der Datei.
  final List<String> names;

  /// Bei einem String: sein statischer Text, mit den Literalen aus seinen
  /// Interpolationen angehängt.
  final String text;
  final int line;
}

/// Gerade genug von einem Dart-Lexer, um String-Literale und die Bezeichner
/// um sie herum zu finden: Kommentare werden übersprungen, und ein String
/// behält seinen statischen Text und den jedes Literals in seinen
/// Interpolationen.
class _DartScanner {
  _DartScanner(this.source);

  final String source;
  int at = 0;
  int line = 1;

  /// Die Bezeichner, die der gerade gelesene String interpoliert.
  List<String> _names = <String>[];

  bool get done => at >= source.length;

  bool _startsWith(String prefix) => source.startsWith(prefix, at);

  void _advance([int n = 1]) {
    for (int k = 0; k < n && !done; k++) {
      if (source[at] == '\n') {
        line++;
      }
      at++;
    }
  }

  List<_Token> tokens() {
    final List<_Token> out = <_Token>[];
    while (!done) {
      final _Token? token = _next();
      if (token != null) {
        out.add(token);
      }
    }
    return out;
  }

  _Token? _next() {
    final String c = source[at];
    if (c.trim().isEmpty) {
      _advance();
      return null;
    }
    if (_startsWith('//')) {
      while (!done && source[at] != '\n') {
        _advance();
      }
      return null;
    }
    if (_startsWith('/*')) {
      _blockComment();
      return null;
    }
    final int startLine = line;
    if (_startsWith('r"') || _startsWith("r'")) {
      _advance();
      _names = <String>[];
      return _Token(_Kind.string, _string(raw: true), startLine, _names);
    }
    if (c == '"' || c == "'") {
      _names = <String>[];
      return _Token(_Kind.string, _string(raw: false), startLine, _names);
    }
    if (RegExp(r'[A-Za-z_$]').hasMatch(c)) {
      final int start = at;
      while (!done && RegExp(r'[A-Za-z0-9_$]').hasMatch(source[at])) {
        _advance();
      }
      return _Token(_Kind.identifier, source.substring(start, at), startLine);
    }
    _advance();
    return _Token(_Kind.symbol, c, startLine);
  }

  void _blockComment() {
    int depth = 0;
    while (!done) {
      if (_startsWith('/*')) {
        depth++;
        _advance(2);
      } else if (_startsWith('*/')) {
        depth--;
        _advance(2);
        if (depth == 0) {
          return;
        }
      } else {
        _advance();
      }
    }
  }

  /// Liest ein Literal ab seinem Anführungszeichen und gibt seinen statischen
  /// Text zurück.
  String _string({required bool raw}) {
    final String quote = source[at];
    final bool triple = _startsWith(quote * 3);
    final String end = triple ? quote * 3 : quote;
    _advance(end.length);
    final StringBuffer text = StringBuffer();
    while (!done) {
      if (_startsWith(end)) {
        _advance(end.length);
        return text.toString();
      }
      final String c = source[at];
      if (!raw && c == r'\') {
        _escape();
        text.write('_');
        continue;
      }
      if (!raw && _startsWith(r'${')) {
        _advance(2);
        final int start = at;
        // Ein Literal in der Interpolation wird mit angezeigt:
        // `'${on ? "An" : "Aus"}'` zeigt „An“ oder „Aus“.
        _interpolation(text);
        final String inner = source.substring(start, at - 1).trim();
        if (RegExp(r'^[A-Za-z_][A-Za-z0-9_]*$').hasMatch(inner)) {
          _names.add(inner);
        }
        continue;
      }
      if (!raw && c == r'$') {
        _advance();
        final int start = at;
        while (!done && RegExp(r'[A-Za-z0-9_]').hasMatch(source[at])) {
          _advance();
        }
        _names.add(source.substring(start, at));
        continue;
      }
      text.write(c);
      _advance();
    }
    return text.toString();
  }

  /// Überspringt eine Escape-Folge: `\n`, `\x41`, `\u00B7`, `\u{1F600}`. Ihre
  /// Hex-Ziffern sind keine Buchstaben eines Wortes.
  void _escape() {
    _advance();
    if (done) {
      return;
    }
    final String kind = source[at];
    _advance();
    if (kind == 'x') {
      _advance(2);
    } else if (kind == 'u' && !done && source[at] == '{') {
      while (!done && source[at] != '}') {
        _advance();
      }
      _advance();
    } else if (kind == 'u') {
      _advance(4);
    }
  }

  /// Überspringt `${ ... }` samt geschachtelter Klammern; der statische Text
  /// jedes String-Literals darin geht nach [text], durch Leerzeichen
  /// abgesetzt.
  void _interpolation(StringBuffer text) {
    int depth = 1;
    while (!done) {
      final String c = source[at];
      if (c == '{') {
        depth++;
        _advance();
      } else if (c == '}') {
        depth--;
        _advance();
        if (depth == 0) {
          return;
        }
      } else if (_startsWith('r"') || _startsWith("r'")) {
        _advance();
        text.write(' ${_string(raw: true)} ');
      } else if (c == '"' || c == "'") {
        text.write(' ${_string(raw: false)} ');
      } else {
        _advance();
      }
    }
  }
}

/// Die Codes des Registers, in der Reihenfolge, in der sie dort stehen.
List<String> registerCodes(String codesRs) => <String>[
  for (final RegExpMatch match in RegExp(
    r'^\s+([A-Z]+_\d{3}) =>',
    multiLine: true,
  ).allMatches(codesRs))
    match.group(1)!,
];

/// Das Präfix des ARB-Schlüssels eines Diagnose-Codes: `SANDBOX_001` ist
/// `diagSANDBOX001`.
String diagnosticKey(String code) => 'diag${code.replaceAll('_', '')}';

/// Prüfung 4: Jeder registrierte Code hat einen Titel und einen Grund.
List<L10nFinding> checkDiagnostics(
  List<String> codes,
  Map<String, Object?> en,
) {
  final Map<String, String> messages = arbMessages(en);
  return <L10nFinding>[
    for (final String code in codes)
      for (final String part in const <String>['Title', 'Why'])
        if (!messages.containsKey('${diagnosticKey(code)}$part'))
          L10nFinding(
            'diagnostic',
            '$code has no ${diagnosticKey(code)}$part in app_en.arb',
          ),
  ];
}

/// Eine Zeile der Glossar-Tabelle.
class GlossaryEntry {
  /// Erzeugt eine Zeile.
  const GlossaryEntry(this.term, this.en, this.de, this.key);

  /// Der Begriff, den die Zeile festlegt.
  final String term;

  /// Das englische Wort.
  final String en;

  /// Das deutsche Wort.
  final String de;

  /// Der ARB-Schlüssel, der beide trägt.
  final String key;
}

List<List<String>> _table(String markdown, String heading) {
  final List<String> lines = markdown.split('\n');
  final int start = lines.indexWhere((String l) => l.trim() == '## $heading');
  if (start < 0) {
    return const <List<String>>[];
  }
  final List<List<String>> rows = <List<String>>[];
  for (final String raw in lines.skip(start + 1)) {
    final String l = raw.trim();
    if (l.startsWith('## ')) {
      break;
    }
    if (!l.startsWith('|')) {
      continue;
    }
    final List<String> cells = l
        .substring(1, l.endsWith('|') ? l.length - 1 : l.length)
        .split('|')
        .map((String c) => c.trim())
        .toList();
    if (cells.every((String c) => RegExp(r'^:?-+:?$').hasMatch(c))) {
      continue;
    }
    rows.add(cells);
  }
  // Die erste Zeile ist der Kopf.
  return rows.skip(1).toList();
}

/// Die Zeilen der Tabelle „Begriffe“ in `docs/GLOSSARY.md`.
List<GlossaryEntry> glossaryEntries(String markdown) => <GlossaryEntry>[
  for (final List<String> row in _table(markdown, 'Begriffe'))
    if (row.length >= 4)
      GlossaryEntry(row[0], row[1], row[2], row[3].replaceAll('`', '')),
];

/// Die Wörter der Tabelle „Verbotene Wörter“ in `docs/GLOSSARY.md`.
List<String> forbiddenWords(String markdown) => <String>[
  for (final List<String> row in _table(markdown, 'Verbotene Wörter'))
    if (row.isNotEmpty && row.first.isNotEmpty) row.first,
];

String _unescape(String message) => message.replaceAll("''", "'");

bool _hasWord(String text, String word) => RegExp(
  '(?<![\\p{L}\\p{N}])${RegExp.escape(word)}(?![\\p{L}\\p{N}])',
  caseSensitive: false,
  unicode: true,
).hasMatch(_unescape(text));

/// Prüfung 5: Das Glossar steht in beiden ARB-Dateien, und Deutsch meidet
/// die verbotenen Wörter.
List<L10nFinding> checkGlossary(
  String markdown,
  Map<String, Object?> en,
  Map<String, Object?> de,
) {
  final List<L10nFinding> found = <L10nFinding>[];
  final List<GlossaryEntry> entries = glossaryEntries(markdown);
  if (entries.isEmpty) {
    found.add(
      const L10nFinding(
        'glossary',
        'docs/GLOSSARY.md has no rows under "## Begriffe"',
      ),
    );
  }
  final Map<String, String> source = arbMessages(en);
  final Map<String, String> german = arbMessages(de);
  for (final GlossaryEntry entry in entries) {
    for (final (String lang, Map<String, String> messages, String word)
        in <(String, Map<String, String>, String)>[
          ('en', source, entry.en),
          ('de', german, entry.de),
        ]) {
      final String? message = messages[entry.key];
      if (message == null) {
        found.add(
          L10nFinding(
            'glossary',
            '"${entry.term}": ${entry.key} is missing in app_$lang.arb',
          ),
        );
      } else if (!_hasWord(message, word)) {
        found.add(
          L10nFinding(
            'glossary',
            '"${entry.term}": ${entry.key} in $lang is "$message", '
                'not "$word"',
          ),
        );
      }
    }
  }
  for (final String word in forbiddenWords(markdown)) {
    final RegExp start = RegExp(
      '(?<![\\p{L}\\p{N}])${RegExp.escape(word)}',
      caseSensitive: false,
      unicode: true,
    );
    for (final MapEntry<String, String> message in german.entries) {
      if (start.hasMatch(message.value)) {
        found.add(
          L10nFinding(
            'glossary',
            '${message.key} in de uses the forbidden word "$word"',
          ),
        );
      }
    }
  }
  return found;
}

/// Jede Dart-Datei unter [dir], sortiert, ohne erzeugten Code.
List<File> dartFiles(Directory dir) {
  if (!dir.existsSync()) {
    return const <File>[];
  }
  final List<File> files =
      dir
          .listSync(recursive: true)
          .whereType<File>()
          .where(
            (File f) =>
                f.path.endsWith('.dart') &&
                !f.path.endsWith('.g.dart') &&
                !f.path.endsWith('.freezed.dart'),
          )
          .toList()
        ..sort((File a, File b) => a.path.compareTo(b.path));
  return files;
}

/// Führt alle fünf Prüfungen gegen das Repository unter [root] aus und gibt
/// die Befunde zurück.
List<L10nFinding> lintRepository(Directory root) {
  String read(String relative) =>
      File('${root.path}/$relative').readAsStringSync();
  Map<String, Object?> arb(String relative) =>
      (jsonDecode(read(relative)) as Map<String, Object?>);

  final Map<String, Object?> en = arb('app/l10n/app_en.arb');
  final Map<String, Object?> de = arb('app/l10n/app_de.arb');
  final List<L10nFinding> found = <L10nFinding>[...checkArb(en, de)];
  for (final String scanned in const <String>[
    'app/lib/features',
    'app/packages/ui/lib',
  ]) {
    for (final File file in dartFiles(Directory('${root.path}/$scanned'))) {
      final String relative = file.path.substring(root.path.length + 1);
      found.addAll(checkLiterals(relative, file.readAsStringSync()));
    }
  }
  final List<String> codes = registerCodes(
    read('daemon/crates/core-types/src/diagnostics/codes.rs'),
  );
  if (codes.isEmpty) {
    found.add(const L10nFinding('diagnostic', 'no code found in the register'));
  }
  found.addAll(checkDiagnostics(codes, en));
  found.addAll(checkGlossary(read('docs/GLOSSARY.md'), en, de));
  return found;
}

/// Führt die Prüfung aus und gibt den Exit-Code zurück: 0 sauber, 1 Befunde.
int run(Directory root, {IOSink? out, IOSink? err}) {
  final IOSink report = out ?? stdout;
  final IOSink problems = err ?? stderr;
  final List<L10nFinding> found = lintRepository(root);
  for (final L10nFinding finding in found) {
    problems.writeln(finding);
  }
  report.writeln(
    found.isEmpty ? 'l10n_lint: ok' : 'l10n_lint: ${found.length} finding(s)',
  );
  return found.isEmpty ? 0 : 1;
}

void main(List<String> args) {
  Directory root = Directory.current.parent;
  final int flag = args.indexOf('--root');
  if (flag >= 0) {
    if (flag + 1 >= args.length) {
      stderr.writeln('usage: dart run tool/l10n_lint.dart [--root <repo>]');
      exitCode = 2;
      return;
    }
    root = Directory(args[flag + 1]);
  }
  if (!File('${root.path}/app/l10n/app_en.arb').existsSync()) {
    stderr.writeln(
      'l10n_lint: ${root.path} is not the repository; run from app/ or pass '
      '--root',
    );
    exitCode = 2;
    return;
  }
  exitCode = run(root);
}
