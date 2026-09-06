// Der einmalige Hinweis ueber der Aktionsleiste (HUM-044).
//
// Vier Zusagen, vier Tests: Er erscheint beim ersten gehaltenen Request; er
// verdeckt die Entscheidung nicht, die er erklaert; `Esc` und der Knopf
// schliessen ihn; und er kommt nach einem Neustart nicht wieder.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/coach_mark.dart';

import '../../harness/ui_state.dart';
import 'fixtures.dart';
import 'harness.dart';

/// Der Hinweis, wenn er auf dem Schirm ist.
Finder get mark => find.byKey(const Key('intercept-coach-mark'));

void main() {
  testWidgets('coach_mark_shown_once', (WidgetTester tester) async {
    // Ein Speicher, der noch nichts weiss: der allererste Start.
    final Override store = uiStateOverride(seen: false);

    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(1)])),
      uiState: store,
    );
    await playScript(tester);
    expect(mark, findsOneWidget);
    expect(find.text('This request is waiting for you'), findsOneWidget);

    // Zweiter Start derselben Installation, derselbe Speicher: kein Hinweis.
    // Das ist der Unterschied zwischen „einmal" und „einmal je Fenster".
    await tester.pumpWidget(const SizedBox.shrink());
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(2)])),
      uiState: store,
    );
    await playScript(tester);
    expect(mark, findsNothing);
  });

  testWidgets('the coach mark does not cover the action bar', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(1)])),
      uiState: uiStateOverride(seen: false),
    );
    await playScript(tester);
    expect(mark, findsOneWidget);

    // Nicht Geometrie auf gut Glueck, sondern die Aussage selbst: Der Kasten
    // endet, bevor die Leiste anfaengt, und beide Entscheidungen bleiben
    // anklickbar.
    final Rect popover = tester.getRect(mark);
    final Rect bar = tester.getRect(find.byType(ActionBar));
    expect(popover.bottom, lessThanOrEqualTo(bar.top));
    expect(
      tester.getRect(find.byKey(const Key('intercept-allow'))).height,
      greaterThan(0),
    );
    expect(
      tester.getRect(find.byKey(const Key('intercept-block'))).height,
      greaterThan(0),
    );
  });

  testWidgets('escape and the button close it, and it stays closed', (
    WidgetTester tester,
  ) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(1)])),
      uiState: uiStateOverride(seen: false),
    );
    await playScript(tester);
    expect(mark, findsOneWidget);

    // Der Knopf.
    await tester.tap(
      find.descendant(of: mark, matching: find.byType(HIconButton)),
    );
    await tester.pump();
    expect(mark, findsNothing);

    // Und er bleibt zu, auch wenn die naechste Anfrage ankommt.
    await playScript(tester);
    expect(mark, findsNothing);
  });

  testWidgets('escape closes it', (WidgetTester tester) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(1)])),
      uiState: uiStateOverride(seen: false),
    );
    await playScript(tester);
    expect(mark, findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(mark, findsNothing);
  });

  testWidgets('a seen mark never appears', (WidgetTester tester) async {
    await pumpIntercept(
      tester,
      client: fakeDaemon(holdScript(<FlowDetail>[held(1)])),
      uiState: uiStateOverride(),
    );
    await playScript(tester);
    expect(mark, findsNothing);
    expect(find.byType(CoachMark), findsOneWidget);
  });
}
