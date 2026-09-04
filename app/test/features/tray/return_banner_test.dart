// Das Rückkehr-Banner als Widget (HUM-034): Tastatur, Textskalierung,
// reduzierte Bewegung.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/tray/widgets/return_banner.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Das Banner in einem Fenster von [width], mit [textScaler].
Widget banner({
  required VoidCallback onJump,
  required VoidCallback onDismiss,
  double width = 900,
  TextScaler textScaler = TextScaler.noScaling,
  bool disableAnimations = false,
}) => WidgetsApp(
  color: HColors.bg0,
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  builder: (BuildContext context, Widget? _) => MediaQuery(
    data: MediaQueryData(
      textScaler: textScaler,
      disableAnimations: disableAnimations,
    ),
    child: HTheme(
      tokens: HTokens.dark,
      child: Focus(
        autofocus: true,
        child: Align(
          alignment: Alignment.topCenter,
          child: SizedBox(
            width: width,
            child: ReturnBanner(
              sentence: 'The agent has been waiting 4 minutes',
              onJump: onJump,
              onDismiss: onDismiss,
            ),
          ),
        ),
      ),
    ),
  ),
);

/// Tabbt, bis der Fokus auf dem Control mit [key] steht.
Future<void> tabTo(WidgetTester tester, Key key) async {
  bool onIt() {
    final BuildContext? context = FocusManager.instance.primaryFocus?.context;
    if (context == null) {
      return false;
    }
    return context.findAncestorWidgetOfExactType<HButton>()?.key == key;
  }

  for (int i = 0; i < 12 && !onIt(); i++) {
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
  }
  expect(onIt(), isTrue, reason: 'focus never reached $key');
}

void main() {
  testWidgets('the_banner_says_one_line_and_offers_two_ways_out', (
    WidgetTester tester,
  ) async {
    int jumps = 0;
    int dismissals = 0;
    await tester.pumpWidget(
      banner(onJump: () => jumps++, onDismiss: () => dismissals++),
    );
    await tester.pump(HMotion.arrive);

    expect(find.text('The agent has been waiting 4 minutes'), findsOneWidget);
    await tester.tap(find.byKey(const Key('return-banner-jump')));
    expect(jumps, 1);
    await tester.tap(find.byKey(const Key('return-banner-dismiss')));
    expect(dismissals, 1);
  });

  testWidgets('both_controls_answer_the_keyboard', (WidgetTester tester) async {
    int jumps = 0;
    int dismissals = 0;
    await tester.pumpWidget(
      banner(onJump: () => jumps++, onDismiss: () => dismissals++),
    );
    await tester.pump(HMotion.arrive);

    // Jede Zeigergeste hat eine Tastenentsprechung (`docs/UX.md` 5.1).
    await tabTo(tester, const Key('return-banner-jump'));
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(jumps, 1);

    await tabTo(tester, const Key('return-banner-dismiss'));
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pump();
    expect(dismissals, 1);
  });

  testWidgets('it_grows_with_the_text_instead_of_clipping_it', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      banner(
        onJump: () {},
        onDismiss: () {},
        width: 700,
        textScaler: const TextScaler.linear(2),
      ),
    );
    await tester.pump(HMotion.arrive);

    expect(tester.takeException(), isNull);
    // Die 36 px sind eine Mindesthöhe, keine Höhe (`docs/UX.md` 6).
    expect(
      tester.getSize(find.byType(ReturnBanner)).height,
      greaterThan(HSize.row),
    );
  });

  testWidgets('reduced_motion_keeps_the_fade_and_drops_the_travel', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      banner(onJump: () {}, onDismiss: () {}, disableAnimations: true),
    );
    await tester.pump();

    // Kein Transform, also kein Weg; das Einblenden bleibt (`docs/UX.md` 2.10).
    expect(find.byType(Transform), findsNothing);
    expect(find.byType(FadeTransition), findsOneWidget);
  });
}
