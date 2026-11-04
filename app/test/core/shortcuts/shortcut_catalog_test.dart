// Der Katalog der Tastenkürzel (HUM-420).
//
// Vier Zusagen:
//
// 1. Vollständig: Außerhalb des Katalogs baut kein Code in `lib/` und
//    `packages/ui/lib/` einen Aktivator, auch nicht als Tear-off oder eigene
//    Klasse, nennt keine bestimmte Taste und hat keinen eigenen
//    Tastenhandler. Ausgenommen sind die benannten Konstanten in
//    `intents.dart`, die der Katalog selbst nennt, die Konstruktoren der
//    eigenen Aktivatoren, die `Cmd`-Variante, die `withCommandOnMac` aus
//    einer `Ctrl`-Bindung ableitet, und die Dateien der beiden Listen unten,
//    jede mit ihrem Grund.
// 2. Deckungsgleich: Jede Bindung der Bereiche steht im Katalog unter ihrem
//    Bereich, und jeder Eintrag dieser Bereiche ist gebunden. So kann die
//    Übersicht nicht von dem abweichen, was gebunden ist.
// 3. Eindeutig: Innerhalb eines Bereichs löst keine Taste zwei Einträge aus,
//    auch nicht zwischen „Überall“ und „Terminal“, die eine Map teilen.
// 4. Benannt: Jeder Eintrag hat einen Namen in beiden Sprachen und Tasten,
//    die sich zeigen lassen, unter Linux und auf macOS.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/shortcuts/shortcut_catalog.dart';
import 'package:humanitl/core/shortcuts/shortcut_labels.dart';
import 'package:humanitl/features/editor/editor_intents.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/intercept/intents.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Ein Aufruf, der einen Aktivator baut, auch als Tear-off (`.new`).
final RegExp _construction = RegExp(
  r'\b(SingleActivator|LogicalKeySet|CharacterActivator|'
  r'TerminalZoomActivator|UiScaleActivator|TerminalClipboardActivator|'
  r'TerminalFocusActivator)(\s*\(|\.new\b)',
);

/// Eine eigene Aktivator-Klasse.
final RegExp _ownActivator = RegExp(
  r'\b(extends|implements|with)\s+[\w\s,]*\bShortcutActivator\b',
);

/// Ein Verweis auf eine bestimmte Taste.
final RegExp _namedKey = RegExp(r'\b(Logical|Physical)KeyboardKey\.\w');

/// Ein eigener Tastenhandler.
final RegExp _keyHandler = RegExp(
  r'\bonKeyEvent\s*:|\bonKey\s*:|HardwareKeyboard\.instance\.addHandler',
);

/// Die benannten Konstanten in `intents.dart`.
final RegExp _namedConstant = RegExp(
  r'^const SingleActivator (\w+) = SingleActivator\($',
);

/// Der Konstruktor eines eigenen Aktivators in `intents.dart`.
final RegExp _constructorDeclaration = RegExp(r'^\s+const \w+Activator\(');

const String _catalogue = 'lib/core/shortcuts/shortcut_catalog.dart';
const String _intents = 'lib/core/shortcuts/intents.dart';

/// Dateien, die eine bestimmte Taste nennen dürfen, mit Grund.
///
/// Die Bausteine aus `humanitl_ui` tragen das allgemeine Verhalten jedes
/// solchen Elements, das keiner Handlung der Anwendung gehört; die Übersicht
/// nennt es mit [AppShortcut.closeLayer] nur für die Dialoge der Anwendung.
const Map<String, String> namedKeyAllowlist = <String, String>{
  _catalogue: 'the catalogue itself',
  _intents: 'the triggers of the own activators of the shell',
  'lib/core/shortcuts/shortcut_labels.dart': 'names of keys for display',
  'lib/features/intercept/widgets/queue_row.dart':
      'which modifier is held during a click (Ctrl/Shift-click selection), '
      'a pointer gesture and no shortcut',
  'packages/ui/lib/src/widgets/h_modal.dart': 'generic: Esc closes a modal',
  'packages/ui/lib/src/widgets/h_sheet.dart': 'generic: Esc closes a sheet',
  'packages/ui/lib/src/widgets/h_context_menu.dart':
      'generic: Esc closes a context menu',
  'packages/ui/lib/src/widgets/h_control.dart':
      'generic: the arrows pass through a control to the screen',
  'packages/ui/lib/src/widgets/h_pill.dart':
      'generic: Enter and Space activate a pill',
};

/// Dateien mit einem eigenen Tastenhandler, mit Grund. Jeder davon nennt
/// keine Taste selbst ([namedKeyAllowlist] gilt für ihn nicht), sondern fragt
/// den Katalog oder nimmt jede Taste.
const Map<String, String> keyHandlerAllowlist = <String, String>{
  'lib/features/intercept/intercept_screen.dart':
      'the repeat lock of the decision keys, read from the catalogue',
  'lib/features/settings/widgets/settings_nav.dart':
      'list navigation, read from the catalogue',
  'lib/features/settings/widgets/servers/server_list.dart':
      'list navigation, read from the catalogue',
  'lib/features/settings/categories/models_category.dart':
      'Esc from the detail back to the list, read from the catalogue',
  'lib/features/settings/settings_overlay.dart':
      'type to search: any printable character starts the search',
  'lib/features/sandbox/widgets/terminal_pane.dart':
      'forwards every key to the emulator after asking the catalogue',
  'packages/ui/lib/src/widgets/h_pill.dart':
      'generic: Enter and Space activate a pill',
};

/// [line] ohne Kommentar.
String _code(String line) {
  final int at = line.indexOf('//');
  return at < 0 ? line : line.substring(0, at);
}

/// Die Dart-Quellen in `lib/` und `packages/ui/lib/`, ohne erzeugten Code.
Iterable<File> sources() sync* {
  for (final String root in <String>['lib', 'packages/ui/lib']) {
    for (final FileSystemEntity entity in Directory(
      root,
    ).listSync(recursive: true)) {
      final String path = entity.path;
      if (entity is File &&
          path.endsWith('.dart') &&
          !path.endsWith('.g.dart') &&
          !path.endsWith('.freezed.dart') &&
          !path.contains('/generated/')) {
        yield entity;
      }
    }
  }
}

/// Alles, was eine Taste an der Katalog vorbei bindet oder liest, als
/// `Datei:Zeile`.
List<String> strayActivators() {
  final List<String> stray = <String>[];
  for (final File file in sources()) {
    final String path = file.path;
    final bool intents = path == _intents;
    final List<String> lines = file.readAsLinesSync();
    for (int i = 0; i < lines.length; i++) {
      final String code = _code(lines[i]);
      final String at = '$path:${i + 1}: ${lines[i].trim()}';
      if (_namedKey.hasMatch(code) && !namedKeyAllowlist.containsKey(path)) {
        stray.add(at);
      }
      if (_keyHandler.hasMatch(code) &&
          !keyHandlerAllowlist.containsKey(path)) {
        stray.add(at);
      }
      if (_ownActivator.hasMatch(code) && !intents) {
        stray.add(at);
      }
      if (path == _catalogue || !_construction.hasMatch(code)) {
        continue;
      }
      // Ein Muster, kein Bau: `TerminalZoomActivator(:final ...) =>`.
      if (RegExp(r'Activator\((:|\)\s*=>)').hasMatch(code)) {
        continue;
      }
      if (intents &&
          (_namedConstant.hasMatch(code) ||
              _constructorDeclaration.hasMatch(code) ||
              // Die `Cmd`-Variante in `withCommandOnMac`.
              (i > 0 && lines[i - 1].contains('case final SingleActivator')))) {
        continue;
      }
      // Die allgemeinen Bindungen der Bausteine (Esc, Pfeile).
      if (namedKeyAllowlist.containsKey(path) &&
          path.startsWith('packages/ui/')) {
        continue;
      }
      stray.add(at);
    }
  }
  return stray;
}

/// Eine Taste mit ihren Modifikatoren, für den Vergleich zweier Bindungen.
String chordId(ShortcutActivator activator) => switch (activator) {
  final SingleActivator a =>
    '${a.trigger.keyId}/c${a.control}/s${a.shift}/a${a.alt}/m${a.meta}',
  _ => '${activator.runtimeType}:${activator.debugDescribeKeys()}',
};

/// Die Bereiche, deren Bindung eine Map ist, die ein Test aufrufen kann; die
/// übrigen (Einstellungen, Palette, Dialoge) binden ihre Einträge in Widgets
/// und sind über Test 1 an den Katalog gehalten.
final Map<String, (List<ShortcutScope>, Map<ShortcutActivator, Intent>)> _maps =
    <String, (List<ShortcutScope>, Map<ShortcutActivator, Intent>)>{
      'shell': (
        <ShortcutScope>[ShortcutScope.global, ShortcutScope.terminal],
        shellShortcuts(),
      ),
      'intercept': (<ShortcutScope>[ShortcutScope.queue], interceptShortcuts()),
      'findings pause': (
        <ShortcutScope>[ShortcutScope.findingsPause],
        findingsPauseShortcuts(),
      ),
      'editor': (<ShortcutScope>[ShortcutScope.editor], editorShortcuts()),
      'history': (<ShortcutScope>[ShortcutScope.history], historyShortcuts()),
      'rule list': (<ShortcutScope>[ShortcutScope.rules], ruleListShortcuts()),
    };

/// Die Einträge des Terminals, die nicht in der Map der Shell stehen, weil
/// `TerminalPane` sie selbst fragt (HUM-313, HUM-353).
const Set<AppShortcut> askedByTheTerminal = <AppShortcut>{
  AppShortcut.terminalCopy,
  AppShortcut.terminalCopySelection,
  AppShortcut.terminalPaste,
};

void main() {
  final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
  final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

  test('catalogue_complete: no activator is built outside the catalogue', () {
    expect(strayActivators(), isEmpty);
  });

  test('every exception of the completeness scan is still needed', () {
    for (final (Map<String, String> list, RegExp pattern)
        in <(Map<String, String>, RegExp)>[
          (namedKeyAllowlist, _namedKey),
          (keyHandlerAllowlist, _keyHandler),
        ]) {
      for (final String path in list.keys) {
        expect(
          File(path).readAsLinesSync().map(_code).any(pattern.hasMatch),
          isTrue,
          reason: '$path still needs its exception (${list[path]})',
        );
      }
    }
  });

  test('the named activators of intents.dart are entries of the catalogue', () {
    final List<String> names = <String>[
      for (final String line in File(
        'lib/core/shortcuts/intents.dart',
      ).readAsLinesSync())
        if (_namedConstant.firstMatch(line) case final RegExpMatch m)
          m.group(1)!,
    ];
    expect(names, isNotEmpty);
    final String catalogue = File('lib/core/shortcuts/shortcut_catalog.dart')
        .readAsStringSync();
    for (final String name in names) {
      expect(
        RegExp('\\b$name\\b').hasMatch(catalogue),
        isTrue,
        reason: '$name is listed in the catalogue',
      );
    }
  });

  for (final MapEntry<
        String,
        (List<ShortcutScope>, Map<ShortcutActivator, Intent>)
      >
      map
      in _maps.entries) {
    test('catalogue_matches_binding: ${map.key}', () {
      final (List<ShortcutScope> scopes, Map<ShortcutActivator, Intent> bound) =
          map.value;
      final Set<ShortcutActivator> catalogued = <ShortcutActivator>{
        for (final ShortcutScope scope in scopes)
          for (final AppShortcut shortcut in AppShortcut.of(scope))
            if (!askedByTheTerminal.contains(shortcut)) ...shortcut.activators,
      };
      expect(
        bound.keys.map(chordId).toSet(),
        catalogued.map(chordId).toSet(),
        reason: 'the ${map.key} bindings are exactly its catalogue entries',
      );
      // Dieselben Objekte, nicht nur dieselben Tasten: Die Bindung liest den
      // Katalog und schreibt nichts ab.
      for (final ShortcutActivator activator in bound.keys) {
        expect(
          catalogued.any((ShortcutActivator c) => identical(c, activator)),
          isTrue,
          reason: '${activator.debugDescribeKeys()} comes from the catalogue',
        );
      }
    });
  }

  test('the terminal asks the catalogue for its own chords', () {
    expect(AppShortcut.terminalCopy.single, isA<TerminalClipboardActivator>());
    expect(AppShortcut.terminalPaste.single, isA<TerminalClipboardActivator>());
    expect(
      chordId(AppShortcut.terminalCopySelection.single),
      chordId(const SingleActivator(LogicalKeyboardKey.keyC, control: true)),
    );
  });

  test('no_duplicate_key_in_scope', () {
    final List<List<ShortcutScope>> spaces = <List<ShortcutScope>>[
      for (final ShortcutScope scope in ShortcutScope.values)
        <ShortcutScope>[scope],
      // Eine Map für beide (`shellShortcuts`).
      <ShortcutScope>[ShortcutScope.global, ShortcutScope.terminal],
    ];
    for (final List<ShortcutScope> space in spaces) {
      final Map<String, AppShortcut> seen = <String, AppShortcut>{};
      for (final ShortcutScope scope in space) {
        for (final AppShortcut shortcut in AppShortcut.of(scope)) {
          for (final ShortcutActivator activator in shortcut.activators) {
            final String id = chordId(activator);
            expect(
              seen[id],
              anyOf(isNull, shortcut),
              reason: '$id is bound twice in $space',
            );
            seen[id] = shortcut;
          }
        }
      }
    }
  });

  test('every entry has a name in both languages, unique in its scope', () {
    for (final AppLocalizations l10n in <AppLocalizations>[en, de]) {
      for (final ShortcutScope scope in ShortcutScope.values) {
        expect(scope.title(l10n), isNotEmpty);
        final List<String> labels = <String>[
          for (final AppShortcut shortcut in AppShortcut.of(scope))
            shortcut.label(l10n),
        ];
        expect(labels, isNotEmpty, reason: '$scope has entries');
        expect(labels.every((String l) => l.isNotEmpty), isTrue);
        expect(labels.toSet(), hasLength(labels.length), reason: '$labels');
      }
    }
  });

  group('the keys as shown', () {
    test('Linux names the modifiers as printed on the keyboard', () {
      String text(AppShortcut s, AppLocalizations l10n) =>
          shortcutText(s, l10n, platform: TargetPlatform.linux);
      expect(text(AppShortcut.palette, en), 'Ctrl+K');
      expect(text(AppShortcut.palette, de), 'Strg+K');
      expect(text(AppShortcut.toggleRail, de), 'Strg+Umschalt+0');
      expect(text(AppShortcut.navigate, en), 'Ctrl+1–6');
      expect(text(AppShortcut.allow, en), 'Enter / A');
      expect(text(AppShortcut.blockAnywhere, en), 'Ctrl+Enter / Ctrl+L');
      expect(text(AppShortcut.terminalZoomIn, en), 'Ctrl+Alt+Plus');
      expect(text(AppShortcut.scaleReset, en), 'Ctrl+0');
      expect(text(AppShortcut.terminalCopy, en), 'Ctrl+Shift+C');
      expect(text(AppShortcut.rememberTarget, en), 'Shift+1–4');
      expect(text(AppShortcut.ruleUp, en), 'Alt+↑');
      expect(text(AppShortcut.editorNextFinding, en), 'F3');
      expect(text(AppShortcut.terminalNextTab, de), 'Strg+Bild ↓');
      expect(
        shortcutChords(
          AppShortcut.terminalFullWindow,
          en,
          platform: TargetPlatform.linux,
        ),
        <List<String>>[
          <String>['Ctrl', 'Shift', 'Enter'],
        ],
        reason: 'the Enter of the number block is not listed apart',
      );
    });

    test('macOS shows ⌘ where Ctrl is bound as Cmd as well', () {
      String text(AppShortcut s) =>
          shortcutText(s, en, platform: TargetPlatform.macOS);
      expect(text(AppShortcut.palette), '⌘K');
      expect(text(AppShortcut.toggleRail), '⇧⌘0');
      expect(text(AppShortcut.allowAnywhere), '⌘F');
      expect(text(AppShortcut.scaleIn), '⌘+');
      expect(text(AppShortcut.terminalCopy), '⌘C');
      // Nicht über `withCommandOnMac` gebunden: dort bleibt es `Ctrl`.
      expect(text(AppShortcut.settingsSearch), '⌃F');
      expect(text(AppShortcut.terminalZoomIn), '⌃⌥+');
      expect(text(AppShortcut.terminalFocus), '⌃`');
      expect(
        AppShortcut.terminalCopySelection.appliesOn(TargetPlatform.macOS),
        isFalse,
      );
    });

    test('every entry can be shown on every desktop', () {
      for (final TargetPlatform platform in <TargetPlatform>[
        TargetPlatform.linux,
        TargetPlatform.macOS,
      ]) {
        for (final AppShortcut shortcut in AppShortcut.values) {
          final List<List<String>> chords = shortcutChords(
            shortcut,
            de,
            platform: platform,
          );
          expect(chords, isNotEmpty, reason: '$shortcut on $platform');
          expect(
            chords.every(
              (List<String> c) => c.every((String k) => k.isNotEmpty),
            ),
            isTrue,
          );
        }
      }
    });

    test('an entry bound through withCommandOnMac really answers to Cmd', () {
      debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
      addTearDown(() => debugDefaultTargetPlatformOverride = null);
      final Map<ShortcutScope, Map<ShortcutActivator, Intent>> onMac =
          <ShortcutScope, Map<ShortcutActivator, Intent>>{
            ShortcutScope.global: shellShortcuts(),
            ShortcutScope.terminal: shellShortcuts(),
            ShortcutScope.queue: interceptShortcuts(),
            ShortcutScope.editor: editorShortcuts(),
          };
      for (final MapEntry<ShortcutScope, Map<ShortcutActivator, Intent>> map
          in onMac.entries) {
        final Set<String> bound = map.value.keys.map(chordId).toSet();
        for (final AppShortcut shortcut in AppShortcut.of(map.key)) {
          if (askedByTheTerminal.contains(shortcut)) {
            continue;
          }
          for (final ShortcutActivator activator in shortcut.activators) {
            if (activator is SingleActivator && activator.control) {
              expect(shortcut.commandOnMac, isTrue);
              expect(
                bound,
                contains(
                  chordId(
                    SingleActivator(
                      activator.trigger,
                      meta: true,
                      shift: activator.shift,
                      alt: activator.alt,
                    ),
                  ),
                ),
                reason: '$shortcut answers to Cmd on macOS',
              );
            }
          }
        }
      }
    });
  });
}
