// Jede Tastenkombination löst genau ihren Intent aus (HUM-054,
// `shortcuts_map_to_intents`).
//
// Die Tabelle unten ist die Liste aus `backlog/CONVENTIONS.md` 3.9, ergänzt um
// die Bindungen, die später dazukamen (Gruppen, Notiz, Pause, Editor,
// Einstellungen, Regelliste). Geprüft wird mit echten Tastenereignissen durch ein
// `Shortcuts` mit der Bindung der App, nicht durch einen Blick in die Map:
// Ein Modifikator, der fehlt oder zu viel ist, ein Intent mit falschem
// Parameter oder zwei Bindungen auf derselben Taste fallen nur so auf.
//
// Zwei Richtungen: Jede Zeile der Tabelle feuert genau ihren Intent, und jede
// Bindung der App hat eine Zeile. Eine neue Taste ohne Zeile ist rot, damit
// sie nicht ungeprüft dazukommt.
//
// Linux als Plattform: Ohne Variante spielt `flutter test` Android, und die
// Tasten dieses Produkts sind die eines Linux-Desktops (`Ctrl`, nicht `Cmd`).

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/features/editor/editor_intents.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/intercept/intents.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';

/// Ein Intent als Text, mit seinem Parameter.
String describe(Intent intent) => switch (intent) {
  AllowIntent(:final bool chord) => 'Allow(chord: $chord)',
  BlockIntent(:final bool chord) => 'Block(chord: $chord)',
  NavIntent(:final int index) => 'Nav($index)',
  RememberDurationIntent(:final int index) => 'RememberDuration($index)',
  RememberTargetIntent(:final int index) => 'RememberTarget($index)',
  MoveRuleIntent(:final int delta) => 'MoveRule($delta)',
  ToggleGroupIntent(:final bool open) => 'ToggleGroup(open: $open)',
  _ => '${intent.runtimeType}',
};

/// Eine Zeile der Tabelle: welche Taste, mit welchen Modifikatoren, und was
/// sie auslösen muss.
typedef KeyRow = ({
  LogicalKeyboardKey key,
  bool control,
  bool shift,
  bool alt,
  String intent,
});

KeyRow plain(LogicalKeyboardKey key, String intent) =>
    (key: key, control: false, shift: false, alt: false, intent: intent);

KeyRow ctrl(LogicalKeyboardKey key, String intent) =>
    (key: key, control: true, shift: false, alt: false, intent: intent);

KeyRow shift(LogicalKeyboardKey key, String intent) =>
    (key: key, control: false, shift: true, alt: false, intent: intent);

KeyRow ctrlShift(LogicalKeyboardKey key, String intent) =>
    (key: key, control: true, shift: true, alt: false, intent: intent);

KeyRow alt(LogicalKeyboardKey key, String intent) =>
    (key: key, control: false, shift: false, alt: true, intent: intent);

/// Die Shell: `Ctrl+1..6`, `Ctrl+K`, `Ctrl+,`, `Ctrl+0` (HUM-254).
final List<KeyRow> shellTable = <KeyRow>[
  ctrl(LogicalKeyboardKey.digit1, 'Nav(0)'),
  ctrl(LogicalKeyboardKey.digit2, 'Nav(1)'),
  ctrl(LogicalKeyboardKey.digit3, 'Nav(2)'),
  ctrl(LogicalKeyboardKey.digit4, 'Nav(3)'),
  ctrl(LogicalKeyboardKey.digit5, 'Nav(4)'),
  ctrl(LogicalKeyboardKey.digit6, 'Nav(5)'),
  ctrl(LogicalKeyboardKey.keyK, 'PaletteIntent'),
  ctrl(LogicalKeyboardKey.comma, 'SettingsIntent'),
  ctrl(LogicalKeyboardKey.digit0, 'ToggleRailIntent'),
  ctrlShift(LogicalKeyboardKey.enter, 'ToggleTerminalFullWindowIntent'),
  ctrlShift(LogicalKeyboardKey.numpadEnter, 'ToggleTerminalFullWindowIntent'),
  ctrl(LogicalKeyboardKey.backquote, 'FocusTerminalIntent'),
];

/// Die Warteschlange (CONVENTIONS 3.9 und die Bindungen von HUM-029, HUM-072).
final List<KeyRow> interceptTable = <KeyRow>[
  plain(LogicalKeyboardKey.enter, 'Allow(chord: false)'),
  plain(LogicalKeyboardKey.numpadEnter, 'Allow(chord: false)'),
  plain(LogicalKeyboardKey.keyA, 'Allow(chord: false)'),
  ctrl(LogicalKeyboardKey.keyF, 'Allow(chord: true)'),
  plain(LogicalKeyboardKey.keyB, 'Block(chord: false)'),
  ctrl(LogicalKeyboardKey.enter, 'Block(chord: true)'),
  ctrl(LogicalKeyboardKey.keyL, 'Block(chord: true)'),
  plain(LogicalKeyboardKey.keyJ, 'NextFlowIntent'),
  plain(LogicalKeyboardKey.arrowDown, 'NextFlowIntent'),
  plain(LogicalKeyboardKey.keyK, 'PrevFlowIntent'),
  plain(LogicalKeyboardKey.arrowUp, 'PrevFlowIntent'),
  shift(LogicalKeyboardKey.enter, 'OpenRememberIntent'),
  plain(LogicalKeyboardKey.keyN, 'NoteIntent'),
  plain(LogicalKeyboardKey.keyE, 'EditIntent'),
  plain(LogicalKeyboardKey.escape, 'CloseCoachMarkIntent'),
  ctrlShift(LogicalKeyboardKey.keyF, 'AllowGroupIntent'),
  ctrlShift(LogicalKeyboardKey.keyL, 'BlockGroupIntent'),
  ctrl(LogicalKeyboardKey.keyA, 'SelectGroupIntent'),
  shift(LogicalKeyboardKey.keyJ, 'MergeArrivalsIntent'),
  plain(LogicalKeyboardKey.arrowRight, 'ToggleGroup(open: true)'),
  plain(LogicalKeyboardKey.arrowLeft, 'ToggleGroup(open: false)'),
  plain(LogicalKeyboardKey.digit1, 'RememberDuration(0)'),
  plain(LogicalKeyboardKey.digit2, 'RememberDuration(1)'),
  plain(LogicalKeyboardKey.digit3, 'RememberDuration(2)'),
  plain(LogicalKeyboardKey.digit4, 'RememberDuration(3)'),
  shift(LogicalKeyboardKey.digit1, 'RememberTarget(0)'),
  shift(LogicalKeyboardKey.digit2, 'RememberTarget(1)'),
  shift(LogicalKeyboardKey.digit3, 'RememberTarget(2)'),
  shift(LogicalKeyboardKey.digit4, 'RememberTarget(3)'),
];

/// Die Pause mit offenen Funden (HUM-049).
final List<KeyRow> pauseTable = <KeyRow>[
  plain(LogicalKeyboardKey.keyS, 'SendAnywayIntent'),
  plain(LogicalKeyboardKey.keyP, 'PseudonymizeIntent'),
  plain(LogicalKeyboardKey.escape, 'CloseFindingsPauseIntent'),
];

/// Der Editor (HUM-047).
final List<KeyRow> editorTable = <KeyRow>[
  ctrl(LogicalKeyboardKey.keyR, 'PseudonymizeSelectionIntent'),
  ctrl(LogicalKeyboardKey.enter, 'SendEditedIntent'),
  plain(LogicalKeyboardKey.escape, 'CloseEditorIntent'),
  plain(LogicalKeyboardKey.f3, 'NextFindingIntent'),
];

/// Die History (HUM-032).
final List<KeyRow> historyTable = <KeyRow>[
  plain(LogicalKeyboardKey.enter, 'OpenFlowIntent'),
  plain(LogicalKeyboardKey.numpadEnter, 'OpenFlowIntent'),
  plain(LogicalKeyboardKey.slash, 'FilterIntent'),
  plain(LogicalKeyboardKey.keyJ, 'NextFlowIntent'),
  plain(LogicalKeyboardKey.arrowDown, 'NextFlowIntent'),
  plain(LogicalKeyboardKey.keyK, 'PrevFlowIntent'),
  plain(LogicalKeyboardKey.arrowUp, 'PrevFlowIntent'),
];

/// Die Regelliste (HUM-033): eine Regel in der Kette verschieben.
final List<KeyRow> rulesTable = <KeyRow>[
  alt(LogicalKeyboardKey.arrowUp, 'MoveRule(-1)'),
  alt(LogicalKeyboardKey.arrowDown, 'MoveRule(1)'),
];

/// Schreibt jeden Intent mit, der bei ihr ankommt.
class Recorder extends Action<Intent> {
  /// Schreibt nach [log].
  Recorder(this.log);

  /// Was ankam, in der Reihenfolge.
  final List<String> log;

  @override
  Object? invoke(Intent intent) {
    log.add(describe(intent));
    return null;
  }
}

/// Ein Fokus unter [bindings], mit einem [Recorder] für jeden ihrer Intents.
Future<List<String>> pumpBindings(
  WidgetTester tester,
  Map<ShortcutActivator, Intent> bindings,
) async {
  final List<String> log = <String>[];
  final Recorder recorder = Recorder(log);
  await tester.pumpWidget(
    Shortcuts(
      shortcuts: bindings,
      child: Actions(
        actions: <Type, Action<Intent>>{
          for (final Intent intent in bindings.values)
            intent.runtimeType: recorder,
        },
        child: const Focus(autofocus: true, child: SizedBox.shrink()),
      ),
    ),
  );
  await tester.pump();
  return log;
}

/// Drückt [row] mit seinen Modifikatoren.
Future<void> press(WidgetTester tester, KeyRow row) async {
  if (row.control) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  }
  if (row.shift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  if (row.alt) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.altLeft);
  }
  await tester.sendKeyEvent(row.key);
  if (row.alt) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.altLeft);
  }
  if (row.shift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  if (row.control) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  }
  await tester.pump();
}

/// Die Zeile, die zu [activator] gehört, als Text: zum Vergleich der beiden
/// Richtungen.
String activatorLabel(ShortcutActivator activator) {
  // Die Tastatur ans Terminal fragt nach der physischen Taste links der 1
  // (HUM-303); ihre Zeile steht unter dem Namen dieser Taste auf US-Belegung.
  if (activator is TerminalFocusActivator) {
    return 'Ctrl+${LogicalKeyboardKey.backquote.keyLabel}';
  }
  final SingleActivator single = activator as SingleActivator;
  return '${single.control ? 'Ctrl+' : ''}${single.shift ? 'Shift+' : ''}'
      '${single.alt ? 'Alt+' : ''}${single.trigger.keyLabel}';
}

String rowLabel(KeyRow row) =>
    '${row.control ? 'Ctrl+' : ''}${row.shift ? 'Shift+' : ''}'
    '${row.alt ? 'Alt+' : ''}${row.key.keyLabel}';

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  for (final (
        String name,
        Map<ShortcutActivator, Intent> Function() bind,
        List<KeyRow> table,
      )
      in <(String, Map<ShortcutActivator, Intent> Function(), List<KeyRow>)>[
        ('shell', shellShortcuts, shellTable),
        ('intercept', interceptShortcuts, interceptTable),
        ('findings pause', findingsPauseShortcuts, pauseTable),
        ('editor', editorShortcuts, editorTable),
        ('history', historyShortcuts, historyTable),
        ('rule list', ruleListShortcuts, rulesTable),
      ]) {
    testWidgets('shortcuts_map_to_intents: $name', (WidgetTester tester) async {
      final Map<ShortcutActivator, Intent> bindings = bind();
      final List<String> log = await pumpBindings(tester, bindings);

      for (final KeyRow row in table) {
        log.clear();
        await press(tester, row);
        expect(log, <String>[
          row.intent,
        ], reason: '${rowLabel(row)} in $name fires exactly ${row.intent}');
      }

      // Die andere Richtung: keine Bindung ohne Zeile.
      expect(
        <String>{
          for (final MapEntry<ShortcutActivator, Intent> entry
              in bindings.entries)
            '${activatorLabel(entry.key)} -> ${describe(entry.value)}',
        },
        <String>{
          for (final KeyRow row in table) '${rowLabel(row)} -> ${row.intent}',
        },
        reason: 'every binding of $name has a row in this table',
      );
    }, variant: linux);
  }
}
