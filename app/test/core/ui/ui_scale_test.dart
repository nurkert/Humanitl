// Die Skalierung der ganzen Anwendung als Notiz im Kern, ihre Griffe und das
// Render-Objekt, das sie anlegt (HUM-356).
//
// Gerechnet wird in ganzen Prozent. Die Grenze folgt aus dem Fenster und
// nimmt nur Vergrößerung zurück; die Tasten rasten auf Zehnern ein und
// rechnen von der Skalierung, die gilt. Das Render-Objekt legt sein Kind in
// der geteilten Größe an und trifft dort, wohin es malt.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/ui_scale.dart';
import 'package:humanitl/core/ui/ui_scale_host.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';

/// Ein Container über einer Zustandsdatei mit [content], oder ohne Datei.
(ProviderContainer, String) containerWith(Object? content) {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-scale');
  addTearDown(() => dir.deleteSync(recursive: true));
  final String path = '${dir.path}/${UiStateFile.fileName}';
  if (content != null) {
    File(path).writeAsStringSync(jsonEncode(content));
  }
  final ProviderContainer container = ProviderContainer(
    overrides: [uiStateFileProvider.overrideWithValue(UiStateFile(path))],
  );
  addTearDown(container.dispose);
  return (container, path);
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
  group('the limit of the window', () {
    test('is the largest whole percent that keeps 1100 x 700', () {
      expect(appMinimumLayoutSize, const Size(1100, 700));
      // 1080 / 700 = 1,542...; die Höhe bestimmt.
      expect(uiScaleLimitFor(const Size(1920, 1080)), 154);
      expect(uiScaleLimitFor(const Size(1280, 800)), 114);
      expect(uiScaleLimitFor(const Size(1650, 1050)), 150);
      expect(uiScaleLimitFor(const Size(2560, 1600)), 228);
    });

    test('stops at 300 and never goes below 100', () {
      expect(uiScaleLimitFor(const Size(3840, 2160)), 300);
      expect(uiScaleLimitFor(const Size(7680, 4320)), 300);
      expect(uiScaleLimitFor(const Size(800, 600)), 100);
      expect(uiScaleLimitFor(Size.infinite), 300);
    });

    test('takes back only what is above 100', () {
      const Size small = Size(1280, 800);
      expect(effectiveUiScale(200, small), 114);
      expect(effectiveUiScale(110, small), 110);
      expect(effectiveUiScale(90, small), 90);
      expect(effectiveUiScale(50, const Size(800, 600)), 50);
      expect(effectiveUiScale(200, const Size(800, 600)), 100);
      expect(effectiveUiScale(300, const Size(3840, 2160)), 300);
    });
  });

  group('the choice', () {
    test('starts at 100 and reads a stored value', () {
      expect(containerWith(null).$1.read(uiScaleProvider), 100);
      expect(
        containerWith(<String, Object?>{uiScaleKey: 125}).$1
            .read(uiScaleProvider),
        125,
      );
      expect(
        containerWith(<String, Object?>{uiScaleKey: 999}).$1
            .read(uiScaleProvider),
        uiScaleMax,
      );
      expect(
        containerWith(<String, Object?>{uiScaleKey: 3}).$1
            .read(uiScaleProvider),
        uiScaleMin,
      );
      expect(
        containerWith(<String, Object?>{uiScaleKey: 'huge'}).$1
            .read(uiScaleProvider),
        uiScaleDefault,
      );
    });

    test('the keys snap to tens, up and down', () {
      final ProviderContainer container = containerWith(null).$1;
      final UiScale scale = container.read(uiScaleProvider.notifier);
      scale.step(UiScaleStep.zoomIn);
      expect(container.read(uiScaleProvider), 110);
      scale.choose(125);
      scale.step(UiScaleStep.zoomIn);
      expect(container.read(uiScaleProvider), 130);
      scale.choose(125);
      scale.step(UiScaleStep.zoomOut);
      expect(container.read(uiScaleProvider), 120);
      scale.step(UiScaleStep.reset);
      expect(container.read(uiScaleProvider), 100);
    });

    test('the keys stop at 50 and 300', () {
      final ProviderContainer container = containerWith(null).$1;
      final UiScale scale = container.read(uiScaleProvider.notifier);
      for (int i = 0; i < 30; i++) {
        scale.step(UiScaleStep.zoomOut);
      }
      expect(container.read(uiScaleProvider), 50);
      for (int i = 0; i < 30; i++) {
        scale.step(UiScaleStep.zoomIn);
      }
      expect(container.read(uiScaleProvider), 300);
    });

    test('the keys count from what applies, not from the choice', () {
      final ProviderContainer container = containerWith(<String, Object?>{
        uiScaleKey: 200,
      }).$1;
      final UiScale scale = container.read(uiScaleProvider.notifier);
      final int requests = container.read(uiScaleRequestsProvider).serial;
      // Das Fenster trägt 154: größer ändert nichts, sagt es aber an.
      scale.step(UiScaleStep.zoomIn, limit: 154);
      expect(container.read(uiScaleProvider), 200);
      expect(container.read(uiScaleRequestsProvider).serial, requests + 1);
      expect(container.read(uiScaleRequestsProvider).refused, isTrue);
      // Kleiner geht von 154 aus, nicht von 200.
      scale.step(UiScaleStep.zoomOut, limit: 154);
      expect(container.read(uiScaleProvider), 150);
      // Unter der Grenze wieder größer, bis über sie hinaus: die Wahl darf
      // höher stehen, als das Fenster trägt.
      scale.step(UiScaleStep.zoomIn, limit: 154);
      expect(container.read(uiScaleProvider), 160);
    });

    // Review (Codex): Im Fenster der Mindestgröße liegt die Grenze bei 100.
    // Ein Schritt nach oben schrieb trotzdem 110 in die Datei, und die
    // Anwendung sprang später darauf, sobald das Fenster wuchs.
    test('a step up at the limit changes nothing, also at exactly 100', () {
      final (ProviderContainer container, String path) = containerWith(null);
      final UiScale scale = container.read(uiScaleProvider.notifier);
      scale.step(UiScaleStep.zoomIn, limit: 100);
      expect(container.read(uiScaleProvider), 100);
      expect(container.read(uiScaleRequestsProvider).refused, isTrue);
      expect(File(path).existsSync(), isFalse, reason: 'nothing written');
      // Gleich der Grenze über 100: ebenso.
      scale.choose(110);
      scale.step(UiScaleStep.zoomIn, limit: 110);
      expect(container.read(uiScaleProvider), 110);
      expect(container.read(uiScaleRequestsProvider).refused, isTrue);
      // An 300 ist es die Grenze der Tasten, nicht des Fensters.
      scale.choose(300);
      scale.step(UiScaleStep.zoomIn);
      expect(container.read(uiScaleProvider), 300);
      expect(container.read(uiScaleRequestsProvider).refused, isFalse);
    });

    test('a choice is written down, the default as no choice', () {
      final (ProviderContainer container, String path) = containerWith(null);
      final UiScale scale = container.read(uiScaleProvider.notifier);
      Object? stored() =>
          (jsonDecode(File(path).readAsStringSync())
              as Map<String, Object?>)[uiScaleKey];
      scale.choose(150);
      expect(stored(), 150);
      scale.step(UiScaleStep.zoomIn);
      expect(stored(), 160);
      scale.step(UiScaleStep.reset);
      expect(stored(), isNull);
      expect(
        (jsonDecode(File(path).readAsStringSync()) as Map<String, Object?>)
            .containsKey(uiScaleKey),
        isTrue,
      );
    });
  });

  group('the chords', () {
    const UiScaleActivator larger = UiScaleActivator(UiScaleStep.zoomIn);
    const UiScaleActivator smaller = UiScaleActivator(UiScaleStep.zoomOut);
    const UiScaleActivator reset = UiScaleActivator(UiScaleStep.reset);

    testWidgets('need Ctrl, with Shift only for larger', (
      WidgetTester tester,
    ) async {
      expect(await accepts(tester, larger, LogicalKeyboardKey.equal), isTrue);
      expect(
        await accepts(
          tester,
          larger,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.shiftLeft,
          ],
        ),
        isTrue,
        reason: 'the + of a US keyboard is Shift+=',
      );
      expect(
        await accepts(tester, larger, LogicalKeyboardKey.numpadAdd),
        isTrue,
      );
      expect(await accepts(tester, smaller, LogicalKeyboardKey.minus), isTrue);
      expect(await accepts(tester, reset, LogicalKeyboardKey.digit0), isTrue);
      expect(
        await accepts(
          tester,
          larger,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[],
        ),
        isFalse,
        reason: 'a plain = belongs to whoever has the focus',
      );
      expect(
        await accepts(
          tester,
          larger,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.altLeft,
          ],
        ),
        isFalse,
        reason: 'Ctrl+Alt is the zoom of the terminal',
      );
      expect(
        await accepts(
          tester,
          smaller,
          LogicalKeyboardKey.minus,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.shiftLeft,
          ],
        ),
        isFalse,
        reason: 'Ctrl+_ belongs to the agent',
      );
      expect(
        await accepts(
          tester,
          reset,
          LogicalKeyboardKey.digit0,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.shiftLeft,
          ],
        ),
        isFalse,
        reason: 'Ctrl+Shift+0 is the rail',
      );
      expect(
        await accepts(
          tester,
          larger,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[LogicalKeyboardKey.metaLeft],
        ),
        isFalse,
        reason: 'Meta alone counts on macOS only',
      );
    });

    testWidgets('on macOS Cmd counts as well as Ctrl, never both', (
      WidgetTester tester,
    ) async {
      const UiScaleActivator mac = UiScaleActivator(
        UiScaleStep.zoomIn,
        platform: TargetPlatform.macOS,
      );
      expect(
        await accepts(
          tester,
          mac,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[LogicalKeyboardKey.metaLeft],
        ),
        isTrue,
      );
      expect(await accepts(tester, mac, LogicalKeyboardKey.equal), isTrue);
      expect(
        await accepts(
          tester,
          mac,
          LogicalKeyboardKey.equal,
          held: const <LogicalKeyboardKey>[
            LogicalKeyboardKey.controlLeft,
            LogicalKeyboardKey.metaLeft,
          ],
        ),
        isFalse,
      );
    });

    testWidgets('larger and smaller repeat while held, the reset does not', (
      WidgetTester tester,
    ) async {
      expect(
        await accepts(tester, larger, LogicalKeyboardKey.equal, repeat: true),
        isTrue,
      );
      expect(
        await accepts(tester, smaller, LogicalKeyboardKey.minus, repeat: true),
        isTrue,
      );
      expect(
        await accepts(tester, reset, LogicalKeyboardKey.digit0, repeat: true),
        isFalse,
      );
    });
  });

  group('the render object', () {
    testWidgets('lays the child out in the divided size and hits under the '
        'pointer', (WidgetTester tester) async {
      Offset? local;
      await tester.pumpWidget(
        Center(
          child: SizedBox(
            width: 800,
            height: 600,
            child: UiScaleBox(
              scale: 2,
              child: Listener(
                behavior: HitTestBehavior.opaque,
                onPointerDown: (PointerDownEvent event) =>
                    local = event.localPosition,
                child: const SizedBox.expand(key: Key('inner')),
              ),
            ),
          ),
        ),
      );
      final RenderBox inner = tester.renderObject(
        find.byKey(const Key('inner')),
      );
      expect(inner.size, const Size(400, 300), reason: 'the layout reflows');
      // Gemalt doppelt so groß: Die Fläche füllt die 800 x 600.
      expect(
        tester.getRect(find.byKey(const Key('inner'))).size,
        const Size(800, 600),
      );
      final Offset origin = tester.getTopLeft(find.byType(UiScaleBox));
      await tester.tapAt(origin + const Offset(300, 200));
      expect(local, const Offset(150, 100));
      expect(
        inner.localToGlobal(const Offset(150, 100)),
        origin + const Offset(300, 200),
      );
    });

    testWidgets('at 1 it adds no layer', (WidgetTester tester) async {
      // Ein Kind mit eigener Schicht, damit eine Transformation eine Schicht
      // braucht und nicht nur die Leinwand dreht.
      const Widget child = RepaintBoundary(child: SizedBox.expand());
      await tester.pumpWidget(const UiScaleBox(scale: 1, child: child));
      final RenderUiScale box = tester.renderObject(find.byType(UiScaleBox));
      expect(box.debugLayer, isNull);
      await tester.pumpWidget(const UiScaleBox(scale: 1.5, child: child));
      expect(box.debugLayer, isA<TransformLayer>());
    });

    test(
      'the media query divides size and insets and multiplies the ratio',
      () {
        const MediaQueryData outer = MediaQueryData(
          size: Size(3000, 2000),
          devicePixelRatio: 2,
          padding: EdgeInsets.all(30),
        );
        final MediaQueryData inner = scaledMediaQuery(
          outer,
          const Size(3000, 2000),
          1.5,
        );
        expect(inner.size, const Size(2000, 2000 / 1.5));
        expect(inner.devicePixelRatio, 3);
        expect(inner.padding, const EdgeInsets.all(20));
        expect(scaledMediaQuery(outer, outer.size, 1), same(outer));
      },
    );
  });

  group('the hairline', () {
    /// Eine Linie bei logisch 41 unter [scale], mit der `MediaQuery`, die der
    /// Wirt darunter meldet; die zweite Linie steht in einer eigenen
    /// Schicht mit fester Größe, die beim Wechsel der Skalierung kein neues
    /// Layout bekommt.
    Widget scaled(double scale) {
      const MediaQueryData outer = MediaQueryData(
        size: Size(800, 600),
        devicePixelRatio: 1,
      );
      return HTheme(
        tokens: HTokens.dark,
        child: UiScaleBox(
          scale: scale,
          child: MediaQuery(
            data: scaledMediaQuery(outer, outer.size, scale),
            child: const Align(
              alignment: Alignment.topLeft,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  SizedBox(height: 41, width: 200),
                  SizedBox(width: 200, child: HHairline()),
                  SizedBox(height: 7.3),
                  RepaintBoundary(
                    child: SizedBox(
                      width: 200,
                      height: 1,
                      child: HHairline(key: Key('cached')),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
    }

    List<RenderHairline> lines(WidgetTester tester) => tester
        .renderObjectList<RenderHairline>(
          find.descendant(
            of: find.byType(HHairline),
            matching: find.byWidgetPredicate(
              (Widget w) => w is LeafRenderObjectWidget,
            ),
          ),
        )
        .toList();

    test('rounds its thickness to whole physical pixels', () {
      RenderHairline at(double ratio) => RenderHairline(
        color: const Color(0xFF000000),
        vertical: false,
        ratio: ratio,
      );
      expect(at(1.25).thicknessFor(1) * 1.25, closeTo(1, 1e-9));
      expect(at(1.75).thicknessFor(1) * 1.75, closeTo(2, 1e-9));
      expect(at(1.25).integral, isFalse);
      expect(at(1).integral, isTrue);
      expect(at(2).integral, isTrue);
    });

    testWidgets('draws one sharp physical pixel inside a scaled tree, '
        'wherever it stands', (WidgetTester tester) async {
      await tester.pumpWidget(scaled(1.25));
      final List<RenderHairline> found = lines(tester);
      expect(found, hasLength(2));
      for (final RenderHairline line in found) {
        expect(line.ratio, 1.25);
        Rect? drawn;
        bool? antiAlias;
        expect(
          line,
          paints..something((Symbol method, List<dynamic> arguments) {
            if (method != #drawRect) {
              return false;
            }
            drawn = arguments[0] as Rect;
            antiAlias = (arguments[1] as Paint).isAntiAlias;
            return true;
          }),
        );
        // Am Anfang der Fläche, 0,8 logische Pixel stark: ein physischer.
        expect(drawn!.top, 0);
        expect(drawn!.height * 1.25, closeTo(1, 1e-9));
        expect(antiAlias, isFalse);
      }
    });

    // Review: Eine aus der Lage auf dem Schirm gerechnete Linie veraltet in
    // einer Schicht, die nicht neu malt. Die Linie hängt deshalb nur an der
    // `MediaQuery`, und ein Wechsel der Skalierung erreicht auch die Linie
    // in ihrer eigenen Schicht.
    testWidgets('a change of scale reaches a line in a cached layer', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(scaled(1.25));
      final RenderHairline cached = tester.renderObject<RenderHairline>(
        find.descendant(
          of: find.byKey(const Key('cached')),
          matching: find.byWidgetPredicate(
            (Widget w) => w is LeafRenderObjectWidget,
          ),
        ),
      );
      expect(cached.ratio, 1.25);
      await tester.pumpWidget(scaled(1.75));
      expect(cached.ratio, 1.75);
      expect(cached.thicknessFor(1) * 1.75, closeTo(2, 1e-9));
    });
  });
}
