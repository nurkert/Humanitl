// Die Seitenleiste klappt auf und zu (HUM-254): Knopf, `Ctrl+0`, Palette,
// Beschriftung nur ausgeklappt, reduzierte Bewegung, kein Überlauf in der
// Zwischenbreite und ein Zustand, der den Neustart überlebt.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/coach_mark.dart';
import 'package:humanitl/features/shell/providers/rail.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../harness/app_harness.dart';

/// Die Leiste allein, mit einem Zustand, den ihr Knopf umschaltet.
Widget railHarness({
  bool expanded = false,
  Locale locale = const Locale('en'),
  TextScaler textScaler = TextScaler.noScaling,
}) {
  final HTokens tokens = HThemeMode.dark.resolve(Brightness.dark);
  bool open = expanded;
  return WidgetsApp(
    color: tokens.colors.bg0,
    locale: locale,
    localizationsDelegates: <LocalizationsDelegate<Object?>>[
      ...AppLocalizations.localizationsDelegates,
      ...hLocalizationsDelegates,
    ],
    supportedLocales: AppLocalizations.supportedLocales,
    builder: (BuildContext context, Widget? _) => MediaQuery(
      data: MediaQuery.of(context).copyWith(textScaler: textScaler),
      child: HTheme(
        tokens: tokens,
        child: Overlay(
          initialEntries: <OverlayEntry>[
            OverlayEntry(
              builder: (BuildContext context) => StatefulBuilder(
                builder: (BuildContext context, StateSetter setState) => Align(
                  alignment: Alignment.topLeft,
                  child: SizedBox(
                    height: 600,
                    child: IconRail(
                      active: Section.rules,
                      onSelect: (_) {},
                      expanded: open,
                      onToggle: () => setState(() => open = !open),
                    ),
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    ),
  );
}

final Finder rail = find.byKey(const Key('icon-rail'));
final Finder toggle = find.byKey(const Key('icon-rail-toggle'));

double railWidthOf(WidgetTester tester) => tester.getSize(rail).width;

/// Die Deckkraft der Beschriftung des ersten Eintrags.
double labelOpacity(WidgetTester tester) => tester
    .widget<FadeTransition>(
      find
          .descendant(
            of: find.byType(RailLabelStrip),
            matching: find.byType(FadeTransition),
          )
          .first,
    )
    .opacity
    .value;

/// Reduzierte Bewegung, wie die Plattform sie meldet: `MediaQuery` und
/// `AnimationController` lesen beide daraus.
void reduceMotion(WidgetTester tester) {
  tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
      const FakeAccessibilityFeatures(disableAnimations: true);
  addTearDown(
    tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
  );
}

void main() {
  testWidgets('toggle button expands and collapses the rail', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(railHarness());
    expect(railWidthOf(tester), railWidth);
    expect(find.bySemanticsLabel('Expand sidebar (Ctrl+0)'), findsOneWidget);

    await tester.tap(toggle);
    await tester.pump();
    await tester.pump(HMotion.arrive ~/ 2);
    // Unterwegs: weder die alte noch die neue Breite.
    expect(railWidthOf(tester), greaterThan(railWidth));
    expect(railWidthOf(tester), lessThan(railExpandedWidth));

    await tester.pumpAndSettle();
    expect(railWidthOf(tester), railExpandedWidth);
    expect(find.bySemanticsLabel('Collapse sidebar (Ctrl+0)'), findsOneWidget);

    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(railWidthOf(tester), railWidth);
  });

  testWidgets('labels show only when expanded', (WidgetTester tester) async {
    await tester.pumpWidget(railHarness());
    for (final String name in <String>['Intercept', 'Rules', 'Setup']) {
      expect(
        find.descendant(of: rail, matching: find.text(name)),
        findsNothing,
      );
    }
    expect(
      find.descendant(of: rail, matching: find.text('Ctrl+1')),
      findsNothing,
    );

    await tester.tap(toggle);
    await tester.pumpAndSettle();
    for (final (String name, int digit) in <(String, int)>[
      ('Intercept', 1),
      ('History', 2),
      ('Rules', 3),
      ('Sandbox', 4),
      ('Audit', 5),
      ('Setup', 6),
    ]) {
      expect(
        find.descendant(of: rail, matching: find.text(name)),
        findsOneWidget,
      );
      expect(
        find.descendant(of: rail, matching: find.text('Ctrl+$digit')),
        findsOneWidget,
      );
    }
    expect(labelOpacity(tester), 1);

    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(
      find.descendant(of: rail, matching: find.text('Rules')),
      findsNothing,
    );
  });

  testWidgets('reduced motion: the width jumps, the names still fade', (
    WidgetTester tester,
  ) async {
    reduceMotion(tester);
    await tester.pumpWidget(railHarness());

    await tester.tap(toggle);
    await tester.pump();
    // Ein einziger Frame: schon die Endbreite, kein Weg dazwischen.
    expect(railWidthOf(tester), railExpandedWidth);
    final List<double> widths = <double>[];
    for (int i = 0; i < 12; i++) {
      await tester.pump(const Duration(milliseconds: 16));
      widths.add(railWidthOf(tester));
    }
    expect(widths, everyElement(railExpandedWidth));
    await tester.pumpAndSettle();

    // Das Einblenden ist Rückmeldung und behält seine Dauer (UX 2.10).
    await tester.tap(toggle);
    await tester.pump();
    expect(railWidthOf(tester), railWidth);
    await tester.tap(toggle);
    await tester.pump();
    expect(labelOpacity(tester), 0);
    await tester.pump(HMotion.arrive ~/ 2);
    expect(labelOpacity(tester), greaterThan(0));
    expect(labelOpacity(tester), lessThan(1));
    await tester.pump(HMotion.arrive);
    expect(labelOpacity(tester), 1);
  });

  testWidgets('no overflow mid-animation, in both languages and at 2x text', (
    WidgetTester tester,
  ) async {
    for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
      for (final double scale in const <double>[1, 2]) {
        await tester.pumpWidget(
          railHarness(locale: locale, textScaler: TextScaler.linear(scale)),
        );
        for (int direction = 0; direction < 2; direction++) {
          await tester.tap(toggle);
          await tester.pump();
          for (int t = 0; t < HMotion.arrive.inMilliseconds * 2; t += 8) {
            await tester.pump(const Duration(milliseconds: 8));
            final String at = '$locale x$scale direction $direction at $t ms';
            expect(tester.takeException(), isNull, reason: at);
            // Gesetzt wird in der vollen Breite, in jedem Frame: Ein Überlauf
            // unter durchsichtiger Beschriftung zeichnete keinen Fehler, und
            // ein Name, der in der Zwischenbreite umbricht, auch nicht.
            for (final Element row
                in find
                    .descendant(
                      of: find.byType(RailLabelStrip),
                      matching: find.byType(Row),
                    )
                    .evaluate()) {
              expect(
                row.size!.width,
                RailLabelStrip.contentWidth - HSpace.x3,
                reason: at,
              );
            }
          }
        }
        await tester.pumpWidget(const SizedBox.shrink());
      }
    }
  });

  testWidgets('Ctrl+0 and the palette toggle the rail in the shell', (
    WidgetTester tester,
  ) async {
    await pumpApp(tester, client: FakeDaemonClient.empty());
    expect(railWidthOf(tester), railWidth);

    await pressCtrl(tester, LogicalKeyboardKey.digit0);
    await tester.pump(HMotion.arrive);
    await tester.pump(HMotion.arrive);
    expect(railWidthOf(tester), railExpandedWidth);

    await pressCtrl(tester, LogicalKeyboardKey.digit0);
    await tester.pump(HMotion.arrive);
    await tester.pump(HMotion.arrive);
    expect(railWidthOf(tester), railWidth);

    // Die Palette ist das Handbuch (UX 5.1): derselbe Schalter steht darin.
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.tap(find.byKey(const Key('palette-toggle-rail')));
    await tester.pump();
    await tester.pump(HMotion.arrive);
    await tester.pump(HMotion.arrive);
    expect(railWidthOf(tester), railExpandedWidth);
    expect(tester.takeException(), isNull);
  });

  testWidgets('the state survives a restart', (WidgetTester tester) async {
    final Directory dir = Directory.systemTemp.createTempSync('hum-254-rail');
    addTearDown(() => dir.deleteSync(recursive: true));
    final File file = File('${dir.path}/${UiStateFile.fileName}')
      ..writeAsStringSync('{"$coachMarkSeenKey": true}');
    final UiStateFile store = UiStateFile(file.path);

    await pumpApp(
      tester,
      client: FakeDaemonClient.empty(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    expect(railWidthOf(tester), railWidth);
    await pressCtrl(tester, LogicalKeyboardKey.digit0);
    await tester.pump(HMotion.arrive);
    await tester.pump(HMotion.arrive);

    final Object? written = jsonDecode(file.readAsStringSync());
    expect(written, <String, Object?>{
      coachMarkSeenKey: true,
      railExpandedKey: true,
    });

    // Ein neuer Start über derselben Datei: ausgeklappt vom ersten Frame an.
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpApp(
      tester,
      client: FakeDaemonClient.empty(),
      uiState: uiStateFileProvider.overrideWithValue(store),
    );
    expect(railWidthOf(tester), railExpandedWidth);

    // Und zurück: `false` steht danach in der Datei, nicht der alte Wert.
    await pressCtrl(tester, LogicalKeyboardKey.digit0);
    await tester.pump(HMotion.arrive);
    await tester.pump(HMotion.arrive);
    expect(
      (jsonDecode(file.readAsStringSync())
          as Map<String, Object?>)[railExpandedKey],
      isFalse,
    );
  });

  testWidgets('collapsing fades the names out while the rail shrinks', (
    WidgetTester tester,
  ) async {
    // Ausgeklappt gestartet, wie nach einem Neustart mit gemerktem Zustand.
    await tester.pumpWidget(railHarness(expanded: true));
    await tester.pumpAndSettle();
    expect(labelOpacity(tester), 1);

    await tester.tap(toggle);
    await tester.pump();
    await tester.pump(HMotion.arrive ~/ 2);
    // Derselbe Streifen blendet aus; ein neu gebauter stünde schon auf 0.
    expect(railWidthOf(tester), greaterThan(railWidth));
    expect(labelOpacity(tester), greaterThan(0));
    expect(labelOpacity(tester), lessThan(1));

    await tester.pumpAndSettle();
    expect(find.byType(RailLabelStrip), findsNothing);
  });

  testWidgets('the toggle button says whether the rail is expanded', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle semantics = tester.ensureSemantics();
    await tester.pumpWidget(railHarness());
    expect(
      tester.getSemantics(toggle),
      isSemantics(
        label: 'Expand sidebar (Ctrl+0)',
        isButton: true,
        hasExpandedState: true,
        isExpanded: false,
      ),
    );

    await tester.tap(toggle);
    await tester.pumpAndSettle();
    expect(
      tester.getSemantics(toggle),
      isSemantics(
        label: 'Collapse sidebar (Ctrl+0)',
        hasExpandedState: true,
        isExpanded: true,
      ),
    );
    semantics.dispose();
  });
}
