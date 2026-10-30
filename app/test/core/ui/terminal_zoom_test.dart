// Die Schriftgröße des Terminals als Notiz im Kern, und die Griffe, die sie
// ändern (HUM-301).
//
// Die Vorgabe ist die Größe, die das Terminal vor diesem Issue hatte; ein
// Wert aus der Zustandsdatei, den niemand so geschrieben haben kann, fällt auf
// die Vorgabe oder die nächste Grenze zurück. Die Aktivatoren verlangen genau
// `Ctrl`, und nur der Zoom wiederholt sich beim Halten.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/terminal_zoom.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';

/// Ein Container über einer Zustandsdatei mit [content], oder ohne Datei.
ProviderContainer containerWith(Object? content) {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-zoom');
  addTearDown(() => dir.deleteSync(recursive: true));
  final String path = '${dir.path}/${UiStateFile.fileName}';
  if (content != null) {
    File(path).writeAsStringSync(jsonEncode(content));
  }
  final ProviderContainer container = ProviderContainer(
    overrides: [uiStateFileProvider.overrideWithValue(UiStateFile(path))],
  );
  addTearDown(container.dispose);
  return container;
}

/// Was [activator] zu [key] sagt, während [held] gedrückt sind.
Future<bool> accepts(
  WidgetTester tester,
  ShortcutActivator activator,
  LogicalKeyboardKey key, {
  List<LogicalKeyboardKey> held = const <LogicalKeyboardKey>[
    LogicalKeyboardKey.controlLeft,
  ],
  bool repeat = false,
}) async {
  bool? answer;
  await tester.pumpWidget(
    Focus(
      autofocus: true,
      onKeyEvent: (FocusNode _, KeyEvent event) {
        if (event.logicalKey == key &&
            (event is KeyDownEvent || event is KeyRepeatEvent)) {
          answer = activator.accepts(event, HardwareKeyboard.instance);
        }
        return KeyEventResult.handled;
      },
      child: const SizedBox.shrink(),
    ),
  );
  await tester.pump();
  for (final LogicalKeyboardKey modifier in held) {
    await tester.sendKeyDownEvent(modifier);
  }
  if (repeat) {
    await tester.sendKeyDownEvent(key);
    answer = null;
    await tester.sendKeyRepeatEvent(key);
    await tester.sendKeyUpEvent(key);
  } else {
    await tester.sendKeyEvent(key);
  }
  for (final LogicalKeyboardKey modifier in held.reversed) {
    await tester.sendKeyUpEvent(modifier);
  }
  return answer ?? false;
}

void main() {
  test('the default is the size of mono13', () {
    for (final HThemeMode mode in HThemeMode.values) {
      for (final Brightness brightness in Brightness.values) {
        expect(
          mode.resolve(brightness).typography.mono13.fontSize,
          terminalFontSizeDefault,
        );
      }
    }
  });

  test('without a file the size is the default', () {
    expect(containerWith(null).read(terminalFontSizeProvider), 13);
  });

  test('a stored size is read, an impossible one falls back', () {
    expect(
      containerWith(<String, Object?>{terminalFontSizeKey: 17})
          .read(terminalFontSizeProvider),
      17,
    );
    expect(
      containerWith(<String, Object?>{terminalFontSizeKey: 99})
          .read(terminalFontSizeProvider),
      terminalFontSizeMax,
    );
    expect(
      containerWith(<String, Object?>{terminalFontSizeKey: 2})
          .read(terminalFontSizeProvider),
      terminalFontSizeMin,
    );
    expect(
      containerWith(<String, Object?>{terminalFontSizeKey: 'huge'})
          .read(terminalFontSizeProvider),
      terminalFontSizeDefault,
    );
  });

  test('a change is written down, the default as no choice', () {
    final ProviderContainer container = containerWith(null);
    final TerminalFontSize size = container.read(
      terminalFontSizeProvider.notifier,
    );
    final UiStateFile file = container.read(uiStateFileProvider);
    size.zoomIn();
    expect(file.read()[terminalFontSizeKey], 14);
    size.reset();
    expect(file.read().containsKey(terminalFontSizeKey), isTrue);
    expect(file.read()[terminalFontSizeKey], isNull);
  });

  group('the activators', () {
    const TerminalZoomActivator zoomIn = TerminalZoomActivator(
      TerminalZoom.zoomIn,
    );
    const TerminalZoomActivator zoomOut = TerminalZoomActivator(
      TerminalZoom.zoomOut,
    );
    const TerminalZoomActivator reset = TerminalZoomActivator(
      TerminalZoom.reset,
    );

    testWidgets('need Ctrl and nothing but Ctrl and Shift', (
      WidgetTester tester,
    ) async {
      expect(await accepts(tester, zoomIn, LogicalKeyboardKey.equal), isTrue);
      expect(
        await accepts(
          tester,
          zoomIn,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[],
        ),
        isFalse,
        reason: 'a plain = belongs to the agent',
      );
      expect(
        await accepts(
          tester,
          zoomIn,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.altLeft,
          ],
        ),
        isFalse,
      );
      expect(
        await accepts(
          tester,
          zoomOut,
          LogicalKeyboardKey.minus,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.metaLeft,
          ],
        ),
        isFalse,
      );
      expect(
        await accepts(tester, reset, LogicalKeyboardKey.digit0),
        isFalse,
        reason: 'Ctrl+0 is the rail',
      );
      expect(
        await accepts(
          tester,
          reset,
          LogicalKeyboardKey.numpad0,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.shiftLeft,
          ],
        ),
        isFalse,
      );
    });

    testWidgets('zoom repeats while held, the reset does not', (
      WidgetTester tester,
    ) async {
      expect(
        await accepts(tester, zoomIn, LogicalKeyboardKey.equal, repeat: true),
        isTrue,
      );
      expect(
        await accepts(tester, zoomOut, LogicalKeyboardKey.minus, repeat: true),
        isTrue,
      );
      expect(
        await accepts(tester, reset, LogicalKeyboardKey.numpad0, repeat: true),
        isFalse,
      );
    });
  });
}
