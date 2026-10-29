// Die Naht zwischen Terminal und Reitern lässt sich ziehen (HUM-305).
//
// Der Besitzer am 2026-10-29: „die Mounts usw müssen nicht so viel Platz
// einnehmen, ich kann die Kante vom Terminal nicht runter- und das Terminal
// größer ziehen". Geprüft wird, was er tut: ziehen, doppelt klicken, die
// Pfeiltasten, ein Neustart über derselben Zustandsdatei -- und dass der
// Agent die neue Größe bekommt, ohne dass jeder Pixel ein Neuzeichnen ist.

import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/h_resizable_panes.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';
import 'package:humanitl/features/sandbox/providers/terminal_provider.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';
import 'package:humanitl/features/sandbox/widgets/sandbox_terminal.dart';
import 'package:humanitl/features/sandbox/widgets/terminal_pane.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:xterm2/core.dart';
import 'package:xterm2/ui.dart';

import 'harness.dart';

/// Ein Zustandsspeicher in einer eigenen, leeren Datei, deren Weg der Test
/// kennt: Er liest nach, was der Bildschirm hineingeschrieben hat, und baut
/// einen zweiten Bildschirm über derselben Datei.
UiStateFile _store() {
  final Directory dir = Directory.systemTemp.createTempSync('hum305-split');
  addTearDown(() => dir.deleteSync(recursive: true));
  return UiStateFile('${dir.path}/${UiStateFile.fileName}');
}

Rect _terminal(WidgetTester tester) =>
    tester.getRect(find.byType(SandboxTerminal));

Rect _panes(WidgetTester tester) =>
    tester.getRect(find.byType(HResizablePanes));

/// Die Mitte der Naht unter dem Terminal: Dort liegt der Griff.
Offset _seam(WidgetTester tester) {
  final Rect terminal = _terminal(tester);
  return Offset(terminal.center.dx, terminal.bottom + HSize.border / 2);
}

/// Was die Datei für den Sandbox-Abschnitt hält, oder null.
List<double>? _stored(UiStateFile store) {
  final Object? value = store.read()[sandboxPaneSplit.key];
  return value is List<Object?>
      ? <double>[for (final Object? v in value) (v! as num).toDouble()]
      : null;
}

/// Der Fokusknoten des Griffs.
FocusNode _gripFocus(WidgetTester tester) {
  final Finder grip = find
      .descendant(
        of: find.byType(HResizablePanes),
        matching: find.byType(FocusableActionDetector),
      )
      .last;
  return Focus.of(
    tester.element(
      find.descendant(of: grip, matching: find.byType(GestureDetector)).first,
    ),
  );
}

TerminalSessionState _session(WidgetTester tester, SandboxTestClient client) =>
    ProviderScope.containerOf(tester.element(find.byType(TerminalPane)))
        .read(terminalSessionProvider(client.sandbox.sandboxId!.value));

void main() {
  testWidgets('dragging the seam down grows the terminal', (
    WidgetTester tester,
  ) async {
    final UiStateFile store = _store();
    await pumpSandbox(
      tester,
      client: runningClient(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    final double before = _terminal(tester).height;
    // Die Vorgabe: 60 Prozent der Höhe, Naht abgezogen.
    expect(before, closeTo((_panes(tester).height - HSize.border) * 0.6, 1));

    // Mit der Maus, wie am Schreibtisch: Ihre Schwelle ist ein Pixel, und
    // die Naht folgt dem Zeiger.
    await tester.dragFrom(
      _seam(tester),
      const Offset(0, 120),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump(kDoubleTapTimeout);

    expect(_terminal(tester).height, closeTo(before + 120, 2));
    expect(tester.takeException(), isNull);
  });

  testWidgets('the tabs shrink to their minimum and no further, and scroll', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: runningClient());

    await tester.dragFrom(_seam(tester), const Offset(0, 2000));
    await tester.pump(kDoubleTapTimeout);

    final double details =
        _panes(tester).height - _terminal(tester).height - HSize.border;
    expect(details, closeTo(sandboxDetailsMinHeight, 1));
    // Die Reiterleiste bleibt erreichbar, der Inhalt rollt, statt über den
    // Rand zu malen.
    expect(
      find.byKey(const Key('sandbox-tab-mounts')).hitTestable(),
      findsOneWidget,
    );
    expect(find.byKey(const Key('sandbox-details-scroll')), findsOneWidget);
    expect(tester.takeException(), isNull);

    // Und nach oben bleibt dem Terminal sein Minimum.
    await tester.dragFrom(_seam(tester), const Offset(0, -2000));
    await tester.pump(kDoubleTapTimeout);
    expect(_terminal(tester).height, closeTo(sandboxTerminalMinHeight, 1));
    expect(tester.takeException(), isNull);
  });

  testWidgets('the split is written when the drag ends and comes back', (
    WidgetTester tester,
  ) async {
    final UiStateFile store = _store();
    await pumpSandbox(
      tester,
      client: runningClient(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );

    final TestGesture gesture = await tester.startGesture(_seam(tester));
    for (int i = 0; i < 10; i++) {
      await gesture.moveBy(const Offset(0, 12));
      await tester.pump();
    }
    // Mitten im Zug steht noch nichts in der Datei: Geschrieben wird einmal,
    // am Ende, nicht für jeden Pixel.
    expect(_stored(store), isNull);
    await gesture.up();
    await tester.pump(kDoubleTapTimeout);

    final double dragged = _terminal(tester).height;
    final List<double>? stored = _stored(store);
    expect(stored, isNotNull);
    expect(stored![0], greaterThan(sandboxTerminalFraction));

    // Ein Neustart: ein neuer Baum über derselben Datei.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpSandbox(
      tester,
      client: runningClient(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    expect(_terminal(tester).height, closeTo(dragged, 1));
  });

  testWidgets('a double click on the seam restores the default split', (
    WidgetTester tester,
  ) async {
    final UiStateFile store = _store();
    await pumpSandbox(
      tester,
      client: runningClient(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    final double initial = _terminal(tester).height;
    await tester.dragFrom(_seam(tester), const Offset(0, 150));
    await tester.pump(kDoubleTapTimeout);
    expect(_terminal(tester).height, isNot(closeTo(initial, 1)));

    await tester.tapAt(_seam(tester));
    await tester.pump(kDoubleTapMinTime);
    await tester.tapAt(_seam(tester));
    await tester.pump(kDoubleTapTimeout);

    expect(_terminal(tester).height, closeTo(initial, 1));
    final List<double> stored = _stored(store)!;
    expect(stored[0], closeTo(sandboxTerminalFraction, 1e-9));
  });

  testWidgets('a drag and a click right after it are no double click', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: runningClient());
    await tester.dragFrom(
      _seam(tester),
      const Offset(0, 80),
      kind: PointerDeviceKind.mouse,
    );
    await tester.pump();
    final double dragged = _terminal(tester).height;

    // Ein einzelner Klick innerhalb der Frist: Der Zug davor war kein Klick,
    // auch wenn er kürzer als die Doppelklick-Schwelle war.
    await tester.tapAt(_seam(tester), kind: PointerDeviceKind.mouse);
    await tester.pump(kDoubleTapTimeout);

    expect(_terminal(tester).height, closeTo(dragged, 1));
  });

  testWidgets('the arrow keys move a focused seam; the terminal keeps them', (
    WidgetTester tester,
  ) async {
    final UiStateFile store = _store();
    final SandboxTestClient client = runningClient();
    await pumpSandbox(
      tester,
      client: client,
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    await tester.pump();
    await tester.pump();
    final double initial = _terminal(tester).height;

    // Mit dem Fokus im Terminal gehört der Pfeil dem Agenten.
    await tester.tap(find.byType(TerminalView));
    await tester.pump();
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(client.typed, <int>[0x1b, 0x5b, 0x42]);
    expect(_terminal(tester).height, initial);

    // Mit dem Fokus auf der Naht verschiebt er sie, und zwar quer zur Naht.
    _gripFocus(tester).requestFocus();
    await tester.pump();
    client.typed.clear();
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
    await tester.pump();
    expect(_terminal(tester).height, closeTo(initial + HSize.splitterStep, 1));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowUp);
    await tester.pump();
    expect(_terminal(tester).height, closeTo(initial - HSize.splitterStep, 1));
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pump();
    expect(_terminal(tester).height, closeTo(initial - HSize.splitterStep, 1));
    expect(client.typed, isEmpty, reason: 'the seam had the keys');
    // Ein Tastendruck ist ein ganzer Zug und wird gespeichert wie einer.
    expect(_stored(store)![0], lessThan(sandboxTerminalFraction));
  });

  testWidgets('the terminal resize reaches the daemon, but not per pixel', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);
    await tester.pump();
    await tester.pump();
    final Terminal terminal = _session(tester, client).terminal;
    expect(_session(tester, client).phase, TerminalPhase.attached);
    // Die erste Geometrie geht mit `TerminalOpen`; ab hier zählt der Zug.
    await tester.pump(terminalResizeInterval * 2);
    client.resizes.clear();

    // Ein Zug wie von Hand: ein Frame alle 16 ms, die Uhr läuft mit.
    const Duration frame = Duration(milliseconds: 16);
    const int steps = 40;
    final Set<int> rowsSeen = <int>{terminal.viewHeight};
    final TestGesture gesture = await tester.startGesture(_seam(tester));
    for (int i = 0; i < steps; i++) {
      await gesture.moveBy(const Offset(0, 6));
      await tester.pump(frame);
      rowsSeen.add(terminal.viewHeight);
    }
    await gesture.up();
    await tester.pump();
    // Die Frist läuft ab, und die letzte Größe geht hinauf.
    await tester.pump(terminalResizeInterval);

    final int rowChanges = rowsSeen.length - 1;
    expect(rowChanges, greaterThan(4), reason: 'the drag changed many rows');
    expect(client.resizes, isNotEmpty);
    final Duration elapsed = frame * steps + terminalResizeInterval;
    expect(
      client.resizes.length,
      lessThanOrEqualTo(
        elapsed.inMicroseconds ~/ terminalResizeInterval.inMicroseconds + 2,
      ),
      reason: 'at most one resize per interval, not one per row change',
    );
    expect(client.resizes.length, lessThan(rowChanges));
    expect(client.resizes.last, (
      terminal.viewWidth,
      terminal.viewHeight,
    ), reason: 'the last size of the drag always reaches the agent');
  });
}
