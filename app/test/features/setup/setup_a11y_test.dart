// Die vier Ausnahmen aus `docs/UX.md` 6, soweit sie diesen Bildschirm treffen
// (HUM-044).
//
// Barrierefreiheit als Programm ist bis 1.0.0 vertagt; vier Punkte gelten
// weiter, weil sie nur wie Barrierefreiheit aussehen und in Wahrheit
// Sicherheit oder schlichte Benutzbarkeit sind. Auf dem Setup-Bildschirm
// treffen drei davon zu, und der vierte nicht:
//
//  1. Die volle Haltedauer: **trifft nicht zu.** Hier gibt es kein
//     Halten-zum-Bestaetigen; der Start ist umkehrbar (stoppen), und die
//     unumkehrbare Haelfte der Tastatur liegt in der Warteschlange.
//  2. Trefferflaechen: mindestens 28 x 28 px allgemein. Der Start-Knopf ist
//     keine Entscheidung ueber eine Anfrage, also gilt die groessere Flaeche
//     von 32 x 120 px hier nicht -- aber die allgemeine schon, und sie wird
//     gemessen und nicht behauptet.
//  3. Text ueber etwa 3:1: jedes Wort dieses Bildschirms.
//  4. Ein Klick zeigt eine sichtbare Reaktion: der Start-Knopf ist ein
//     `HButton` und faellt damit unter die Messung des Designsystems; hier
//     steht die Zusage, dass ein **gesperrter** Knopf trotzdem sagt, warum.

import 'dart:math' as math;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';

import '../../harness/app_harness.dart';

/// Die Mindestkantenlaenge einer Trefferflaeche (`docs/UX.md` 6).
const double hitMin = HSize.hitMin;

/// Der Kontrast zweier Farben nach WCAG.
double contrast(Color a, Color b) {
  double channel(double value) => value <= 0.03928
      ? value / 12.92
      : math.pow((value + 0.055) / 1.055, 2.4).toDouble();
  double luminance(Color c) =>
      0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b);
  final double first = luminance(a);
  final double second = luminance(b);
  final double light = first > second ? first : second;
  final double dark = first > second ? second : first;
  return (light + 0.05) / (dark + 0.05);
}

void main() {
  testWidgets('every control of the screen is at least 28 by 28', (
    WidgetTester tester,
  ) async {
    await pumpApp(
      tester,
      client: FakeDaemonClient()..doctorReport = fakeDoctorOk(),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    for (final Key key in <Key>[
      const Key('setup-start'),
      const Key('setup-sandbox-recheck'),
      const Key('setup-llm-probe'),
      const Key('setup-workdir'),
    ]) {
      final Size size = tester.getSize(find.byKey(key));
      expect(size.height, greaterThanOrEqualTo(hitMin), reason: '$key height');
      expect(size.width, greaterThanOrEqualTo(hitMin), reason: '$key width');
    }
  });

  testWidgets('a locked start button still says why', (
    WidgetTester tester,
  ) async {
    // Ein Control, das aus ist, sagt auf sich selbst warum (`docs/UX.md` 5.3).
    // Ohne diesen Satz waere der graue Knopf die ganze Auskunft.
    await pumpApp(tester, client: FakeDaemonClient.unavailable());

    final HButton start = tester.widget<HButton>(
      find.byKey(const Key('setup-start')),
    );
    expect(start.enabled, isFalse);
    final String reason = tester
        .widget<Text>(find.byKey(const Key('setup-start-reason')))
        .data!;
    expect(reason, isNotEmpty);
    expect(reason, contains('Background service'));
  });

  testWidgets('no row overflows, at normal size and at double', (
    WidgetTester tester,
  ) async {
    // `docs/UX.md` 6: bis `TextScaler.linear(2.0)` ohne `RenderFlex`-Overflow.
    // Ein Overflow malt sich als gestreifter Balken und wirft im Test; ohne
    // diesen Test bliebe er still, weil kein anderer Test skaliert. Gemessen
    // wurden 6,0 px bei doppelter Größe in der Zeile aus Überschrift und
    // Zustandswort.
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    for (final double scale in <double>[1, 2]) {
      tester.platformDispatcher.textScaleFactorTestValue = scale;

      await pumpApp(tester, client: FakeDaemonClient.unavailable());
      expect(tester.takeException(), isNull, reason: 'unavailable at $scale');
      await tester.pumpWidget(const SizedBox.shrink());

      await pumpApp(tester, client: FakeDaemonClient());
      await pressCtrl(tester, LogicalKeyboardKey.digit6);
      await tester.pump();
      expect(tester.takeException(), isNull, reason: 'four rows at $scale');
      // Und die Zeilen sind noch da: Ein Test, der nur „keine Ausnahme" prüft,
      // bestünde auch über einem leeren Bildschirm.
      for (final SetupCheckKind kind in SetupCheckKind.values) {
        expect(
          find.byKey(Key('setup-state-${kind.name}')),
          findsOneWidget,
          reason: '$kind at $scale',
        );
      }
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });

  testWidgets('every word of a row reaches 3:1 on the surface it stands on', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient());
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    final HTokens tokens = HTheme.of(
      tester.element(find.byKey(const Key('setup-start'))),
    );
    for (final Key key in <Key>[
      const Key('setup-state-daemon'),
      const Key('setup-state-llm'),
      const Key('setup-state-project'),
      const Key('setup-state-sandbox'),
      const Key('setup-start-reason'),
    ]) {
      final Text text = tester.widget<Text>(find.byKey(key));
      final Color? colour = text.style?.color;
      expect(colour, isNotNull, reason: '$key has no colour');
      expect(
        contrast(colour!, tokens.colors.bg0),
        greaterThan(3),
        reason: '$key on bg0',
      );
    }
  });
}
