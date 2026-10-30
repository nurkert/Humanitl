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
  FloatAnchor anchor = FloatAnchor.bottomRight,
  Size size = terminalFloatDefaultSize,
  List<Rect> keepClear = const <Rect>[],
  Rect within = region,
}) => placeFloatingTerminal(
  region: within,
  geometry: TerminalFloatGeometry(anchor: anchor, size: size),
  keepClear: keepClear,
  inset: inset,
);

void main() {
  group('placeFloatingTerminal', () {
    test('the_chosen_corner_at_the_chosen_size_when_nothing_is_in_the_way', () {
      final FloatPlacement? placed = place();
      expect(placed?.anchor, FloatAnchor.bottomRight);
      expect(
        placed?.rect,
        const Rect.fromLTWH(1500 - 12 - 480, 800 - 12 - 300, 480, 300),
      );
    });

    test('the_default_corner_is_bottom_right', () {
      expect(const TerminalFloatGeometry().anchor, FloatAnchor.bottomRight);
    });

    test('a_region_to_keep_clear_in_the_corner_shrinks_it_first', () {
      // Eine Aktionsleiste links von der Ecke: 480 breit stößt an, 320 nicht.
      const Rect actionBar = Rect.fromLTWH(400, 700, 700, 100);
      final FloatPlacement? placed = place(keepClear: <Rect>[actionBar]);
      expect(placed?.anchor, FloatAnchor.bottomRight);
      expect(placed?.rect.size, terminalFloatMinSize);
      expect(placed!.rect.overlaps(actionBar), isFalse);
    });

    test('a_blocked_corner_gives_way_to_the_next_free_one', () {
      // Die ganze rechte Hälfte unten ist besetzt, auch für die kleinste
      // Größe: dann die Ecke daneben, unten links.
      const Rect blocked = Rect.fromLTWH(700, 400, 800, 400);
      final FloatPlacement? placed = place(keepClear: <Rect>[blocked]);
      expect(placed?.anchor, FloatAnchor.bottomLeft);
      expect(placed!.rect.overlaps(blocked), isFalse);
    });

    test('the_order_is_beside_then_above_then_across', () {
      // Unten links und unten rechts besetzt: dann oben rechts.
      const List<Rect> bottom = <Rect>[Rect.fromLTWH(0, 400, 1500, 400)];
      expect(place(keepClear: bottom)?.anchor, FloatAnchor.topRight);
      // Dazu die rechte Hälfte oben: bleibt oben links.
      const List<Rect> more = <Rect>[
        Rect.fromLTWH(0, 400, 1500, 400),
        Rect.fromLTWH(700, 0, 800, 400),
      ];
      expect(place(keepClear: more)?.anchor, FloatAnchor.topLeft);
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

  // --- HUM-352 ---------------------------------------------------------------

  group('FloatAnchor', () {
    /// Das Fenster in Vorgabegröße an [anchor].
    Rect at(FloatAnchor anchor) =>
        terminalFloatRectAt(region, anchor, terminalFloatDefaultSize, inset);

    test('nine_anchors_on_a_grid_of_three', () {
      expect(FloatAnchor.values, hasLength(9));
      expect(
        FloatAnchor.values.map((FloatAnchor a) => (a.x, a.y)).toSet(),
        hasLength(9),
      );
    });

    test('every_anchor_lies_inside_the_room_with_the_same_inset', () {
      final Rect room = region.deflate(inset);
      for (final FloatAnchor anchor in FloatAnchor.values) {
        final Rect rect = at(anchor);
        expect(room.contains(rect.topLeft), isTrue, reason: '$anchor');
        expect(room.contains(rect.bottomRight - const Offset(1, 1)), isTrue);
      }
      expect(at(FloatAnchor.center).center, region.center);
      expect(at(FloatAnchor.topCenter).top, inset);
      expect(at(FloatAnchor.bottomCenter).bottom, region.bottom - inset);
      expect(at(FloatAnchor.centerLeft).left, inset);
      expect(at(FloatAnchor.centerRight).right, region.right - inset);
    });

    test('a_calm_drop_takes_the_nearest_anchor', () {
      for (final FloatAnchor anchor in FloatAnchor.values) {
        // Ein Stück daneben losgelassen, ohne Schwung.
        final Rect dropped = at(anchor).shift(const Offset(30, -20));
        expect(
          FloatAnchor.release(region, dropped, inset, const Offset(100, 0)),
          anchor,
          reason: 'below the fling speed',
        );
      }
    });

    test('a_throw_takes_the_neighbour_in_its_direction', () {
      // Von unten rechts kurz nach links geworfen: unten Mitte, nicht zurück.
      final Rect barely = at(FloatAnchor.bottomRight)
          .shift(const Offset(-40, 0));
      expect(
        FloatAnchor.release(region, barely, inset, const Offset(-1500, 0)),
        FloatAnchor.bottomCenter,
      );
      expect(
        FloatAnchor.release(region, barely, inset, const Offset(0, -1500)),
        FloatAnchor.centerRight,
      );
      expect(
        FloatAnchor.release(region, barely, inset, const Offset(-1000, -1000)),
        FloatAnchor.center,
      );
      // Gegen den Rand wirft es nicht über das Raster hinaus.
      expect(
        FloatAnchor.release(region, barely, inset, const Offset(1500, 0)),
        FloatAnchor.bottomRight,
      );
    });

    test('a_middle_falls_back_to_the_corners_from_bottom_right', () {
      // Die Mitte ist besetzt: Ausweichen in die Ecken, nie in eine andere
      // Mitte.
      final FloatPlacement? placed = place(
        anchor: FloatAnchor.center,
        keepClear: <Rect>[at(FloatAnchor.center)],
      );
      expect(placed?.anchor, FloatAnchor.bottomRight);
      expect(
        place(anchor: FloatAnchor.topCenter)?.anchor,
        FloatAnchor.topCenter,
      );
    });

    test('the_grip_sits_across_from_the_anchor', () {
      expect(
        (FloatAnchor.bottomRight.gripX, FloatAnchor.bottomRight.gripY),
        (-1, -1),
      );
      expect(
        (FloatAnchor.bottomLeft.gripX, FloatAnchor.bottomLeft.gripY),
        (1, -1),
      );
      expect((FloatAnchor.topLeft.gripX, FloatAnchor.topLeft.gripY), (1, 1));
      expect((FloatAnchor.center.gripX, FloatAnchor.center.gripY), (1, 1));
      expect(
        (FloatAnchor.bottomCenter.gripX, FloatAnchor.bottomCenter.gripY),
        (1, -1),
      );
    });
  });

  group('growTerminalFloat', () {
    Size grow(
      FloatAnchor anchor,
      Size by, {
      List<Rect> keepClear = const <Rect>[],
    }) => growTerminalFloat(
      region: region,
      anchor: anchor,
      from: terminalFloatDefaultSize,
      by: by,
      keepClear: keepClear,
      inset: inset,
    );

    test('the_largest_size_is_the_same_at_every_anchor', () {
      final Set<Size> largest = <Size>{
        for (final FloatAnchor anchor in FloatAnchor.values)
          grow(anchor, const Size(5000, 5000)),
      };
      expect(largest, <Size>{terminalFloatMaxIn(region.size)});
    });

    test('it_stops_at_the_edge_of_a_region_to_keep_clear', () {
      // Eine Leiste links neben dem Fenster unten rechts; ein großer Schritt
      // endet an ihrer Kante statt vor ihr.
      final Rect window = terminalFloatRectAt(
        region,
        FloatAnchor.bottomRight,
        terminalFloatDefaultSize,
        inset,
      );
      final Rect bar = Rect.fromLTRB(0, 700, window.left - 100, 800);
      final Size grown = grow(
        FloatAnchor.bottomRight,
        const Size(300, 0),
        keepClear: <Rect>[bar],
      );
      expect(grown.width, closeTo(terminalFloatDefaultSize.width + 100, 0.1));
      final Rect after = terminalFloatRectAt(
        region,
        FloatAnchor.bottomRight,
        grown,
        inset,
      );
      expect(after.overlaps(bar), isFalse);
    });

    test('shrinking_is_always_taken_down_to_the_minimum', () {
      expect(
        grow(FloatAnchor.topLeft, const Size(-5000, -5000)),
        terminalFloatMinSize,
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
        ..snapTo(FloatAnchor.topLeft)
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
          anchor: FloatAnchor.topLeft,
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
      FloatAnchor anchor = FloatAnchor.bottomRight,
      List<Rect> keepClear = const <Rect>[],
      Rect within = region,
    }) => placeTerminalStrip(
      region: within,
      anchor: anchor,
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
      expect(placed?.anchor, FloatAnchor.bottomRight);
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
        placeStrip(keepClear: <Rect>[bottomRight])?.anchor,
        FloatAnchor.bottomLeft,
      );
      expect(
        placeStrip(
          anchor: FloatAnchor.topLeft,
          keepClear: const <Rect>[Rect.fromLTWH(0, 0, 400, 100)],
        )?.anchor,
        FloatAnchor.topRight,
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
