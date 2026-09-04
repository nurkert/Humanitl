// Das Icon, das die Zahl trägt (HUM-034). Gezeichnet statt geladen, damit
// Zustandsfarbe und Design-Token dieselben bleiben.

import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/tray/desktop_ports.dart';
import 'package:humanitl/features/tray/tray_icon.dart';

/// Der Pixel in der Mitte von [pixmap], als (a, r, g, b).
(int, int, int, int) centre(TrayPixmap pixmap) {
  final int index =
      ((pixmap.height ~/ 2) * pixmap.width + pixmap.width ~/ 2) * 4;
  final Uint8List bytes = pixmap.argb;
  return (bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]);
}

void main() {
  group('the_label_says_what_fits', () {
    test('idle carries none, held carries the digit', () {
      expect(trayIconLabel(TrayIconState.idle, 0), '');
      expect(trayIconLabel(TrayIconState.held, 1), '1');
      expect(trayIconLabel(TrayIconState.held, 9), '9');
    });

    test('ten and more collapse to 9+', () {
      expect(trayIconLabel(TrayIconState.held, 10), '9+');
      expect(trayIconLabel(TrayIconState.held, 128), '9+');
    });

    test('offline is a question mark, never a stale number', () {
      expect(trayIconLabel(TrayIconState.offline, 7), '?');
    });
  });

  group('the_colour_is_the_state_colour', () {
    test('one hue per state, straight from the tokens', () {
      expect(trayIconColor(TrayIconState.idle), HColors.fg2);
      expect(trayIconColor(TrayIconState.held), HColors.held);
      expect(trayIconColor(TrayIconState.alert), HColors.blocked);
      expect(trayIconColor(TrayIconState.offline), HColors.timedOut);
    });

    test('only the two states that want attention are filled', () {
      expect(trayIconIsFilled(TrayIconState.held), isTrue);
      expect(trayIconIsFilled(TrayIconState.alert), isTrue);
      expect(trayIconIsFilled(TrayIconState.idle), isFalse);
      expect(trayIconIsFilled(TrayIconState.offline), isFalse);
    });
  });

  testWidgets('the_icon_is_argb_in_both_sizes', (WidgetTester tester) async {
    late List<TrayPixmap> pixmaps;
    await tester.runAsync(() async {
      pixmaps = await renderTrayIcon(state: TrayIconState.held, count: 0);
    });

    expect(pixmaps.map((TrayPixmap p) => p.width).toList(), <int>[22, 44]);
    for (final TrayPixmap pixmap in pixmaps) {
      expect(pixmap.argb.length, pixmap.width * pixmap.height * 4);
    }
    // Alpha first, then the held amber: a chip that fills its own area.
    final (int a, int r, int g, int b) = centre(pixmaps.first);
    expect(a, 255);
    expect(r, (HColors.held.r * 255).round());
    expect(g, (HColors.held.g * 255).round());
    expect(b, (HColors.held.b * 255).round());
  });

  testWidgets('an_outlined_icon_leaves_its_middle_empty', (
    WidgetTester tester,
  ) async {
    late List<TrayPixmap> pixmaps;
    await tester.runAsync(() async {
      pixmaps = await renderTrayIcon(
        state: TrayIconState.idle,
        count: 0,
        sizes: const <int>[22],
      );
    });

    final (int a, _, _, _) = centre(pixmaps.single);
    expect(a, 0, reason: 'nothing waits, so nothing is saturated');
  });
}
