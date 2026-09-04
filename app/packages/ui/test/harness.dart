import 'package:flutter/widgets.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

/// Wraps [child] in the minimum a wrapper widget needs to render.
///
/// Deliberately not an app: these widgets must work under any host, and a test
/// that needs a `WidgetsApp` to see a button hides a dependency.
///
/// Wer Tasten prüft, legt [keyboard] darum: die Zuordnung Taste auf Intent
/// gehört der Anwendung, nicht dem Control.
Widget harness(Widget child, {Brightness brightness = Brightness.dark}) {
  return Directionality(
    textDirection: TextDirection.ltr,
    child: MediaQuery(
      data: const MediaQueryData(),
      child: HTheme(
        tokens: HTokens.forBrightness(brightness),
        child: Align(
          alignment: Alignment.topLeft,
          child: SizedBox(width: 480, child: child),
        ),
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
