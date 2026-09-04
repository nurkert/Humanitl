import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

/// Wraps [child] in the minimum a wrapper widget needs to render.
///
/// Deliberately not an app: these widgets must work under any host, and a test
/// that needs a `WidgetsApp` to see a button hides a dependency.
///
/// Mit [overlay] kommt eine Overlay-Ebene dazu. Sie wird genau dort
/// gebraucht, wo ein Text mit dem Zeiger ausgewählt werden kann: die Griffe
/// der Auswahl liegen auf einer solchen Ebene, und Flutter bricht ohne sie ab.
/// Nicht als Vorgabe, weil ein `Overlay` seine Einträge bei jedem
/// `pumpWidget` neu aufbaut und damit jeden laufenden Übergang darunter
/// zurücksetzt — genau das, was ein Test über Dauern messen will. Die
/// Anwendung hängt einen `Overlay` an ihre Wurzel (`app.dart`).
///
/// Wer Tasten prüft, legt [keyboard] darum: die Zuordnung Taste auf Intent
/// gehört der Anwendung, nicht dem Control.
Widget harness(
  Widget child, {
  Brightness brightness = Brightness.dark,
  bool overlay = false,
}) {
  final Widget framed = Align(
    alignment: Alignment.topLeft,
    child: SizedBox(width: 480, child: child),
  );
  return Directionality(
    textDirection: TextDirection.ltr,
    child: MediaQuery(
      data: const MediaQueryData(),
      child: HTheme(
        tokens: HTokens.forBrightness(brightness),
        child: overlay
            ? Overlay(
                initialEntries: <OverlayEntry>[
                  OverlayEntry(builder: (BuildContext context) => framed),
                ],
              )
            : framed,
      ),
    ),
  );
}

/// Legt um [child], was sonst die Anwendung mitbringt: die
/// Standard-Tastenzuordnung und die Standard-Actions, die aus `Enter` und der
/// Leertaste ein `ActivateIntent` und aus `Tab` einen Fokuswechsel machen.
///
/// Ein Control liefert seine Actions selbst; die Zuordnung Taste auf Intent
/// gehört dem Host. Ohne sie prüfte ein Test die Tastaturparität aus
/// `docs/UX.md` 5.1 gegen einen Baum, in dem keine Taste ankommt.
Widget keyboard(Widget child) => Shortcuts(
  shortcuts: WidgetsApp.defaultShortcuts,
  child: Actions(
    actions: WidgetsApp.defaultActions,
    child: FocusTraversalGroup(child: child),
  ),
);

/// Die Farbe, die das Control unter [of] in diesem Frame wirklich malt.
///
/// Die `H*`-Controls stehen auf `Clickable` aus `shadcn_flutter`; dessen
/// Fläche ist eine `OverflowDecoratedBox` und kein `Container` mit
/// `decoration`. Ein Test, der die gemalte Farbe prüfen will
/// — und das ist der Unterschied zwischen „die Füllung ist unterwegs" und
/// „die Füllung ist angekommen" —, liest sie hier. Der Testcode des Pakets
/// darf die Bibliothek sehen; ein Feature nicht (ADR-0009).
Color paintedFill(WidgetTester tester, Finder of) {
  final BoxDecoration decoration =
      tester
              .widget<shad.OverflowDecoratedBox>(
                find
                    .descendant(
                      of: of,
                      matching: find.byType(shad.OverflowDecoratedBox),
                    )
                    .first,
              )
              .decoration
          as BoxDecoration;
  return decoration.color ?? const Color(0x00000000);
}

/// Die `BoxDecoration`, die das Control unter [of] in diesem Frame malt.
BoxDecoration paintedDecoration(WidgetTester tester, Finder of) =>
    tester
            .widget<shad.OverflowDecoratedBox>(
              find
                  .descendant(
                    of: of,
                    matching: find.byType(shad.OverflowDecoratedBox),
                  )
                  .first,
            )
            .decoration
        as BoxDecoration;

/// Ob der Fokusring, den `shadcn_flutter` selbst zeichnet, unter [of] gerade
/// sichtbar ist.
///
/// `HTextField` ist das eine Control, dessen Ring aus der Bibliothek kommt:
/// ihr `TextField` bringt ihn fest eingebaut mit. Seine Maße sind unsere,
/// gesetzt über das `FocusOutlineTheme` in `HTheme`.
bool libraryFocusRing(WidgetTester tester, Finder of) => tester
    .widget<shad.FocusOutline>(
      find.descendant(of: of, matching: find.byType(shad.FocusOutline)).first,
    )
    .focused;

/// Ob der Platzhalter mit dem Text [text] gerade zu sehen ist.
///
/// Die Bibliothek lässt ihn im Baum stehen und blendet ihn aus, sobald das
/// Feld einen Text hat; `findsNothing` wäre deshalb die falsche Frage.
bool placeholderVisible(WidgetTester tester, String text) => tester
    .widget<Visibility>(
      find
          .ancestor(of: find.text(text), matching: find.byType(Visibility))
          .first,
    )
    .visible;

/// Die Farbe, in der der Text [text] wirklich gemalt wird.
///
/// Nicht `Text.style?.color`: seit die Controls auf `Clickable` aus
/// `shadcn_flutter` stehen, kommt ihre Schrift aus einem `DefaultTextStyle`,
/// den der Stil setzt, und das `Text`-Widget selbst trägt keine Farbe mehr. Gelesen wird deshalb der Absatz, den das Rendering baut.
Color? paintedTextColor(WidgetTester tester, String text) =>
    tester.renderObject<RenderParagraph>(find.text(text)).text.style?.color;
