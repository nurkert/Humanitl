// Die Trefferflächen der Aktionsleiste (HUM-028).
//
// **Warum diese Datei neben `release_valve_test.dart` steht.** Dort ist das
// Entscheidungspaar gemessen: Erlauben und Blockieren sind größer als alles
// andere (`HSize.hitDecision`, 32 mal 120). Für das Nebensächliche gilt eine
// Untergrenze, `HSize.hitMin` mit 28 px, und die stand bis zum 2026-09-07 nur
// im Code: `remember_grid.dart` setzt sie als `BoxConstraints`, der Chevron
// der Ventilzeile als Breite. Eine Zahl, die keine Messung hat, ist eine
// Absicht und keine Zusage -- und `docs/UX.md` 5.4 macht daraus eine Zusage.
//
// Gemessen wird die Fläche, die ein Zeiger wirklich trifft, also die Größe des
// Elements im Baum und nicht die Zahl in seinen Constraints.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/rule_sentence.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:humanitl/features/intercept/widgets/remember_grid.dart';

Future<void> pumpControl(WidgetTester tester, Widget child) async {
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: HTheme.dark(child: Center(child: child)),
    ),
  );
}

/// Die Beschriftungen der Dauer-Gruppe, wörtlich die ausgelieferten
/// (`app/l10n/app_en.arb`).
///
/// **Wörtlich, und das hat einen Grund.** Die erste Fassung erfand kürzere
/// Namen, und der schmalste davon (`1h` statt `1 h`) war zugleich der, an dem
/// die Breite der Zelle hängt. Ein Test, der eigene Beschriftungen baut, misst
/// eine Fläche, die es im Programm nicht gibt.
///
/// **Vollständig, und das ist der Punkt.** Die erste Fassung dieser Liste hat
/// `1h` vergessen; `RememberDuration` hat vier Werte, gemessen waren drei --
/// und ein Kriterium, das „jedes klickbare Element" sagt, ist mit sieben von
/// acht nicht erfüllt. Gefunden im Review am 2026-09-07.
const List<String> durationLabels = <String>[
  'Once',
  'Session',
  '1 h',
  'Forever',
];

/// Die Beschriftungen der Zielgruppe, ebenfalls die ausgelieferten.
const List<String> targetLabels = <String>[
  'URL',
  'Host',
  'Domain',
  'Host + method',
];

/// Alle acht Zellen, in der Reihenfolge des Rasters.
const List<String> segments = <String>[...durationLabels, ...targetLabels];

/// Das Raster, wie die Aktionsleiste es baut.
RememberGrid grid({bool enabled = true}) => RememberGrid(
  heading: 'Remember',
  durationLabel: 'Duration',
  targetLabel: 'Scope',
  duration: RememberDuration.session,
  target: RememberTarget.host,
  durationLabels: durationLabels,
  targetLabels: targetLabels,
  enabled: enabled,
  onDuration: (RememberDuration _) {},
  onTarget: (RememberTarget _) {},
);

void main() {
  testWidgets(
    'every_segment_of_the_remember_grid_is_at_least_the_hit_minimum',
    (WidgetTester tester) async {
      await pumpControl(tester, grid());

      for (final String label in segments) {
        final Finder cell = find.ancestor(
          of: find.text(label),
          matching: find.byType(GestureDetector),
        );
        expect(cell, findsOneWidget, reason: 'the segment $label is one box');
        final Size size = tester.getSize(cell);
        expect(
          size.height,
          greaterThanOrEqualTo(HSize.hitMin),
          reason: 'the segment $label is $size',
        );
        expect(
          size.width,
          greaterThanOrEqualTo(HSize.hitMin),
          reason: 'the segment $label is $size',
        );
      }
    },
  );

  testWidgets('a_segment_without_a_word_is_still_wide_enough_to_hit', (
    WidgetTester tester,
  ) async {
    // **Die Breite hängt an einer Untergrenze, und die ist nur dann eine
    // Zusage, wenn sie auch trägt.** Mit den ausgelieferten Beschriftungen ist
    // die schmalste Zelle (`1 h`) schon 53 px breit; `minWidth` könnte
    // ersatzlos verschwinden, ohne dass ein Test es merkte -- gemessen im
    // Review am 2026-09-07. Ein Raster ohne Wörter zeigt die Untergrenze
    // selbst, und eine leere Beschriftung ist kein erfundener Fall: Sie ist
    // das, was von einer Übersetzung übrig bleibt, die jemand vergessen hat.
    await pumpControl(
      tester,
      RememberGrid(
        heading: 'Remember',
        durationLabel: 'Duration',
        targetLabel: 'Scope',
        duration: RememberDuration.session,
        target: RememberTarget.host,
        durationLabels: const <String>['', '', '', ''],
        targetLabels: const <String>['', '', '', ''],
        onDuration: (RememberDuration _) {},
        onTarget: (RememberTarget _) {},
      ),
    );

    final Iterable<Element> cells = find
        .descendant(
          of: find.byType(RememberGrid),
          matching: find.byType(GestureDetector),
        )
        .evaluate();
    expect(cells.length, segments.length, reason: 'all eight cells are there');
    for (final Element cell in cells) {
      final Size size = tester.getSize(
        find.byElementPredicate((Element e) => e == cell),
      );
      expect(
        size.width,
        greaterThanOrEqualTo(HSize.hitMin),
        reason: 'an empty segment is $size',
      );
      expect(size.height, greaterThanOrEqualTo(HSize.hitMin));
    }
  });

  testWidgets('a_segment_answers_a_tap', (WidgetTester tester) async {
    // Die drei Messungen darüber sind Geometrie; ohne diese hier bliebe die
    // Fläche eine Zahl ohne Wirkung. Gefunden im Review: `onTap: null` ließ
    // alle drei grün.
    RememberDuration? chosen;
    await pumpControl(
      tester,
      RememberGrid(
        heading: 'Remember',
        durationLabel: 'Duration',
        targetLabel: 'Scope',
        duration: RememberDuration.session,
        target: RememberTarget.host,
        durationLabels: durationLabels,
        targetLabels: targetLabels,
        onDuration: (RememberDuration duration) => chosen = duration,
        onTarget: (RememberTarget _) {},
      ),
    );

    await tester.tap(find.text('Forever'));
    await tester.pump();
    expect(chosen, RememberDuration.forever);
  });

  testWidgets('the_chevron_of_the_valve_is_at_least_the_hit_minimum', (
    WidgetTester tester,
  ) async {
    await pumpControl(
      tester,
      ReleaseValve(
        label: 'Allow',
        holdLabel: 'Allow for session',
        shortcutHint: 'Enter',
        semanticsValue: '4:59 left',
        optionsLabel: 'Duration and scope of the rule',
        onAllow: () {},
        onAllowRemembered: () {},
        onToggleOptions: () {},
      ),
    );

    final Size size = tester.getSize(
      find.byKey(const Key('intercept-valve-options')),
    );
    expect(
      size.height,
      greaterThanOrEqualTo(HSize.hitMin),
      reason: 'the chevron is $size',
    );
    expect(
      size.width,
      greaterThanOrEqualTo(HSize.hitMin),
      reason: 'the chevron is $size',
    );
  });

  testWidgets('a_disabled_grid_keeps_its_hit_targets', (
    WidgetTester tester,
  ) async {
    // Ausgegraut heißt nicht kleiner: Wer mit dem Zeiger daneben trifft, soll
    // dieselbe Fläche haben wie vorher, sonst wandert das Ziel unter dem
    // Finger weg, sobald eine Entscheidung läuft (`docs/UX.md` 5.4).
    await pumpControl(tester, grid(enabled: false));

    // Alle acht und nicht nur eine: Die Zielgruppe wird über einen eigenen
    // Weg gesperrt (`scopeEnabled` in `remember_grid.dart`), und eine Messung
    // an einer Dauer-Zelle sagte darüber nichts.
    for (final String label in segments) {
      final Finder cell = find.ancestor(
        of: find.text(label),
        matching: find.byType(GestureDetector),
      );
      final Size size = tester.getSize(cell);
      expect(
        size.height,
        greaterThanOrEqualTo(HSize.hitMin),
        reason: 'the disabled segment $label is $size',
      );
      expect(
        size.width,
        greaterThanOrEqualTo(HSize.hitMin),
        reason: 'the disabled segment $label is $size',
      );
    }
  });
}
