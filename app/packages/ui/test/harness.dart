import 'package:flutter/widgets.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

/// Wraps [child] in the minimum a wrapper widget needs to render.
///
/// Deliberately not an app: these widgets must work under any host, and a test
/// that needs a `WidgetsApp` to see a button hides a dependency.
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
