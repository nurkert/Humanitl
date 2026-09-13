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

/// Die gemalte Fläche eines Segments: der Kasten mit Füllung und Hairline,
/// nicht die Trefferfläche darum. Zwischen zwei gemalten Kästen liegt der
/// Raum, um den es in HUM-143 geht.
Rect paintedBox(WidgetTester tester, String label) => tester.getRect(
  find.ancestor(of: find.text(label), matching: find.byType(AnimatedContainer)),
);

/// Die Trefferfläche eines Segments: der Detektor, den ein Zeiger trifft.
Rect hitBox(WidgetTester tester, String label) => tester.getRect(
  find.ancestor(of: find.text(label), matching: find.byType(GestureDetector)),
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

  testWidgets('a_tap_between_two_segments_reaches_a_segment', (
    WidgetTester tester,
  ) async {
    // **Gemessen wird die Lücke, nicht das Segment.** Die Messungen darüber
    // tippen in die Mitte einer Zelle und bleiben grün, während zwischen zwei
    // Zellen 4 px liegen, in denen ein Klick nichts tut: `FocusRing` legte
    // seine Reserve außerhalb des Detektors, also endete `Once` bei 352,5 und
    // `Session` begann bei 356,5 (Review am 2026-09-07, drei Taps bei 353,0,
    // 354,5 und 356,0 erreichten niemanden). Was aussieht wie eine
    // zusammenhängende Fläche, ist eine -- sonst schluckt ein sichtbar
    // durchgehendes Bedienelement den Klick, und das ist die Stille, die
    // `docs/UX.md` 5.3 verbietet.
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

    final Rect once = paintedBox(tester, 'Once');
    final Rect session = paintedBox(tester, 'Session');
    final double band = session.left - once.right;
    // Ohne diese Zusicherung wäre der Test wertlos: Läge kein Raum zwischen
    // den gemalten Kästen, tippte er in ein Segment und bliebe immer grün.
    expect(
      band,
      greaterThan(0),
      reason:
          'Once endet bei ${once.right}, Session beginnt bei $band px '
          'weiter rechts -- ohne Raum dazwischen misst dieser Test nichts',
    );

    final double y = once.center.dy;
    for (final double x in <double>[
      once.right + 0.5,
      once.right + band / 2,
      session.left - 0.5,
    ]) {
      chosen = null;
      await tester.tapAt(Offset(x, y));
      await tester.pump();
      expect(
        chosen,
        anyOf(RememberDuration.once, RememberDuration.session),
        reason:
            'ein Tap bei ($x, $y), zwischen Once (bis ${once.right}) und '
            'Session (ab ${session.left}), erreicht kein Segment',
      );
    }
  });

  testWidgets('a_tap_on_the_upper_and_lower_edge_of_a_segment_reaches_it', (
    WidgetTester tester,
  ) async {
    // Dieselbe Frage senkrecht: Oben und unten reserviert der Ring je 2 px,
    // und sie gehörten bis HUM-143 zu niemandem. Die Zeile darunter misst den
    // Raum zwischen zwei Segmenten, die untereinander stehen.
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

    final Rect box = paintedBox(tester, 'Once');
    final Rect hit = hitBox(tester, 'Once');
    expect(
      hit.top,
      lessThan(box.top),
      reason:
          'über dem gemalten Kasten (${box.top}) liegt keine Reserve; '
          'die Trefferfläche beginnt bei ${hit.top}',
    );

    for (final double y in <double>[box.top - 0.5, box.bottom + 0.5]) {
      chosen = null;
      await tester.tapAt(Offset(box.center.dx, y));
      await tester.pump();
      expect(
        chosen,
        RememberDuration.once,
        reason:
            'ein Tap bei (${box.center.dx}, $y), am waagerechten Rand von '
            'Once (${box.top} bis ${box.bottom}), erreicht das Segment nicht',
      );
    }
  });

  testWidgets('a_tap_between_two_rows_of_segments_reaches_a_segment', (
    WidgetTester tester,
  ) async {
    // Bricht die Gruppe in zwei Zeilen um -- schmale Leiste, große Schrift --,
    // stehen zwei Segmente übereinander, und zwischen ihnen lag dieselbe
    // 4-px-Lücke wie nebeneinander (`_Segments` ist ein `Wrap`).
    RememberDuration? chosen;
    await pumpControl(
      tester,
      SizedBox(
        width: 140,
        child: RememberGrid(
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
      ),
    );

    final List<Rect> boxes = <Rect>[
      for (final String label in durationLabels) paintedBox(tester, label),
    ];
    Rect? upper;
    Rect? lower;
    for (final Rect a in boxes) {
      for (final Rect b in boxes) {
        if (b.top < a.bottom) {
          continue;
        }
        final double overlap =
            (a.right < b.right ? a.right : b.right) -
            (a.left > b.left ? a.left : b.left);
        if (overlap > 0 && (upper == null || b.top - a.bottom > 0)) {
          upper ??= a;
          lower ??= b;
        }
      }
    }
    expect(
      upper,
      isNotNull,
      reason: 'bei 140 px Breite bricht die Gruppe nicht um: $boxes',
    );
    final double band = lower!.top - upper!.bottom;
    expect(
      band,
      greaterThan(0),
      reason: 'zwischen den beiden Zeilen liegt kein Raum ($upper, $lower)',
    );

    final double x =
        ((upper.left > lower.left ? upper.left : lower.left) +
            (upper.right < lower.right ? upper.right : lower.right)) /
        2;
    for (final double y in <double>[
      upper.bottom + 0.5,
      upper.bottom + band / 2,
      lower.top - 0.5,
    ]) {
      chosen = null;
      await tester.tapAt(Offset(x, y));
      await tester.pump();
      expect(
        chosen,
        isNotNull,
        reason:
            'ein Tap bei ($x, $y), zwischen den Zeilen '
            '(${upper.bottom} bis ${lower.top}), erreicht kein Segment',
      );
    }
  });
}
