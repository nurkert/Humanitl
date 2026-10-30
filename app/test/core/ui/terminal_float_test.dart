// Das schwebende Terminal als Rechnung ohne Widget (HUM-300): welche Ecke,
// welche Größe, und nie über einem Bereich, der frei bleiben muss. Dazu die
// Platzierung als Notiz im Kern und das Gedächtnis in der Zustandsdatei.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/terminal_float.dart';
import 'package:humanitl/core/ui/terminal_placement.dart';
import 'package:humanitl/core/ui_state/ui_state_file.dart';

/// Ein Container über einer Zustandsdatei mit [content], oder ohne Datei.
(ProviderContainer, String) containerWith(Object? content) {
  final Directory dir = Directory.systemTemp.createTempSync('humanitl-float');
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

/// Die Fläche der Abschnitte bei einem Fenster von 1600 x 900.
const Rect region = Rect.fromLTWH(0, 0, 1500, 800);

const double inset = 12;

FloatPlacement? place({
  FloatCorner corner = FloatCorner.bottomRight,
  Size size = terminalFloatDefaultSize,
  List<Rect> keepClear = const <Rect>[],
  Rect within = region,
}) => placeFloatingTerminal(
  region: within,
  geometry: TerminalFloatGeometry(corner: corner, size: size),
  keepClear: keepClear,
  inset: inset,
);

void main() {
  group('placeFloatingTerminal', () {
    test('the_chosen_corner_at_the_chosen_size_when_nothing_is_in_the_way', () {
      final FloatPlacement? placed = place();
      expect(placed?.corner, FloatCorner.bottomRight);
      expect(
        placed?.rect,
        const Rect.fromLTWH(1500 - 12 - 480, 800 - 12 - 300, 480, 300),
      );
    });

    test('the_default_corner_is_bottom_right', () {
      expect(const TerminalFloatGeometry().corner, FloatCorner.bottomRight);
    });

    test('a_region_to_keep_clear_in_the_corner_shrinks_it_first', () {
      // Eine Aktionsleiste links von der Ecke: 480 breit stößt an, 320 nicht.
      const Rect actionBar = Rect.fromLTWH(400, 700, 700, 100);
      final FloatPlacement? placed = place(keepClear: <Rect>[actionBar]);
      expect(placed?.corner, FloatCorner.bottomRight);
      expect(placed?.rect.size, terminalFloatMinSize);
      expect(placed!.rect.overlaps(actionBar), isFalse);
    });

    test('a_blocked_corner_gives_way_to_the_next_free_one', () {
      // Die ganze rechte Hälfte unten ist besetzt, auch für die kleinste
      // Größe: dann die Ecke daneben, unten links.
      const Rect blocked = Rect.fromLTWH(700, 400, 800, 400);
      final FloatPlacement? placed = place(keepClear: <Rect>[blocked]);
      expect(placed?.corner, FloatCorner.bottomLeft);
      expect(placed!.rect.overlaps(blocked), isFalse);
    });

    test('the_order_is_beside_then_above_then_across', () {
      // Unten links und unten rechts besetzt: dann oben rechts.
      const List<Rect> bottom = <Rect>[Rect.fromLTWH(0, 400, 1500, 400)];
      expect(place(keepClear: bottom)?.corner, FloatCorner.topRight);
      // Dazu die rechte Hälfte oben: bleibt oben links.
      const List<Rect> more = <Rect>[
        Rect.fromLTWH(0, 400, 1500, 400),
        Rect.fromLTWH(700, 0, 800, 400),
      ];
      expect(place(keepClear: more)?.corner, FloatCorner.topLeft);
    });

    test('nowhere_free_means_no_place_at_all', () {
      // Ein Modal über der ganzen Fläche.
      expect(place(keepClear: const <Rect>[region]), isNull);
    });

    test('a_region_too_small_for_the_smallest_size_means_no_place', () {
      expect(place(within: const Rect.fromLTWH(0, 0, 300, 180)), isNull);
    });

    test('a_region_that_only_touches_does_not_block', () {
      final Rect placed = place()!.rect;
      final Rect touching = Rect.fromLTRB(0, 0, placed.left, region.bottom);
      expect(place(keepClear: <Rect>[touching])?.rect, placed);
    });

    test('the_size_never_exceeds_the_share_of_the_region', () {
      final FloatPlacement? placed = place(size: const Size(5000, 5000));
      expect(placed!.rect.width, lessThanOrEqualTo(region.width * 0.6));
      expect(placed.rect.height, lessThanOrEqualTo(region.height * 0.6));
    });
  });

  group('FloatCorner.nearest', () {
    test('the_quadrant_decides', () {
      expect(
        FloatCorner.nearest(region, const Offset(10, 10)),
        FloatCorner.topLeft,
      );
      expect(
        FloatCorner.nearest(region, const Offset(1400, 10)),
        FloatCorner.topRight,
      );
      expect(
        FloatCorner.nearest(region, const Offset(10, 700)),
        FloatCorner.bottomLeft,
      );
      expect(
        FloatCorner.nearest(region, const Offset(1400, 700)),
        FloatCorner.bottomRight,
      );
    });
  });

  group('TerminalFloat', () {
    test('without_a_file_the_default', () {
      final (ProviderContainer container, String _) = containerWith(null);
      expect(
        container.read(terminalFloatProvider),
        const TerminalFloatGeometry(),
      );
    });

    test('corner_and_size_survive_a_restart', () {
      final (ProviderContainer first, String path) = containerWith(null);
      first.read(terminalFloatProvider.notifier)
        ..snapTo(FloatCorner.topLeft)
        ..resize(const Size(600, 400));
      final Map<String, Object?> stored =
          jsonDecode(File(path).readAsStringSync()) as Map<String, Object?>;
      expect(stored[terminalFloatCornerKey], 'topLeft');
      expect(stored[terminalFloatSizeKey], <double>[600, 400]);

      final ProviderContainer second = ProviderContainer(
        overrides: [uiStateFileProvider.overrideWithValue(UiStateFile(path))],
      );
      addTearDown(second.dispose);
      expect(
        second.read(terminalFloatProvider),
        const TerminalFloatGeometry(
          corner: FloatCorner.topLeft,
          size: Size(600, 400),
        ),
      );
    });

    test('a_size_below_the_minimum_is_raised_to_it', () {
      final (ProviderContainer container, String _) = containerWith(null);
      container.read(terminalFloatProvider.notifier).resize(const Size(10, 10));
      expect(container.read(terminalFloatProvider).size, terminalFloatMinSize);
    });

    test('garbage_in_the_file_falls_back', () {
      final (ProviderContainer container, String _) = containerWith(
        <String, Object?>{
          terminalFloatCornerKey: 'middle',
          terminalFloatSizeKey: <Object?>['wide', 3],
        },
      );
      expect(
        container.read(terminalFloatProvider),
        const TerminalFloatGeometry(),
      );
    });
  });

  group('TerminalPlacementNotifier', () {
    test('float_and_dock', () {
      final (ProviderContainer container, String _) = containerWith(null);
      final TerminalPlacementNotifier placement = container.read(
        terminalPlacementProvider.notifier,
      );
      placement.toggleFloating();
      expect(
        container.read(terminalPlacementProvider),
        TerminalPlacement.floating,
      );
      placement.toggleFloating();
      expect(container.read(terminalPlacementProvider), TerminalPlacement.pane);
      // Zurücklegen, was nicht schwebt, tut nichts.
      placement.dock();
      expect(container.read(terminalPlacementProvider), TerminalPlacement.pane);
    });

    test('the_whole_window_returns_to_the_floating_window', () {
      final (ProviderContainer container, String _) = containerWith(null);
      final TerminalPlacementNotifier placement = container.read(
        terminalPlacementProvider.notifier,
      );
      placement
        ..float()
        ..toggleFullWindow();
      expect(
        container.read(terminalPlacementProvider),
        TerminalPlacement.fullWindow,
      );
      placement.leaveFullWindow();
      expect(
        container.read(terminalPlacementProvider),
        TerminalPlacement.floating,
      );

      // Aus dem Pane kommt das ganze Fenster wie bisher ins Pane zurück.
      placement
        ..dock()
        ..toggleFullWindow()
        ..toggleFullWindow();
      expect(container.read(terminalPlacementProvider), TerminalPlacement.pane);
    });
  });

  // --- HUM-315 ---------------------------------------------------------------

  group('placeTerminalStrip', () {
    const Size strip = Size(240, 28);

    FloatPlacement? placeStrip({
      FloatCorner corner = FloatCorner.bottomRight,
      List<Rect> keepClear = const <Rect>[],
      Rect within = region,
    }) => placeTerminalStrip(
      region: within,
      corner: corner,
      size: strip,
      keepClear: keepClear,
      inset: inset,
    );

    test('a_corner_too_small_for_the_window_still_takes_the_strip', () {
      // Rechts bleiben 300 x 800 frei: zu schmal für 320 x 200 und den
      // Rand, breit genug für den Streifen.
      const Rect taken = Rect.fromLTWH(0, 0, 1200, 800);
      expect(place(keepClear: <Rect>[taken]), isNull);
      final FloatPlacement? placed = placeStrip(keepClear: <Rect>[taken]);
      expect(placed?.corner, FloatCorner.bottomRight);
      expect(
        placed?.rect,
        const Rect.fromLTWH(1500 - 12 - 240, 800 - 12 - 28, 240, 28),
      );
      expect(placed!.rect.overlaps(taken), isFalse);
    });

    test('the_strip_takes_the_corners_in_the_same_order', () {
      // Unten rechts besetzt: der Streifen geht wie das Fenster daneben.
      const Rect bottomRight = Rect.fromLTWH(1000, 600, 500, 200);
      expect(
        placeStrip(keepClear: <Rect>[bottomRight])?.corner,
        FloatCorner.bottomLeft,
      );
      expect(
        placeStrip(
          corner: FloatCorner.topLeft,
          keepClear: const <Rect>[Rect.fromLTWH(0, 0, 400, 100)],
        )?.corner,
        FloatCorner.topRight,
      );
    });

    test('nothing_free_even_for_the_strip_means_no_place', () {
      expect(placeStrip(keepClear: <Rect>[region]), isNull);
    });
  });

  group('terminalHostFor', () {
    test('a_floating_terminal_docks_on_the_sandbox_page_only', () {
      expect(
        terminalHostFor(TerminalPlacement.floating, sandboxPageShown: true),
        TerminalPlacement.pane,
      );
      expect(
        terminalHostFor(TerminalPlacement.floating, sandboxPageShown: false),
        TerminalPlacement.floating,
      );
    });

    test('pane_and_whole_window_do_not_depend_on_the_page', () {
      for (final bool page in <bool>[true, false]) {
        expect(
          terminalHostFor(TerminalPlacement.pane, sandboxPageShown: page),
          TerminalPlacement.pane,
        );
        expect(
          terminalHostFor(TerminalPlacement.fullWindow, sandboxPageShown: page),
          TerminalPlacement.fullWindow,
        );
      }
    });

    test('the_page_never_changes_the_choice', () {
      final (ProviderContainer container, String _) = containerWith(null);
      container.read(terminalPlacementProvider.notifier).float();
      container.read(sandboxPageShownProvider.notifier).set(shown: true);
      expect(container.read(terminalHostProvider), TerminalPlacement.pane);
      expect(
        container.read(terminalPlacementProvider),
        TerminalPlacement.floating,
      );
      container.read(sandboxPageShownProvider.notifier).set(shown: false);
      expect(container.read(terminalHostProvider), TerminalPlacement.floating);
    });
  });
}
