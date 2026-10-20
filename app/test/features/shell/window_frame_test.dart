// Die Kopfleiste ist die Titelleiste des Fensters (HUM-255): Griff zum
// Verschieben, Doppelklick zum Maximieren, eigene Fensterknöpfe.
//
// Alles läuft in der ganzen App über `pumpApp`, mit `FakeDesktop` hinter
// `desktopPortsProvider`; `FakeWindow` zählt, was am Fenster ankommt.
//
// Umkehrungen, gegen die diese Tests stehen (je einmal ausprobiert, je rot):
//
// - Ein Knopf aus `WindowButtons` entfernt: `the header carries ...`.
// - Schließen ruft `quit` statt `close`: `close takes the window manager's
//   way ...`.
// - `onPressed` von Minimieren und Maximieren vertauscht: `minimize and
//   maximize reach the port`.
// - `_WindowButton` in `ExcludeFocus`: `the window buttons answer the
//   keyboard`.
// - `onPanStart` der Griff-Fläche entfernt: `dragging the header moves the
//   window`.
// - `IgnorePointer` um den Bereichstitel entfernt: ebenda, der Titel ist dann
//   ein toter Fleck.
// - Griff-Fläche als Elternteil der Zeile statt als Geschwister darunter:
//   `a control in the header does not drag` und `a control's click is not
//   held back by the double click`.
// - `onDoubleTap` entfernt: `a double click on the header toggles maximize`.
// - `WindowFramed` um `SetupHost` in `ConnectionGate` entfernt: `the setup
//   screen of a cold start can be moved and closed`.

import 'package:flutter/gestures.dart' show kDoubleTapTimeout;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/providers/language.dart';
import 'package:humanitl/features/shell/widgets/command_palette.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/tray/providers/attention.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';
import '../tray/fixtures.dart';

/// Deutsch, unabhängig von Konfiguration und Rechner.
class _German extends LanguageSetting {
  @override
  AppLanguage? build() => AppLanguage.de;
}

const Key _minimize = Key('window-minimize');
const Key _maximize = Key('window-maximize');
const Key _close = Key('window-close');
const Key _palette = Key('header-palette-button');
const Key _title = Key('header-section-title');

/// Baut die App über [desktop].
Future<void> _pump(
  WidgetTester tester,
  FakeDesktop desktop, {
  List<Override> overrides = const <Override>[],
}) => pumpApp(
  tester,
  client: FakeDaemonClient(),
  overrides: <Override>[
    desktopPortsProvider.overrideWithValue(desktop.ports),
    ...overrides,
  ],
);

/// Ein Punkt auf der freien Fläche der Kopfleiste: rechts neben dem
/// Bereichstitel, wo der `Spacer` steht.
Offset _freeSpot(WidgetTester tester) {
  final Rect title = tester.getRect(find.byKey(_title));
  return Offset(title.right + 48, title.center.dy);
}

/// Verlangt unter dem Knopf [key] einen Semantik-Knoten „Knopf" mit [label].
void _expectButton(WidgetTester tester, Key key, String label) {
  expect(
    tester.getSemantics(
      find.descendant(of: find.byKey(key), matching: find.byType(HGlyphIcon)),
    ),
    isSemantics(label: label, isButton: true),
  );
}

void main() {
  testWidgets('the header carries minimize, maximize and close', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle semantics = tester.ensureSemantics();
    await _pump(tester, FakeDesktop());
    final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
    for (final (Key key, String label) in <(Key, String)>[
      (_minimize, en.shellWindowMinimize),
      (_maximize, en.shellWindowMaximize),
      (_close, en.shellWindowClose),
    ]) {
      expect(find.byKey(key), findsOneWidget);
      _expectButton(tester, key, label);
    }
    // Rechts vom Palettenknopf, in dieser Reihenfolge.
    final double palette = tester.getRect(find.byKey(_palette)).right;
    final double minimize = tester.getRect(find.byKey(_minimize)).left;
    final double maximize = tester.getRect(find.byKey(_maximize)).left;
    final double close = tester.getRect(find.byKey(_close)).left;
    expect(
      palette < minimize && minimize < maximize && maximize < close,
      isTrue,
    );
    semantics.dispose();
  });

  testWidgets('the window buttons speak German', (WidgetTester tester) async {
    final SemanticsHandle semantics = tester.ensureSemantics();
    await _pump(
      tester,
      FakeDesktop(),
      overrides: <Override>[languageProvider.overrideWith(_German.new)],
    );
    final AppLocalizations de = lookupAppLocalizations(const Locale('de'));
    expect(de.shellWindowClose, 'Schließen');
    _expectButton(tester, _minimize, de.shellWindowMinimize);
    _expectButton(tester, _maximize, de.shellWindowMaximize);
    _expectButton(tester, _close, de.shellWindowClose);
    expect(tester.takeException(), isNull);
    semantics.dispose();
  });

  testWidgets("close takes the window manager's way, not the tray's", (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    await tester.tap(find.byKey(_close));
    await tester.pump();
    expect(desktop.window.closes, 1);
    expect(desktop.window.quits, 0);
    expect(desktop.log.entries, isNot(contains('window.dispose')));
    expect(desktop.log.entries, isNot(contains('tray.dispose')));
  });

  testWidgets('minimize and maximize reach the port', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    await tester.tap(find.byKey(_minimize));
    await tester.pump();
    expect(desktop.window.minimizes, 1);
    expect(desktop.window.maximizeToggles, 0);
    await tester.tap(find.byKey(_maximize));
    await tester.pump();
    expect(desktop.window.minimizes, 1);
    expect(desktop.window.maximizeToggles, 1);
    expect(desktop.window.closes, 0);
  });

  testWidgets('the window buttons answer the keyboard', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    // Der Fokusknoten des Knopfs ist der von `HControl`, unter dem Glyph.
    Future<void> focus(Key key) async {
      Focus.of(
        tester.element(
          find.descendant(
            of: find.byKey(key),
            matching: find.byType(HGlyphIcon),
          ),
        ),
      ).requestFocus();
      await tester.pump();
    }

    await focus(_close);
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(desktop.window.closes, 1);

    await focus(_minimize);
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pump();
    expect(desktop.window.minimizes, 1);
  });

  testWidgets('dragging the header moves the window', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    await tester.dragFrom(_freeSpot(tester), const Offset(80, 30));
    await tester.pump();
    expect(desktop.window.drags, 1);
    await tester.dragFrom(
      tester.getCenter(find.byKey(_title)),
      const Offset(80, 30),
    );
    await tester.pump();
    expect(desktop.window.drags, 2);
    // Ein Ziehen ist kein Doppelklick.
    await tester.pump(kDoubleTapTimeout);
    expect(desktop.window.maximizeToggles, 0);
  });

  testWidgets('a control in the header does not drag', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    await tester.tap(find.byKey(_palette));
    await tester.pump(kDoubleTapTimeout);
    expect(find.byType(CommandPalette), findsOneWidget);
    expect(desktop.window.drags, 0);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();

    await tester.dragFrom(
      tester.getCenter(find.byKey(_palette)),
      const Offset(-80, 30),
    );
    await tester.pump(kDoubleTapTimeout);
    expect(desktop.window.drags, 0);
    expect(desktop.window.maximizeToggles, 0);
  });

  testWidgets('a double click on the header toggles maximize', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await _pump(tester, desktop);
    final Offset spot = _freeSpot(tester);
    await tester.tapAt(spot);
    await tester.pump(const Duration(milliseconds: 60));
    await tester.tapAt(spot);
    await tester.pump(kDoubleTapTimeout);
    expect(desktop.window.maximizeToggles, 1);
    expect(desktop.window.drags, 0);
    // Ein einzelner Klick maximiert nicht.
    await tester.tapAt(spot);
    await tester.pump(kDoubleTapTimeout * 2);
    expect(desktop.window.maximizeToggles, 1);
  });

  testWidgets("a control's click is not held back by the double click", (
    WidgetTester tester,
  ) async {
    await _pump(tester, FakeDesktop());
    await tester.tap(find.byKey(_palette));
    await tester.pump();
    expect(find.byType(CommandPalette), findsOneWidget);
  });

  testWidgets('the setup screen of a cold start can be moved and closed', (
    WidgetTester tester,
  ) async {
    final FakeDesktop desktop = FakeDesktop();
    await pumpApp(
      tester,
      client: FakeDaemonClient()..goOffline(),
      overrides: <Override>[
        desktopPortsProvider.overrideWithValue(desktop.ports),
      ],
    );
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(find.byKey(_minimize), findsOneWidget);
    expect(find.byKey(_maximize), findsOneWidget);
    await tester.tap(find.byKey(_close));
    await tester.pump();
    expect(desktop.window.closes, 1);
    expect(desktop.window.quits, 0);
    // Die Leiste über dem Setup ist Griff: Ziehen links neben den Knöpfen.
    final Rect bar = tester.getRect(find.byKey(const Key('window-bar')));
    await tester.dragFrom(
      bar.centerLeft + const Offset(40, 0),
      const Offset(60, 20),
    );
    await tester.pump();
    expect(desktop.window.drags, 1);
    // Der Setup-Bildschirm misst nach; seine Zeitgeber laufen mit dem Baum
    // aus, wie in `setup_test.dart`.
    await tester.pumpWidget(const SizedBox.shrink());
    await tester.pump(const Duration(milliseconds: 300));
  });

  testWidgets('the shell shows one set of window buttons', (
    WidgetTester tester,
  ) async {
    await _pump(tester, FakeDesktop());
    expect(find.byKey(_close), findsOneWidget);
    expect(find.byKey(const Key('window-bar')), findsNothing);
  });
}
