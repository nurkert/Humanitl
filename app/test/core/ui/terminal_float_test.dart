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
  bool chosen = false,
}) => placeFloatingTerminal(
  region: within,
  geometry: TerminalFloatGeometry(anchor: anchor, size: size, chosen: chosen),
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
      expect(placed!.rect.width, lessThanOrEqualTo(region.width * 0.8));
      expect(placed.rect.height, lessThanOrEqualTo(region.height * 0.8));
    });

    // --- #129 ----------------------------------------------------------------

    test('a_chosen_anchor_stands_over_a_region_to_keep_clear', () {
      // Die Leiste mit Freigeben und Blockieren unten in der Mitte: Wer das
      // Fenster dorthin gelegt hat, behält es dort, in seiner Größe.
      const Rect actionBar = Rect.fromLTWH(400, 700, 700, 100);
      final FloatPlacement? placed = place(
        anchor: FloatAnchor.bottomCenter,
        keepClear: <Rect>[actionBar],
        chosen: true,
      );
      expect(placed?.anchor, FloatAnchor.bottomCenter);
      expect(placed?.rect.size, terminalFloatDefaultSize);
      expect(placed!.rect.overlaps(actionBar), isTrue);
    });

    test('the_untouched_default_still_gives_way', () {
      const Rect actionBar = Rect.fromLTWH(400, 700, 700, 100);
      final FloatPlacement? placed = place(
        anchor: FloatAnchor.bottomCenter,
        keepClear: <Rect>[actionBar],
      );
      expect(placed?.anchor, isNot(FloatAnchor.bottomCenter));
      expect(placed!.rect.overlaps(actionBar), isFalse);
    });

    test('a_chosen_size_too_large_for_the_region_still_shrinks', () {
      final FloatPlacement? placed = place(
        size: const Size(480, 300),
        within: const Rect.fromLTWH(0, 0, 400, 260),
        chosen: true,
      );
      // 80 Prozent von 400 x 260.
      expect(placed?.rect.size, const Size(320, 208));
      expect(
        place(within: const Rect.fromLTWH(0, 0, 300, 180), chosen: true),
        isNull,
      );
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

  group('clampTerminalFloatSize', () {
    test('the_largest_size_is_the_share_of_the_region', () {
      expect(
        clampTerminalFloatSize(const Size(5000, 5000), region: region.size),
        terminalFloatMaxIn(region.size),
      );
      // #129: 80 Prozent, damit das Terminal neben der Warteschlange lesbar
      // breit wird.
      expect(terminalFloatMaxIn(region.size), const Size(1200, 640));
    });

    test('shrinking_stops_at_the_minimum', () {
      expect(
        clampTerminalFloatSize(const Size(-5000, -5000), region: region.size),
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
      expect(stored[terminalFloatChosenKey], isTrue);
      expect(
        second.read(terminalFloatProvider),
        const TerminalFloatGeometry(
          anchor: FloatAnchor.topLeft,
          size: Size(600, 400),
          chosen: true,
        ),
      );
    });

    test('a_drop_at_the_default_anchor_is_a_choice_too', () {
      final (ProviderContainer container, String path) = containerWith(null);
      container
          .read(terminalFloatProvider.notifier)
          .snapTo(FloatAnchor.bottomRight);
      expect(container.read(terminalFloatProvider).chosen, isTrue);
      final Map<String, Object?> stored =
          jsonDecode(File(path).readAsStringSync()) as Map<String, Object?>;
      expect(stored[terminalFloatChosenKey], isTrue);
    });

    test('a_stored_geometry_from_before_129_counts_as_chosen', () {
      final (ProviderContainer moved, String _) = containerWith(
        <String, Object?>{terminalFloatCornerKey: 'topLeft'},
      );
      expect(moved.read(terminalFloatProvider).chosen, isTrue);
      final (ProviderContainer sized, String _) = containerWith(
        <String, Object?>{
          terminalFloatSizeKey: <double>[600, 400],
        },
      );
      expect(sized.read(terminalFloatProvider).chosen, isTrue);
      final (ProviderContainer untouched, String _) = containerWith(
        <String, Object?>{terminalFloatCornerKey: 'bottomRight'},
      );
      expect(untouched.read(terminalFloatProvider).chosen, isFalse);
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
