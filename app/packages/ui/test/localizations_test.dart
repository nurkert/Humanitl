// Die Wörter der Komponentenbibliothek in jeder Sprache der Anwendung
// (HUM-052). Unter `de` hat die Bibliothek keine eigenen; ohne
// `hLocalizationsDelegates` bricht `ShadcnLocalizations.of` mit einem
// Null-Check ab, und mit ihm jede Komponente, die ihre Wörter braucht.
//
// Das Kontextmenü eines `HTextField` stürzt zusätzlich in jeder Sprache ab,
// weil ihm der `KeyboardShortcutDisplayMapper` der Bibliothek fehlt; das ist
// HUM-173.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

Widget localized(Locale locale, Widget child) => Localizations(
  locale: locale,
  delegates: <LocalizationsDelegate<Object?>>[
    DefaultWidgetsLocalizations.delegate,
    ...hLocalizationsDelegates,
  ],
  child: child,
);

void main() {
  for (final Locale locale in const <Locale>[Locale('de'), Locale('en')]) {
    testWidgets('the library finds its words under ${locale.languageCode}', (
      WidgetTester tester,
    ) async {
      late shad.ShadcnLocalizations words;
      await tester.pumpWidget(
        localized(
          locale,
          Builder(
            builder: (BuildContext context) {
              words = shad.ShadcnLocalizations.of(context);
              return const SizedBox();
            },
          ),
        ),
      );
      expect(words.menuCopy, 'Copy');
    });
  }
}
