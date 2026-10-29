// Allow und Block auf einer Zeile (HUM-307).
//
// Gemeldet am 2026-10-29: Im Intercept-Tab stand „Blockieren" tiefer als
// „Senden", als wäre es in eine zweite Zeile umgebrochen. Das war es: Unter
// 800 px Pane-Breite stellte die Leiste Block unter Allow, und den Pane gibt
// es in kaum einem Fenster so breit, mit aufgeklappter Leiste der Shell
// (HUM-254) nicht einmal bei 1280 px. Geprüft wird hier, in beiden Sprachen,
// bei 1,0- und 1,3-facher Schrift, im schmalsten Pane (480 px), im
// Vorgabefenster und breit, dass beide Entscheidungen auf einer Zeile und auf
// einer Höhe stehen und keine Beschriftung umbricht.
//
// Gemessen wird mit Roboto (`harness/layout_fonts.dart`): Mit den Quadraten
// der Testschrift wäre jede Beschriftung doppelt so breit wie auf dem
// Bildschirm, und der schmale Pane zeigte eine Leiste, die es nicht gibt. Was
// bei doppelter Schrift mit den Quadraten geschieht, prüft `action_bar_test`.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/intercept/widgets/block_button.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';
import 'package:humanitl/features/shell/providers/connection.dart';

import '../../harness/layout_fonts.dart';
import '../../harness/ui_state.dart';
import 'fixtures.dart';

/// A window the bar is measured in.
typedef Window = ({String name, Size size, bool railExpanded});

/// The narrowest pane: 1280 px with the rail expanded leaves the inspector
/// at its minimum of 480 px, the window of the report.
const Window narrow = (
  name: 'narrowest pane',
  size: Size(1280, 800),
  railExpanded: true,
);

/// The default window, rail collapsed: about 540 px of inspector.
const Window standard = (
  name: 'default window',
  size: Size(1280, 800),
  railExpanded: false,
);

/// A wide window: about 820 px of inspector.
const Window wide = (
  name: 'wide window',
  size: Size(1920, 1080),
  railExpanded: false,
);

FlowDetail request({required int findings}) => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    apex: 'github.com',
    path: '/graphql?first=20',
    requestSize: 428,
  ).copyWith(findingCount: findings),
  bodyPreview: '{"query": "mutation { createIssue }"}',
  contentType: 'application/json',
  apex: 'github.com',
  findings: <Finding>[for (int i = 0; i < findings; i++) testFinding()],
);

Future<void> pumpBar(
  WidgetTester tester, {
  required Locale locale,
  required double scale,
  required Window window,
  int findings = 0,
}) async {
  tester.platformDispatcher.localesTestValue = <Locale>[locale];
  tester.platformDispatcher.textScaleFactorTestValue = scale;
  addTearDown(tester.platformDispatcher.clearAllTestValues);
  await tester.binding.setSurfaceSize(window.size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        uiStateOverride(railExpanded: window.railExpanded),
        daemonClientProvider.overrideWithValue(
          FakeDaemonClient(
            script: holdScript(<FlowDetail>[request(findings: findings)]),
            clock: () => testStart,
          ),
        ),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
      ],
      child: const HumanitlApp(),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  await tester.pump();
}

/// How many lines [paragraph] is laid out in.
int linesOf(RenderParagraph paragraph) {
  final int length = paragraph.text.toPlainText().length;
  if (length == 0) {
    return 0;
  }
  return paragraph
      .getBoxesForSelection(TextSelection(baseOffset: 0, extentOffset: length))
      .map((TextBox box) => box.top.round())
      .toSet()
      .length;
}

/// The paragraphs of the action bar's controls (not of the lines under
/// them) that wrap, with their text.
List<String> wrappedControls(WidgetTester tester) => <String>[
  for (final Finder control in <Finder>[
    find.byType(ReleaseValve),
    find.byType(BlockButton),
    find.byKey(const Key('intercept-edit')),
    find.byKey(const Key('intercept-note-open')),
  ])
    for (final RenderParagraph paragraph
        in tester.renderObjectList<RenderParagraph>(
          find.descendant(of: control, matching: find.byType(RichText)),
        ))
      if (linesOf(paragraph) > 1) paragraph.text.toPlainText(),
];

/// The label paragraph of [control]: its first, the one in the label font.
RenderParagraph labelOf(WidgetTester tester, Finder control) => tester
    .renderObjectList<RenderParagraph>(
      find.descendant(of: control, matching: find.byType(RichText)),
    )
    .first;

/// The alphabetic baseline of [control]'s label, in global coordinates.
double baselineOf(WidgetTester tester, Finder control) {
  final RenderParagraph label = labelOf(tester, control);
  return label.localToGlobal(Offset.zero).dy +
      label.computeDistanceToActualBaseline(TextBaseline.alphabetic);
}

void main() {
  setUpAll(loadLayoutFonts);

  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
    for (final double scale in const <double>[1.0, 1.3]) {
      for (final Window window in const <Window>[narrow, standard, wide]) {
        for (final int findings in const <int>[0, 2]) {
          final String name =
              '${locale.languageCode}, ${scale}x, ${window.name}, '
              '$findings findings';

          testWidgets('Allow and Block stand on one line ($name)', (
            WidgetTester tester,
          ) async {
            await pumpBar(
              tester,
              locale: locale,
              scale: scale,
              window: window,
              findings: findings,
            );
            expect(find.byType(ActionBar), findsOneWidget);

            final Rect allow = tester.getRect(find.byType(ReleaseValve));
            final Rect block = tester.getRect(find.byType(BlockButton));
            // Eine Zeile, eine Höhe: dieselbe Mitte der Knöpfe und dieselbe
            // Grundlinie der Beschriftungen.
            expect(
              block.center.dy,
              moreOrLessEquals(allow.center.dy, epsilon: 0.5),
              reason: 'Block stands lower or higher than Allow',
            );
            expect(
              baselineOf(tester, find.byType(BlockButton)),
              moreOrLessEquals(
                baselineOf(tester, find.byType(ReleaseValve)),
                epsilon: 0.5,
              ),
              reason:
                  'the labels of Allow and Block sit on different baselines',
            );
            expect(block.left, greaterThan(allow.right));
            // Der Abstand, der einen verfehlten Klick auf Block nicht auf
            // Allow landen lässt, auch im schmalsten Pane.
            expect(
              block.left - allow.right,
              greaterThanOrEqualTo(decisionGapMin),
            );
            expect(wrappedControls(tester), isEmpty);
            // Bei einfacher Schrift behalten beide Entscheidungen ihre
            // ganze Beschriftung, auch im schmalsten Pane. Ausnahme ist dort
            // „Senden mit 2 Funden": Das Ventil gibt als Letztes nach und
            // kürzt seine Beschriftung, bevor Block unter Allow rutscht;
            // Glyph, Farbe und die Zeile darunter nennen den Fund weiter.
            if (scale == 1.0 && (findings == 0 || window != narrow)) {
              expect(
                labelOf(tester, find.byType(ReleaseValve)).didExceedMaxLines,
                isFalse,
              );
              expect(
                labelOf(tester, find.byType(BlockButton)).didExceedMaxLines,
                isFalse,
              );
            }
            expect(tester.takeException(), isNull);
          }, variant: linux);
        }
      }
    }
  }

  testWidgets('a wide pane keeps the edit control and the full distance', (
    WidgetTester tester,
  ) async {
    await pumpBar(tester, locale: const Locale('de'), scale: 1.0, window: wide);

    final Finder edit = find.byKey(const Key('intercept-edit'));
    expect(edit.hitTestable(), findsOneWidget);
    final Rect allow = tester.getRect(find.byType(ReleaseValve));
    final Rect block = tester.getRect(find.byType(BlockButton));
    expect(tester.getRect(edit).center.dy, moreOrLessEquals(allow.center.dy));
    expect(
      block.left - tester.getRect(edit).right,
      greaterThanOrEqualTo(decisionGap),
    );
  }, variant: linux);

  testWidgets('the narrowest pane drops the edit control from the line', (
    WidgetTester tester,
  ) async {
    await pumpBar(
      tester,
      locale: const Locale('de'),
      scale: 1.0,
      window: narrow,
    );

    // Nicht gezeichnet, nicht zu treffen, nicht vorgelesen und ohne Halt in
    // der Fokusreihenfolge; `E` öffnet den Editor weiter.
    final Finder edit = find.byKey(const Key('intercept-edit'));
    expect(edit.hitTestable(), findsNothing);
    expect(find.bySemanticsLabel(RegExp('Bearbeiten')), findsNothing);
    final FocusNode gate = tester
        .widgetList<Focus>(
          find.ancestor(of: edit, matching: find.byType(Focus)),
        )
        .map((Focus focus) => focus.focusNode)
        .whereType<FocusNode>()
        .firstWhere(
          (FocusNode node) => node.debugLabel == 'DecisionRow edit gate',
        );
    expect(gate.descendantsAreFocusable, isFalse);
    expect(gate.descendantsAreTraversable, isFalse);
  }, variant: linux);
}
