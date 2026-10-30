// Die Griffe zum Kopieren und Einfügen im Terminal (HUM-313).
//
// Zwei Zusagen: Der Aktivator nimmt genau `Ctrl+Shift+C` und `Ctrl+Shift+V`
// unter Linux, `Cmd+C` und `Cmd+V` auf Apple-Plattformen, und keine andere
// Bindung der Anwendung feuert auf diese Griffe. Die zweite wird mit echten
// Tastenereignissen durch jede Tabelle der Anwendung geprüft, wie in
// `shortcuts_map_to_intents_test.dart`: Ein Blick in die Maps übersähe einen
// eigenen Aktivator, der mehr annimmt, als sein Auslöser sagt.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/features/editor/editor_intents.dart';
import 'package:humanitl/features/history/history_screen.dart';
import 'package:humanitl/features/intercept/intents.dart';
import 'package:humanitl/features/rules/widgets/rule_row.dart';

/// Hält die Modifikatoren [held] und fragt, ob [action] die Taste [key]
/// annimmt.
Future<bool> _accepts(
  WidgetTester tester,
  TerminalClipboard action,
  LogicalKeyboardKey key,
  List<LogicalKeyboardKey> held,
) async {
  for (final LogicalKeyboardKey modifier in held) {
    await tester.sendKeyDownEvent(modifier, platform: 'linux');
  }
  final bool accepted = TerminalClipboardActivator(action).accepts(
    KeyDownEvent(
      physicalKey: key == LogicalKeyboardKey.keyC
          ? PhysicalKeyboardKey.keyC
          : PhysicalKeyboardKey.keyV,
      logicalKey: key,
      timeStamp: Duration.zero,
    ),
    HardwareKeyboard.instance,
  );
  for (final LogicalKeyboardKey modifier in held.reversed) {
    await tester.sendKeyUpEvent(modifier, platform: 'linux');
  }
  return accepted;
}

const LogicalKeyboardKey _ctrl = LogicalKeyboardKey.controlLeft;
const LogicalKeyboardKey _shift = LogicalKeyboardKey.shiftLeft;
const LogicalKeyboardKey _alt = LogicalKeyboardKey.altLeft;
const LogicalKeyboardKey _meta = LogicalKeyboardKey.metaLeft;

/// Schreibt jeden Intent mit, der bei ihr ankommt.
class _Recorder extends Action<Intent> {
  _Recorder(this.log);

  final List<String> log;

  @override
  Object? invoke(Intent intent) {
    log.add('${intent.runtimeType}');
    return null;
  }
}

void main() {
  testWidgets('the_linux_chords_are_ctrl_shift_c_and_v_and_nothing_else', (
    WidgetTester tester,
  ) async {
    for (final (TerminalClipboard action, LogicalKeyboardKey key)
        in <(TerminalClipboard, LogicalKeyboardKey)>[
          (TerminalClipboard.copy, LogicalKeyboardKey.keyC),
          (TerminalClipboard.paste, LogicalKeyboardKey.keyV),
        ]) {
      expect(
        await _accepts(tester, action, key, <LogicalKeyboardKey>[
          _ctrl,
          _shift,
        ]),
        isTrue,
        reason: '${action.name}: Ctrl+Shift',
      );
      // Ohne `Shift` gehört die Taste dem Agenten: `0x03`, `0x16`.
      expect(
        await _accepts(tester, action, key, <LogicalKeyboardKey>[_ctrl]),
        isFalse,
        reason: '${action.name}: plain Ctrl stays with the agent',
      );
      expect(
        await _accepts(tester, action, key, <LogicalKeyboardKey>[
          _ctrl,
          _shift,
          _alt,
        ]),
        isFalse,
        reason: '${action.name}: with Alt it is another chord',
      );
      expect(
        await _accepts(tester, action, key, <LogicalKeyboardKey>[_meta]),
        isFalse,
        reason: '${action.name}: Cmd only on Apple platforms',
      );
    }
    // Die andere Taste nimmt keiner der beiden.
    expect(
      await _accepts(
        tester,
        TerminalClipboard.copy,
        LogicalKeyboardKey.keyV,
        <LogicalKeyboardKey>[_ctrl, _shift],
      ),
      isFalse,
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));

  testWidgets('the_apple_chords_are_cmd_c_and_v', (WidgetTester tester) async {
    expect(
      await _accepts(
        tester,
        TerminalClipboard.copy,
        LogicalKeyboardKey.keyC,
        <LogicalKeyboardKey>[_meta],
      ),
      isTrue,
    );
    expect(
      await _accepts(
        tester,
        TerminalClipboard.paste,
        LogicalKeyboardKey.keyV,
        <LogicalKeyboardKey>[_ctrl, _shift],
      ),
      isFalse,
      reason: 'Ctrl reaches the program on a Mac',
    );
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));

  testWidgets('no_binding_of_the_application_fires_on_the_clipboard_chords', (
    WidgetTester tester,
  ) async {
    for (final (String name, Map<ShortcutActivator, Intent> bindings)
        in <(String, Map<ShortcutActivator, Intent>)>[
          ('shell', shellShortcuts()),
          ('intercept', interceptShortcuts()),
          ('findings pause', findingsPauseShortcuts()),
          ('editor', editorShortcuts()),
          ('history', historyShortcuts()),
          ('rule list', ruleListShortcuts()),
        ]) {
      final List<String> log = <String>[];
      final _Recorder recorder = _Recorder(log);
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
      for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
        LogicalKeyboardKey.keyC,
        LogicalKeyboardKey.keyV,
      ]) {
        await tester.sendKeyDownEvent(_ctrl, platform: 'linux');
        await tester.sendKeyDownEvent(_shift, platform: 'linux');
        await tester.sendKeyEvent(key, platform: 'linux');
        await tester.sendKeyUpEvent(_shift, platform: 'linux');
        await tester.sendKeyUpEvent(_ctrl, platform: 'linux');
        await tester.pump();
      }
      expect(log, isEmpty, reason: 'Ctrl+Shift+C/V are free in $name');
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
