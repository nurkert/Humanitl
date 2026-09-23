// Die Sprachprüfung (`tool/l10n_lint.dart`, HUM-052): gegen das echte
// Repository sauber, und jede ihrer fünf Prüfungen schlägt an einem kleinen
// Beispiel-Repository an, das genau einen Verstoß trägt.

import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';

import '../../tool/l10n_lint.dart';

const String _codes = '''
registry! {
    /// Ein Code.
    DEMO_001 => "demo", "Titel", "#demo_001",
        "Auslöser.",
        "Fix.";
}
''';

const String _glossary = '''
# Glossar

## Begriffe

| Begriff | en | de | ARB-Schlüssel | Anmerkung |
|---|---|---|---|---|
| held | Held | Angehalten | `stateHeld` | |

## Verbotene Wörter

| Wort | statt | Grund |
|---|---|---|
| abgefangen | angehalten | |
''';

Map<String, Object?> _en() => <String, Object?>{
  '@@locale': 'en',
  'stateHeld': 'Held',
  '@stateHeld': <String, Object?>{'description': 'State.'},
  'count': '{n, plural, =1{One item} other{{n} items}}',
  '@count': <String, Object?>{
    'description': 'Count.',
    'placeholders': <String, Object?>{
      'n': <String, Object?>{'type': 'int'},
    },
  },
  'diagDEMO001Title': 'Title',
  '@diagDEMO001Title': <String, Object?>{'description': 'Title.'},
  'diagDEMO001Why': 'Why',
  '@diagDEMO001Why': <String, Object?>{'description': 'Why.'},
};

Map<String, Object?> _de() => <String, Object?>{
  '@@locale': 'de',
  'stateHeld': 'Angehalten',
  'count': '{n, plural, =1{Ein Eintrag} other{{n} Einträge}}',
  'diagDEMO001Title': 'Titel',
  'diagDEMO001Why': 'Grund',
};

const String _cleanFeature = '''
import 'package:flutter/widgets.dart';

Widget a(String name) => Text(name);
''';

/// Legt ein kleines Repository an und gibt seine Wurzel zurück.
Directory repo({
  Map<String, Object?>? en,
  Map<String, Object?>? de,
  Map<String, String> features = const <String, String>{
    'clean.dart': _cleanFeature,
  },
  Map<String, String> ui = const <String, String>{},
  String codes = _codes,
  String glossary = _glossary,
}) {
  final Directory root = Directory.systemTemp.createTempSync('hum052-lint-');
  addTearDown(() => root.deleteSync(recursive: true));
  void write(String relative, String text) {
    final File file = File('${root.path}/$relative')
      ..createSync(recursive: true);
    file.writeAsStringSync(text);
  }

  write('app/l10n/app_en.arb', jsonEncode(en ?? _en()));
  write('app/l10n/app_de.arb', jsonEncode(de ?? _de()));
  features.forEach(
    (String name, String text) => write('app/lib/features/x/$name', text),
  );
  ui.forEach(
    (String name, String text) => write('app/packages/ui/lib/src/$name', text),
  );
  write('daemon/crates/core-types/src/diagnostics/codes.rs', codes);
  write('docs/GLOSSARY.md', glossary);
  return root;
}

List<String> checksOf(Directory root) =>
    lintRepository(root).map((L10nFinding f) => f.check).toList();

void main() {
  group('the repository', () {
    final Directory root = Directory.current.parent;

    test('is clean', () {
      expect(lintRepository(root).map((L10nFinding f) => '$f'), isEmpty);
    });

    test('has at least the 44 glossary entries of the specification', () {
      final List<GlossaryEntry> entries = glossaryEntries(
        File('${root.path}/docs/GLOSSARY.md').readAsStringSync(),
      );
      expect(entries.length, greaterThanOrEqualTo(44));
    });

    test('knows every code of the register', () {
      final List<String> codes = registerCodes(
        File('${root.path}/daemon/crates/core-types/src/diagnostics/codes.rs')
            .readAsStringSync(),
      );
      expect(codes.length, greaterThan(100));
      expect(codes, contains('DAEMON_001'));
    });
  });

  group('a clean example', () {
    test('exits 0', () {
      expect(run(repo(), out: _Sink(), err: _Sink()), 0);
    });
  });

  group('literal', () {
    test('an intentional literal in a Text exits 1', () {
      final Directory root = repo(
        features: <String, String>{
          'bad.dart': "import 'x.dart';\n\nWidget b() => Text('Hello');\n",
        },
      );
      expect(run(root, out: _Sink(), err: _Sink()), 1);
      expect(
        lintRepository(root).single.message,
        contains('app/lib/features/x/bad.dart:3'),
      );
    });

    test('double quotes, const, a line break and label:', () {
      expect(
        checkLiterals('a.dart', 'x() => const Text(\n  "Hello",\n);'),
        hasLength(1),
      );
      expect(checkLiterals('a.dart', "x() => f(label: 'Save');"), hasLength(1));
      expect(
        checkLiterals('a.dart', "x() => f(tooltip: 'Open it');"),
        hasLength(1),
      );
      expect(
        checkLiterals('a.dart', "x() => f(semanticsLabel: 'close');"),
        hasLength(1),
      );
      // Nebeneinanderstehende Literale: jedes Stück zählt für sich.
      expect(checkLiterals('a.dart', "x() => Text('Hel' 'lo');"), hasLength(2));
    });

    test('packages/ui is scanned as well', () {
      final Directory root = repo(
        ui: <String, String>{'w.dart': "x() => Text('Hello');\n"},
      );
      expect(checksOf(root), <String>['literal']);
    });

    test('no word, no finding', () {
      expect(checkLiterals('a.dart', "x() => Text('·');"), isEmpty);
      expect(checkLiterals('a.dart', r"x() => Text('\u00B7');"), isEmpty);
      expect(
        checkLiterals('a.dart', r"x() => Text('${l10n.a} · ${l10n.b}');"),
        isEmpty,
      );
      expect(checkLiterals('a.dart', r"x() => Text('$count');"), isEmpty);
      expect(checkLiterals('a.dart', "x() => Text(l10n.title);"), isEmpty);
    });

    // Die Formen aus den Reviews von HUM-052: Jede muss anschlagen.
    for (final (String form, String source) in <(String, String)>[
      (
        'a literal in an interpolation',
        r"""x() => Text('${"Unlocalized"}');""",
      ),
      (
        'a condition in an interpolation',
        r"""x() => Text('${a ? "Active" : "Inactive"}');""",
      ),
      ('Text.rich', "x() => Text.rich(TextSpan(text: 'Hello'));"),
      ('SelectableText.rich', "x() => SelectableText.rich(f('Hello'));"),
      ('TextSpan(text:)', "x() => f(TextSpan(text: 'Hello'));"),
      ('title:', "x() => WidgetsApp(title: 'Humanitl design gallery');"),
      ('hint:', "x() => HTextField(hint: 'Search');"),
      ('body:', "x() => HModal(body: 'Really?');"),
      ('closeLabel:', "x() => HSheet(closeLabel: 'Close');"),
      ('a condition', "x() => Text(on ? 'Active' : 'Inactive');"),
      ('a concatenation', "x() => Text(count + ' items');"),
      ('a named argument first', "x() => Text(key: k, 'Hello');"),
      // Listen sichtbarer Texte (zweite Review-Runde).
      (
        'tabLabels:',
        "x() => DraftEditor(tabLabels: <String>['Body', 'Headers']);",
      ),
      ('labels:', "x() => HTabs(labels: const ['One', 'Two']);"),
      (
        'columns:',
        "x() => SandboxTable(columns: <String>['Path', 'Mode'], rows: r);",
      ),
      // Konstanten derselben Datei in einer Textposition.
      ('Text(_title)', "const _title = 'Hello';\nx() => Text(_title);"),
      (
        'label: _label',
        "final String _label = 'Save';\nx() => f(label: _label);",
      ),
      (
        'a static const',
        "class A {\n  static const String title = 'Welcome';\n}\n"
            'x() => Text(A.title);',
      ),
      (
        'a const list',
        "const List<String> _tabs = <String>['Body', 'Headers'];\n"
            'x() => DraftEditor(tabLabels: _tabs);',
      ),
      // Dritte Review-Runde.
      (
        'a constant in the ? branch',
        "const _a = 'Active';\nx() => Text(c ? _a : l10n.x);",
      ),
      (
        'a const map',
        "const _m = <K, String>{K.a: 'Allow'};\nx() => Text(_m[k]!);",
      ),
      ('hintText:', "x() => InputDecoration(hintText: 'Search');"),
      ('labelText:', "x() => InputDecoration(labelText: 'Name');"),
      ('errorText:', "x() => InputDecoration(errorText: 'Wrong');"),
      ('helperText:', "x() => InputDecoration(helperText: 'Help');"),
      ('Semantics(value:)', "x() => Semantics(value: 'Three findings');"),
      (
        'a constant in an uppercase constructor',
        "const _g = 'Hello';\nx() => HCard(title: Wrap(_g));",
      ),
      ('a constant in parentheses', "const _g = 'Hello';\nx() => Text((_g));"),
      (
        'a constant in any call but l10n',
        "const _title = 'Hello';\nx() => Text(emphasize(_title));",
      ),
      ('a constant in \$name', "const _g = 'Hello';\nx() => Text('\$_g!');"),
      (
        'a constant in \${name}',
        "const _g = 'Hello';\nx() => Text('\${_g}!');",
      ),
      (
        'title: title without a parameter',
        "const title = 'Welcome';\nx() => HCard(title: title);",
      ),
      (
        'the second constant of one declaration',
        "const _a = 'x', _b = 'Hello';\nx() => Text(_b);",
      ),
      // Vierte Review-Runde.
      (
        'a constant interpolated in another constant',
        "const _g = 'Hello';\nconst _s = '\$_g';\nx() => Text(_s);",
      ),
      // Kein Schutz einer Regel aus Runde 4, sondern Verhalten seit Runde 1:
      // Ein Literal in einem geschachtelten Aufruf kann angezeigter Text
      // sein und zählt. Es steht hier, damit die Index-Ausnahme es nicht
      // unbemerkt mit verschluckt.
      ('a literal in a nested call', "x() => Text(format('Hello'));"),
      // Fünfte Runde: Neben einem Index zählt der Rest des Ausdrucks, und
      // eine Liste mit Index ist eine Liste.
      (
        'a fallback beside an index',
        "x() => Text(data['secret'] ?? 'Fallback');",
      ),
      ('an indexed list literal', "x() => Text(['Hello'][0]);"),
    ]) {
      test('flags $form', () {
        expect(checkLiterals('a.dart', source), isNotEmpty);
      });
    }

    test('the fallback is named, the index key is not', () {
      final List<L10nFinding> found = checkLiterals(
        'a.dart',
        "x() => Text(data['secret'] ?? 'Fallback');",
      );
      expect(found, hasLength(1));
      expect(found.single.message, contains('Fallback'));
      expect(found.single.message, isNot(contains('secret')));
    });

    test('what is not text is named with its reason, not skipped blindly', () {
      // Ein Schlüssel ist ein Bezeichner (keyCalls).
      expect(
        checkLiterals(
          'a.dart',
          "x() => Text(key: const Key('audit-total'), a);",
        ),
        isEmpty,
      );
      expect(
        checkLiterals(
          'a.dart',
          "x() => Text(a, key: ValueKey<String>('b-x'));",
        ),
        isEmpty,
      );
      // `debugLabel` erreicht nie den Bildschirm (notTextArguments).
      expect(
        checkLiterals('a.dart', "x() => FocusNode(debugLabel: 'shell');"),
        isEmpty,
      );
      // Ein Diagnostic trägt die Worte des Absenders und wird am Code
      // übersetzt (senderCalls).
      expect(
        checkLiterals(
          'a.dart',
          "x() => Diagnostic(code: c, why: 'flow gone');",
        ),
        isEmpty,
      );
      // In einer Liste zählt nur das Element selbst, nicht das Argument eines
      // Aufrufs darin: `value` ist ein Bezeichner, `label:` prüft sich selbst.
      expect(
        checkLiterals(
          'a.dart',
          "x() => HSegmented(options: [HSegmentOption(value: 'allow', "
              'label: l10n.allow)]);',
        ),
        isEmpty,
      );
      // Eine Konstante als Eingabe eines Aufrufs ist nicht der Text: die
      // ARB-Nachricht zeigt den Konfigurationsschlüssel als Platzhalter.
      expect(
        checkLiterals(
          'a.dart',
          "const String key = 'audit.retention_days';\n"
              'x() => Text(l10n.auditRetentionChain(key));',
        ),
        isEmpty,
      );
      // Ein Literal im Aufruf des Initialisierers ist dessen Eingabe.
      expect(
        checkLiterals('a.dart', "final s = format('Hello');\nx() => Text(s);"),
        isEmpty,
      );
      // Ein Index ist keine Liste: `data['secret']` ist kein Text.
      expect(checkLiterals('a.dart', "x() => Text(data['secret']);"), isEmpty);
      expect(
        checkLiterals('a.dart', "x() => HCard(title: cfg['key']);"),
        isEmpty,
      );
      // Eine Konstante als Index-Schlüssel ist ebenfalls ein Nachschlagen.
      expect(
        checkLiterals(
          'a.dart',
          "const _key = 'secret';\nx() => Text(data[_key]!);",
        ),
        isEmpty,
      );
      // Zwei Konstanten, die einander interpolieren, enden.
      expect(
        checkLiterals(
          'a.dart',
          "const _a = '\$_b';\nconst _b = '\$_a';\nx() => Text(_a);",
        ),
        isEmpty,
      );
      expect(
        checkLiterals(
          'a.dart',
          "final v = data['secret'] == true ? '•••' : '';\nx() => Text(v);",
        ),
        isEmpty,
      );
      // `value:` außerhalb von `Semantics` ist Daten.
      expect(
        checkLiterals('a.dart', "x() => HSegmentOption(value: 'allow');"),
        isEmpty,
      );
      // `title: title` reicht einen Parameter gleichen Namens weiter; die
      // Konstante einer anderen Klasse ist dort verdeckt.
      expect(
        checkLiterals(
          'a.dart',
          "class A {\n  static const String title = 'Welcome';\n}\n"
              'x(String title) => HCard(title: title);',
        ),
        isEmpty,
      );
      // Ein Feld gleichen Namens ist nicht die Konstante.
      expect(
        checkLiterals(
          'a.dart',
          "const title = 'Hello';\nx() => Text(widget.title);",
        ),
        isEmpty,
      );
      // Ein Tastenkürzel, ein Code, eine Methode sind keine Anzeige-Namen.
      expect(
        checkLiterals('a.dart', "x() => f(code: 'SANDBOX_001');"),
        isEmpty,
      );
      // Der Doppelpunkt einer Bedingung ist kein benanntes Argument.
      expect(checkLiterals('a.dart', "x() => f(a ? title : 'x');"), isEmpty);
    });

    test('comments and map keys are not shown text', () {
      expect(checkLiterals('a.dart', "// Text('Hello')\nx() {}"), isEmpty);
      expect(checkLiterals('a.dart', "/* Text('Hello') */ x() {}"), isEmpty);
      expect(
        checkLiterals('a.dart', "x() => <String, String>{'label': 'x'};"),
        isEmpty,
      );
    });

    test('a file with l10n-exempt in its first line is skipped', () {
      expect(
        checkLiterals('g.dart', "// l10n-exempt: gallery\nx() => Text('Hi');"),
        isEmpty,
      );
      expect(
        checkLiterals('g.dart', "\n// l10n-exempt\nx() => Text('Hi');"),
        hasLength(1),
      );
    });
  });

  group('parity', () {
    test('a key missing in de exits 1', () {
      final Map<String, Object?> de = _de()..remove('stateHeld');
      final Directory root = repo(de: de);
      expect(run(root, out: _Sink(), err: _Sink()), 1);
      expect(checksOf(root), contains('parity'));
    });

    test('a key only in de', () {
      final Map<String, Object?> de = _de()..['extra'] = 'Mehr';
      expect(checksOf(repo(de: de)), <String>['parity']);
    });

    test('the plural selector is a use; a renamed placeholder is not', () {
      final Map<String, Object?> de = _de()
        ..['count'] = '{n, plural, =1{Ein Eintrag} other{Einträge}}';
      expect(checksOf(repo(de: de)), isEmpty);
      final Map<String, Object?> renamed = _de()
        ..['count'] = '{m, plural, =1{Ein Eintrag} other{{m} Einträge}}';
      expect(checksOf(repo(de: renamed)), <String>['parity']);
    });

    test('an en key without description', () {
      final Map<String, Object?> en = _en()..remove('@stateHeld');
      expect(checksOf(repo(en: en)), <String>['description']);
    });
  });

  group('placeholdersOf', () {
    test('branch text is not a placeholder, nested names are', () {
      expect(
        placeholdersOf('{count, plural, =1{Einmal} other{{count}×}}'),
        <String>{'count'},
      );
      expect(placeholdersOf('{a} and {b}'), <String>{'a', 'b'});
    });

    test('quoted braces are text', () {
      expect(placeholdersOf("'{literal}' and {x}"), <String>{'x'});
      expect(placeholdersOf("it''s {x}"), <String>{'x'});
    });

    test('malformed messages throw', () {
      expect(() => placeholdersOf('{x'), throwsFormatException);
      expect(() => placeholdersOf('x}'), throwsFormatException);
    });
  });

  group('diagnostic', () {
    test('a code without a Why exits 1', () {
      final Map<String, Object?> en = _en()
        ..remove('diagDEMO001Why')
        ..remove('@diagDEMO001Why');
      final Map<String, Object?> de = _de()..remove('diagDEMO001Why');
      final Directory root = repo(en: en, de: de);
      expect(run(root, out: _Sink(), err: _Sink()), 1);
      expect(checksOf(root), <String>['diagnostic']);
    });

    test('an empty register is a finding, not a pass', () {
      expect(checksOf(repo(codes: 'registry! {}')), <String>['diagnostic']);
    });
  });

  group('glossary', () {
    test('a key that carries another word', () {
      final Map<String, Object?> de = _de()..['stateHeld'] = 'Abgefertigt';
      expect(checksOf(repo(de: de)), <String>['glossary']);
    });

    test('a key missing from both files', () {
      final String glossary = _glossary.replaceFirst('`stateHeld`', '`nope`');
      expect(checksOf(repo(glossary: glossary)), <String>[
        'glossary',
        'glossary',
      ]);
    });

    test('a forbidden word in German', () {
      final Map<String, Object?> de = _de()
        ..['diagDEMO001Why'] = 'Die Anfrage wurde abgefangen.';
      expect(checksOf(repo(de: de)), <String>['glossary']);
    });

    test('a glossary without rows is a finding', () {
      expect(checksOf(repo(glossary: '# Leer\n')), <String>['glossary']);
    });
  });
}

/// Schluckt den Bericht eines Laufs.
class _Sink implements IOSink {
  @override
  Encoding encoding = utf8;

  @override
  void writeln([Object? object = '']) {}

  @override
  void write(Object? object) {}

  @override
  dynamic noSuchMethod(Invocation invocation) => null;
}
