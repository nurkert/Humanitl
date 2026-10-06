// Das Kontextmenü eines `HTextField` (HUM-173).
//
// Ein Rechtsklick öffnet das Desktop-Menü der Komponentenbibliothek, und
// dessen `MenuShortcut` verlangt einen `KeyboardShortcutDisplayMapper` im
// Baum. Flutter hängt das Menü in die Overlay-Ebene der Wurzel, also nicht
// unter das Feld; deshalb legt `HTextField` Mapper und Theme um das Menü
// selbst, und die Tests unten stellen die Ebene über, unter und neben `HTheme`.
// Die Wörter des Menüs bleiben unter `de` englisch, bis die Bibliothek
// Deutsch mitbringt (`hLocalizationsDelegates`).
//
// Unter `TargetPlatformVariant.only(linux)`: `flutter test` läuft sonst als
// Android, und dort entsteht das Desktop-Menü nicht.

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Legt die Sprache über [child]; die Ebene des Menüs steht darunter.
Widget _localized(Locale locale, Widget child) => Localizations(
  locale: locale,
  delegates: <LocalizationsDelegate<Object?>>[
    DefaultWidgetsLocalizations.delegate,
    ...hLocalizationsDelegates,
  ],
  child: child,
);

/// Eine Wurzel nur mit [Overlay]: ohne `HTheme`, ohne Bibliothek.
Widget _bareOverlay(Widget Function(BuildContext) field) => Directionality(
  textDirection: TextDirection.ltr,
  child: MediaQuery(
    data: const MediaQueryData(),
    child: Overlay(
      initialEntries: <OverlayEntry>[
        OverlayEntry(
          builder: (BuildContext context) =>
              Align(alignment: Alignment.topLeft, child: field(context)),
        ),
      ],
    ),
  ),
);

Future<void> _rightClickOpensMenu(WidgetTester tester) async {
  await tester.tap(find.byType(HTextField), buttons: kSecondaryButton);
  await tester.pumpAndSettle();

  expect(tester.takeException(), isNull);
  expect(find.text('Cut'), findsOneWidget);
  expect(find.text('Copy'), findsOneWidget);
  expect(find.text('Paste'), findsOneWidget);
  expect(find.text('Select All'), findsOneWidget);
}

Widget _field(TextEditingController controller) => SizedBox(
  width: 240,
  child: HTextField(controller: controller, semanticsLabel: 'Host'),
);

void main() {
  late TextEditingController controller;

  setUp(() => controller = TextEditingController(text: 'api.example.com'));
  tearDown(() => controller.dispose());

  // `HTheme` über der Ebene: der Aufbau der Anwendung (`app.dart`).
  for (final Locale locale in const <Locale>[Locale('de'), Locale('en')]) {
    testWidgets('context_menu_opens_under_${locale.languageCode}', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        _localized(
          locale,
          harness(
            HTextField(controller: controller, semanticsLabel: 'Host'),
            overlay: true,
          ),
        ),
      );
      await _rightClickOpensMenu(tester);
    }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
  }

  // Kein `HTheme`, ein Overlay darüber: das Feld allein, wie Codex es fand.
  testWidgets(
    'context_menu_opens_for_a_field_without_a_theme_under_an_overlay',
    (WidgetTester tester) async {
      await tester.pumpWidget(
        _localized(
          const Locale('en'),
          _bareOverlay((BuildContext _) => _field(controller)),
        ),
      );
      await _rightClickOpensMenu(tester);
    },
    variant: TargetPlatformVariant.only(TargetPlatform.linux),
  );

  // `HTheme` **unter** der Ebene: Das Menü steht neben ihm, nicht darunter.
  testWidgets('context_menu_opens_when_the_overlay_sits_above_the_theme', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _localized(
        const Locale('en'),
        _bareOverlay(
          (BuildContext _) => HTheme.dark(child: _field(controller)),
        ),
      ),
    );
    await _rightClickOpensMenu(tester);
  }, variant: TargetPlatformVariant.only(TargetPlatform.linux));
}
