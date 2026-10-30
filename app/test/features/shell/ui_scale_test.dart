// Die ganze Anwendung skaliert (HUM-356): `Ctrl` mit `+`, `-` und `0`, die
// Palette und eine Einstellung.
//
// Geprüft wird an der ganzen App: Jede Seite bei jeder Stufe in zwei
// Fenstergrößen ohne Überlauf und ohne Ausnahme; das Layout bricht im
// kleineren logischen Fenster neu um, statt ein vergrößertes Bild zu
// zeigen; ein zu kleines Fenster begrenzt die Skalierung und sagt es; die
// Griffe gelten auch, während das Terminal die Tastatur hat, und erreichen
// den Agenten nicht; die Wahl übersteht einen Neustart.
//
// Linux als Plattform, wo Tasten im Spiel sind: Ohne Variante spielt
// `flutter test` Android.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/gestures.dart' show kSecondaryButton;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/intents.dart';
import 'package:humanitl/core/ui/terminal_zoom.dart';
import 'package:humanitl/core/ui/ui_scale.dart';
import 'package:humanitl/features/intercept/providers/coach_mark.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/features/shell/widgets/window_frame.dart';
import 'package:xterm2/ui.dart';

import '../../harness/app_harness.dart';
import '../intercept/fixtures.dart' hide settle;
import '../sandbox/harness.dart';
import 'terminal_full_window_test.dart' show settle;
import 'terminal_zoom_test.dart'
    show fontSize, geometry, pumpSandboxWithTerminalFocused;

/// Die beiden Fenster der Prüfung: Full HD und 4K, bei einer Pixeldichte
/// von 1 physisch so groß wie logisch.
const List<Size> windows = <Size>[Size(1920, 1080), Size(3840, 2160)];

/// Ein leeres Verzeichnis für die Zustandsdatei dieses Tests.
String freshStatePath() {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-scale');
  addTearDown(() => dir.deleteSync(recursive: true));
  return '${dir.path}/${UiStateFile.fileName}';
}

/// Eine Zustandsdatei unter [path] mit der Wahl [percent], oder ohne Wahl.
Override scaleState(String path, {int? percent}) {
  File(path).writeAsStringSync(
    jsonEncode(<String, Object?>{
      coachMarkSeenKey: true,
      // Die Vorgabe wird nie geschrieben; so steht es auch hier.
      if (percent != null && percent != uiScaleDefault) uiScaleKey: percent,
    }),
  );
  return uiStateFileProvider.overrideWithValue(UiStateFile(path));
}

/// Was in der Zustandsdatei unter [uiScaleKey] steht.
Object? storedScale(String path) =>
    (jsonDecode(File(path).readAsStringSync())
        as Map<String, Object?>)[uiScaleKey];

/// Zwei gehaltene Anfragen, damit Intercept Karte, Leiste und Kontext zeigt.
List<FlowDetail> heldDetails() => <FlowDetail>[
  detailFor(
    heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(minutes: 5)),
      method: Method.post,
      host: 'api.github.com',
      apex: 'github.com',
      path: '/repos/humanitl/humanitl/issues/356/comments?first=20',
      requestSize: 428,
    ),
  ),
  detailFor(
    heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(minutes: 8)),
      host: 'registry.npmjs.org',
      apex: 'npmjs.org',
      path: '/react',
    ),
  ),
];

/// Die App mit gehaltenen Anfragen in [size], mit der Wahl [percent].
Future<String> pumpScaled(
  WidgetTester tester, {
  required Size size,
  int? percent,
  String? path,
}) async {
  final String state = path ?? freshStatePath();
  final List<FlowDetail> details = heldDetails();
  final TestDaemonClient client = TestDaemonClient();
  for (final FlowDetail detail in details) {
    client.details[detail.summary.id] = detail;
  }
  await pumpApp(
    tester,
    client: client,
    size: size,
    uiState: scaleState(state, percent: percent),
    overrides: <Override>[
      nowProvider.overrideWith(
        () => FixedNow(testStart.add(const Duration(seconds: 40))),
      ),
      flowsProvider.overrideWith(
        () => FixedFlows(<FlowId, Flow>{
          for (final FlowDetail detail in details)
            detail.summary.id: detail.summary,
        }),
      ),
    ],
  );
  await tester.pumpAndSettle();
  return state;
}

/// Die logische Größe, in der sich die Shell anlegt: die der Kopfleiste
/// in ihren eigenen Koordinaten, und das Fenster, wie es die `MediaQuery`
/// darunter meldet.
(Size, Size) logicalWindow(WidgetTester tester) {
  final Finder header = find.byType(HeaderBar);
  final RenderBox box = tester.renderObject<RenderBox>(header);
  return (box.size, MediaQuery.sizeOf(tester.element(header)));
}

/// [key] mit gehaltenem `Ctrl`; die Tasten des Ziffernblocks und das `+`
/// einer deutschen Tastatur kommen als Android-Ereignis, weil der Simulator
/// für sie keinen Linux-Code kennt.
Future<void> ctrl(
  WidgetTester tester,
  LogicalKeyboardKey key, {
  bool shift = false,
}) async {
  final Set<LogicalKeyboardKey> androidOnly = <LogicalKeyboardKey>{
    LogicalKeyboardKey.add,
    LogicalKeyboardKey.numpadSubtract,
    LogicalKeyboardKey.numpad0,
  };
  await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
  if (shift) {
    await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyEvent(
    key,
    platform: androidOnly.contains(key) ? 'android' : null,
    physicalKey: key == LogicalKeyboardKey.add
        ? PhysicalKeyboardKey.bracketRight
        : null,
  );
  if (shift) {
    await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
  }
  await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
  await tester.pump();
  await tester.pump();
}

/// Die Skalierung, die gilt, gelesen aus dem Layout: Fensterbreite durch
/// logische Breite der Kopfleiste, in ganzen Prozent.
int appliedPercent(WidgetTester tester) {
  final double window = tester.getRect(find.byType(HeaderBar)).width;
  return (window / logicalWindow(tester).$1.width * 100).round();
}

ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(HeaderBar)));

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  // Kriterium: jede Seite, jede Stufe, zwei Fenster, kein Überlauf. Ein
  // `RenderFlex`-Überlauf meldet sich als Fehler über `FlutterError`, und
  // den hält der Test als Ausnahme fest.
  for (final Size window in windows) {
    for (final int percent in uiScalePresets) {
      testWidgets('every page fits at $percent % in ${window.width.round()}x'
          '${window.height.round()}', (WidgetTester tester) async {
        await pumpScaled(tester, size: window, percent: percent);
        final int expected = effectiveUiScale(percent, window);
        expect(appliedPercent(tester), expected);
        for (final Section section in Section.values) {
          await pressCtrl(tester, navigationKeys[section.index]);
          await tester.pumpAndSettle();
          expect(
            tester.takeException(),
            isNull,
            reason: '${section.name} at $percent %',
          );
          final (Size header, Size media) = logicalWindow(tester);
          // Nie kleiner als das Layout der drei Panes braucht.
          if (expected > 100) {
            expect(header.width, greaterThanOrEqualTo(1100));
            expect(media.height, greaterThanOrEqualTo(700));
          }
        }
      }, variant: linux);
    }
  }

  testWidgets('the layout reflows in the smaller logical window', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(1920, 1080), percent: 150);
    final (Size header, Size media) = logicalWindow(tester);
    // Bei 150 % legt sich die Shell in 1280 x 720 an und malt 1920 breit.
    expect(header.width, 1280);
    expect(media, const Size(1280, 720));
    expect(tester.getRect(find.byType(HeaderBar)).width, 1920);
    expect(
      MediaQuery.devicePixelRatioOf(tester.element(find.byType(HeaderBar))),
      tester.view.devicePixelRatio * 1.5,
    );
  });

  testWidgets('a small window clamps 200 % to what fits and re-clamps on a '
      'resize', (WidgetTester tester) async {
    await pumpScaled(tester, size: const Size(1280, 800), percent: 200);
    expect(tester.takeException(), isNull);
    expect(appliedPercent(tester), 114);
    expect(logicalWindow(tester).$2.width, greaterThanOrEqualTo(1100));
    expect(logicalWindow(tester).$2.height, greaterThanOrEqualTo(700));
    expect(containerOf(tester).read(uiScaleProvider), 200, reason: 'kept');

    // Größer: Die Wahl kehrt zurück.
    await tester.binding.setSurfaceSize(const Size(3840, 2160));
    await tester.pumpAndSettle();
    expect(appliedPercent(tester), 200);
    // Wieder klein: wieder begrenzt, ohne Überlauf.
    await tester.binding.setSurfaceSize(const Size(1280, 800));
    await tester.pumpAndSettle();
    expect(appliedPercent(tester), 114);
    expect(tester.takeException(), isNull);
  });

  // Kriterium 1 knapp über der Mindestgröße: 1210 x 770 trägt genau 110 %,
  // jede größere Wahl wird darauf begrenzt.
  for (final int percent in <int>[110, 125, 200]) {
    testWidgets('every page fits at $percent % just above the minimum window', (
      WidgetTester tester,
    ) async {
      const Size window = Size(1210, 770);
      await pumpScaled(tester, size: window, percent: percent);
      expect(appliedPercent(tester), 110);
      for (final Section section in Section.values) {
        await pressCtrl(tester, navigationKeys[section.index]);
        await tester.pumpAndSettle();
        expect(
          tester.takeException(),
          isNull,
          reason: '${section.name} at $percent %',
        );
        final (Size header, Size media) = logicalWindow(tester);
        expect(header.width, greaterThanOrEqualTo(1100));
        expect(media.height, greaterThanOrEqualTo(700));
      }
    }, variant: linux);
  }

  // Review (Codex, Sonnet): Steht die Wahl genau an der Grenze, änderte
  // `Ctrl+Plus` nichts, und der Hinweis sagte nur „110%".
  testWidgets('a refused step at a choice equal to the limit says so', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(1210, 770), percent: 110);
    await ctrl(tester, LogicalKeyboardKey.equal);
    expect(appliedPercent(tester), 110);
    expect(find.text('110%, the largest this window allows'), findsOneWidget);
  }, variant: linux);

  // Review (Codex): Im Fenster der Mindestgröße schrieb `Ctrl+Plus` bei
  // 100 % trotzdem 110 in die Datei.
  testWidgets(
    'at the minimum window Ctrl+Plus keeps 100 % and writes nothing',
    (WidgetTester tester) async {
      final String path = await pumpScaled(tester, size: const Size(1100, 700));
      await ctrl(tester, LogicalKeyboardKey.equal);
      expect(appliedPercent(tester), 100);
      expect(storedScale(path), isNull);
      expect(find.text('100%, the largest this window allows'), findsOneWidget);
      await tester.binding.setSurfaceSize(const Size(3840, 2160));
      await tester.pumpAndSettle();
      expect(appliedPercent(tester), 100, reason: 'no jump when it grows');
    },
    variant: linux,
  );

  // Review (Sonnet): Die Befehle der Palette lasen die Grenze beim Bau der
  // Liste. Ein Fenster, das danach kleiner wird, muss beim Ausführen zählen.
  testWidgets('a palette command counts from the limit when it runs', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(3840, 2160), percent: 200);
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.enterText(find.byKey(const Key('palette-input')), 'smaller');
    await tester.pump();
    await tester.binding.setSurfaceSize(const Size(1920, 1080));
    await tester.pump();
    expect(find.byKey(const Key('palette-scale-zoomOut')), findsOneWidget);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await settle(tester);
    // Von den 154, die 1920 x 1080 trägt, nicht von 200.
    expect(containerOf(tester).read(uiScaleProvider), 150);
  }, variant: linux);

  testWidgets('a chord at the limit says the window allows no more', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(1280, 800), percent: 110);
    await ctrl(tester, LogicalKeyboardKey.equal);
    expect(appliedPercent(tester), 114);
    expect(find.text('114%, the largest this window allows'), findsOneWidget);
    await tester.pump(const Duration(seconds: 2));
    expect(find.byKey(const Key('ui-scale-notice')), findsNothing);
  }, variant: linux);

  testWidgets('Ctrl with +, - and 0 scale the app and show the value', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(3840, 2160));
    expect(appliedPercent(tester), 100);

    await ctrl(tester, LogicalKeyboardKey.equal);
    expect(appliedPercent(tester), 110);
    expect(find.text('110%'), findsOneWidget);
    await ctrl(tester, LogicalKeyboardKey.equal, shift: true);
    await ctrl(tester, LogicalKeyboardKey.add);
    await ctrl(tester, LogicalKeyboardKey.numpadAdd);
    expect(appliedPercent(tester), 140);
    await ctrl(tester, LogicalKeyboardKey.minus);
    await ctrl(tester, LogicalKeyboardKey.numpadSubtract);
    expect(appliedPercent(tester), 120);
    await ctrl(tester, LogicalKeyboardKey.digit0);
    expect(appliedPercent(tester), 100);
    await ctrl(tester, LogicalKeyboardKey.equal);
    await ctrl(tester, LogicalKeyboardKey.numpad0);
    expect(appliedPercent(tester), 100);

    // `Ctrl+Shift+0` ist die Leiste, nicht die Skalierung.
    await ctrl(tester, LogicalKeyboardKey.equal);
    await ctrl(tester, LogicalKeyboardKey.digit0, shift: true);
    expect(appliedPercent(tester), 110);
    expect(tester.takeException(), isNull);
  }, variant: linux);

  testWidgets('the chords work with the terminal focused and never reach the '
      'agent; the terminal keeps its font and reports its new grid', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = await pumpSandboxWithTerminalFocused(
      tester,
    );
    // Groß genug für 130 % (die Grenze von 1920 x 1080 liegt bei 154).
    await tester.binding.setSurfaceSize(const Size(1920, 1080));
    await tester.pump(terminalResizeInterval * 2);
    await tester.pump();
    final (int, int) before = geometry(tester);
    client.typed.clear();
    client.resizes.clear();

    await ctrl(tester, LogicalKeyboardKey.equal);
    await ctrl(tester, LogicalKeyboardKey.equal);
    await ctrl(tester, LogicalKeyboardKey.equal);
    await tester.pump(terminalResizeInterval * 2);
    await tester.pump();
    expect(appliedPercent(tester), 130);
    expect(client.typed, isEmpty, reason: 'no chord reached the agent');
    expect(fontSize(tester), terminalFontSizeDefault, reason: 'logical font');
    final (int, int) after = geometry(tester);
    expect(after.$1, lessThan(before.$1), reason: 'fewer columns');
    expect(after.$2, lessThan(before.$2), reason: 'fewer rows');
    expect(client.resizes.last, after, reason: 'the daemon heard of it');

    await ctrl(tester, LogicalKeyboardKey.digit0);
    expect(appliedPercent(tester), 100);
    expect(client.typed, isEmpty);
  }, variant: linux);

  testWidgets('the palette scales in every section', (
    WidgetTester tester,
  ) async {
    await pumpScaled(tester, size: const Size(3840, 2160));
    Future<void> run(String query, String id) async {
      await pressCtrl(tester, LogicalKeyboardKey.keyK);
      await tester.enterText(find.byKey(const Key('palette-input')), query);
      await tester.pump();
      expect(find.byKey(Key('palette-scale-$id')), findsOneWidget);
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await settle(tester);
    }

    await run('larger', 'zoomIn');
    expect(appliedPercent(tester), 110);
    await run('smaller', 'zoomOut');
    await run('smaller', 'zoomOut');
    expect(appliedPercent(tester), 90);
    await run('original size', 'reset');
    expect(appliedPercent(tester), 100);
  }, variant: linux);

  testWidgets('the setting offers the presets and a choice survives a '
      'restart', (WidgetTester tester) async {
    final String path = freshStatePath();
    await pumpApp(
      tester,
      client: FakeDaemonClient.empty(),
      size: const Size(3840, 2160),
      uiState: scaleState(path),
    );
    await tester.pumpAndSettle();
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    final Finder chips = find.byKey(const Key('settings-scale'));
    await tester.ensureVisible(chips);
    await tester.pumpAndSettle();
    for (final int percent in uiScalePresets) {
      expect(
        find.descendant(of: chips, matching: find.text('$percent%')),
        findsOneWidget,
      );
    }
    await tester.tap(find.descendant(of: chips, matching: find.text('150%')));
    await tester.pumpAndSettle();
    expect(appliedPercent(tester), 150);
    expect(storedScale(path), 150);
    expect(tester.takeException(), isNull);

    // Neustart über derselben Datei: 150 % vom ersten Bild an.
    await tester.pumpWidget(const SizedBox());
    await pumpApp(
      tester,
      client: FakeDaemonClient.empty(),
      size: const Size(3840, 2160),
      uiState: uiStateFileProvider.overrideWithValue(UiStateFile(path)),
    );
    expect(appliedPercent(tester), 150, reason: 'from the first frame');

    // Zurück auf 100: keine Wahl mehr in der Datei.
    await ctrl(tester, LogicalKeyboardKey.digit0);
    expect(storedScale(path), isNull);
  }, variant: linux);

  testWidgets('a context menu opens under the pointer at 150 %', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpApp(
      tester,
      client: client,
      size: const Size(1920, 1080),
      uiState: scaleState(freshStatePath(), percent: 150),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit4);
    await settle(tester);
    final Offset at = tester.getCenter(find.byType(TerminalView));
    await tester.tapAt(at, buttons: kSecondaryButton);
    await settle(tester);
    final Finder copy = find.text('Copy');
    expect(copy, findsOneWidget);
    // Die erste Zeile des Menüs steht unter dem Zeiger, nicht bei der
    // anderthalbfachen Position.
    expect(
      (tester.getTopLeft(copy) - at).distance,
      lessThan(80),
      reason: 'under the pointer',
    );
  }, variant: linux);
  // Review (Codex): Die Ampel von macOS zeichnet das System in Pixeln des
  // Fensters. Ihr Platz wuchs mit der Skalierung, und bei 50 % begann der
  // Inhalt unter ihr.
  testWidgets('the room of the traffic lights stays 72 window pixels', (
    WidgetTester tester,
  ) async {
    late EdgeInsets padding;
    for (final int percent in <int>[50, 100, 200]) {
      await tester.pumpWidget(
        UiScaleScope(
          preferred: percent,
          limit: uiScaleMax,
          effective: percent,
          child: Builder(
            builder: (BuildContext context) {
              padding = titleBarPadding(context, 12);
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      // In Pixeln des Fensters: der logische Rand mal die Skalierung.
      expect(
        (padding.left - 12) * percent / 100,
        closeTo(trafficLightInset, 1e-9),
        reason: '$percent %',
      );
      expect(padding.right, 12);
    }
  }, variant: TargetPlatformVariant.only(TargetPlatform.macOS));
}
