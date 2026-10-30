// Das gemeinsame Muster der Hinweise (HUM-302): Sie schweben über ihrer
// Region, oben rechts, und nehmen ihr keine Höhe. Geprüft wird an einem
// nackten `HNoticeLayer`, was jeder Ort erbt: die Region behält Größe und
// Zustand, die Karte steht rechtsbündig mit höchstens 420 px, das Schließen
// liegt in der Karte und trifft auf 28 px, und das Erscheinen nimmt niemandem
// die Tastatur.

import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/h_notice_layer.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/ui.dart';

/// Ein Zähler mit eigenem Zustand: verliert die Region ihr Element, beginnt
/// er wieder bei null.
class _Counter extends StatefulWidget {
  const _Counter({required this.focusNode});

  final FocusNode focusNode;

  @override
  State<_Counter> createState() => _CounterState();
}

class _CounterState extends State<_Counter> {
  int value = 0;

  @override
  Widget build(BuildContext context) => Focus(
    focusNode: widget.focusNode,
    // Opak für Zeiger: `ColoredBox` nimmt jeden Treffer in seiner Fläche.
    child: const ColoredBox(
      key: Key('region'),
      color: Color(0xFF000000),
      child: SizedBox.expand(),
    ),
  );
}

/// Was der Wirt zeigt: ob die Hinweise stehen, und wie breit die Region ist.
///
/// Ein `ValueNotifier` und kein neuer Baum je Zustand: Der Eintrag eines
/// `Overlay` baut sich aus seinem ersten Builder, und ein zweites
/// `pumpWidget` erreichte ihn nicht.
typedef HostState = ({bool showing, double width});

/// Ein Wirt mit Theme und Overlay, die Region `width` mal 600 px groß.
Widget host({
  required ValueNotifier<HostState> state,
  required FocusNode focusNode,
  VoidCallback? onDismiss,
}) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  builder: (BuildContext context, Widget? _) => HTheme(
    tokens: HTokens.dark,
    child: Overlay(
      initialEntries: <OverlayEntry>[
        OverlayEntry(
          builder: (BuildContext context) => ValueListenableBuilder<HostState>(
            valueListenable: state,
            builder: (BuildContext context, HostState value, Widget? _) =>
                Align(
                  alignment: Alignment.topLeft,
                  child: SizedBox(
                    width: value.width,
                    height: 600,
                    child: HNoticeLayer(
                      notices: HNoticeColumn(
                        children: <Widget>[
                          if (value.showing)
                            HNoticeCard(
                              key: const Key('card'),
                              dismissLabel: 'Dismiss',
                              dismissKey: const Key('card-dismiss'),
                              onDismiss: onDismiss ?? () {},
                              header: const Text(
                                'LLM endpoint is not on a private network',
                              ),
                              child: const Text(
                                'A longer second line runs below the close '
                                'glyph and may use the full width of the card.',
                                key: Key('body'),
                              ),
                            ),
                          if (value.showing)
                            HNoticeCard(
                              key: const Key('second'),
                              dismissLabel: 'Dismiss',
                              onDismiss: () {},
                              header: const Text('a second notice'),
                              child: const SizedBox.shrink(),
                            ),
                        ],
                      ),
                      child: _Counter(focusNode: focusNode),
                    ),
                  ),
                ),
          ),
        ),
      ],
    ),
  ),
);

/// Baut den Wirt mit [showing] und gibt den Zustand zum Umschalten zurück.
Future<ValueNotifier<HostState>> pumpHost(
  WidgetTester tester, {
  required FocusNode focusNode,
  bool showing = true,
  VoidCallback? onDismiss,
}) async {
  final ValueNotifier<HostState> state = ValueNotifier<HostState>((
    showing: showing,
    width: 900,
  ));
  addTearDown(state.dispose);
  await tester.pumpWidget(
    host(state: state, focusNode: focusNode, onDismiss: onDismiss),
  );
  return state;
}

void main() {
  testWidgets('a_notice_floats_and_the_region_keeps_size_and_state', (
    WidgetTester tester,
  ) async {
    final FocusNode focus = FocusNode();
    addTearDown(focus.dispose);
    final ValueNotifier<HostState> state = await pumpHost(
      tester,
      focusNode: focus,
      showing: false,
    );
    final Rect before = tester.getRect(find.byKey(const Key('region')));
    tester.state<_CounterState>(find.byType(_Counter)).value = 7;
    focus.requestFocus();
    await tester.pump();
    expect(focus.hasPrimaryFocus, isTrue, reason: 'the region holds the keys');

    state.value = (showing: true, width: 900);
    await tester.pump();
    expect(find.byKey(const Key('card')), findsOneWidget);
    // Die Region ist nicht kleiner geworden und nicht neu entstanden.
    expect(tester.getRect(find.byKey(const Key('region'))), before);
    expect(tester.state<_CounterState>(find.byType(_Counter)).value, 7);
    // Und die Tastatur bleibt, wo sie war.
    expect(focus.hasPrimaryFocus, isTrue);

    state.value = (showing: false, width: 900);
    await tester.pump();
    expect(find.byKey(const Key('card')), findsNothing);
    expect(tester.getRect(find.byKey(const Key('region'))), before);
    expect(tester.state<_CounterState>(find.byType(_Counter)).value, 7);
  });

  testWidgets('cards_stand_right_aligned_with_a_gap_and_a_maximum_width', (
    WidgetTester tester,
  ) async {
    final FocusNode focus = FocusNode();
    addTearDown(focus.dispose);
    final ValueNotifier<HostState> state = await pumpHost(
      tester,
      focusNode: focus,
    );
    final Rect region = tester.getRect(find.byKey(const Key('region')));
    final Rect card = tester.getRect(find.byKey(const Key('card')));
    final Rect second = tester.getRect(find.byKey(const Key('second')));

    expect(card.width, noticeMaxWidth);
    expect(card.right, region.right - HSpace.x2);
    expect(card.top, region.top + HSpace.x2);
    expect(second.top - card.bottom, HSpace.x2);
    expect(second.right, card.right);

    // In einer schmalen Region: so breit wie sie, abzüglich des Abstands.
    state.value = (showing: true, width: 300);
    await tester.pump();
    expect(
      tester.getRect(find.byKey(const Key('card'))).width,
      300 - 2 * HSpace.x2,
    );
  });

  testWidgets('the_close_glyph_sits_inside_the_card_with_a_label_on_hover', (
    WidgetTester tester,
  ) async {
    final FocusNode focus = FocusNode();
    addTearDown(focus.dispose);
    int closed = 0;
    await pumpHost(tester, focusNode: focus, onDismiss: () => closed++);
    final Rect card = tester.getRect(find.byKey(const Key('card')));
    final Rect close = tester.getRect(find.byKey(const Key('card-dismiss')));

    // In der Karte, oben rechts, mit dem Abstand des Rasters zur Kante.
    expect(card.contains(close.topLeft), isTrue);
    expect(card.contains(close.bottomRight - const Offset(1, 1)), isTrue);
    expect(close.right, card.right - HSpace.x1);
    expect(close.top, card.top + HSpace.x1);
    expect(close.width, greaterThanOrEqualTo(HSize.hitMin));
    expect(close.height, greaterThanOrEqualTo(HSize.hitMin));
    // Die erste Zeile läuft nicht unter den Knopf, und ihr Platz endet
    // genau vor ihm, mit dem Abstand des Rasters.
    final Rect text = tester.getRect(
      find.text('LLM endpoint is not on a private network'),
    );
    expect(text.right, lessThanOrEqualTo(close.left - HSpace.x1));
    // Darunter gehört die Breite dem Text: keine leere Spalte rechts.
    final Rect body = tester.getRect(find.byKey(const Key('body')));
    expect(body.top, greaterThanOrEqualTo(close.bottom));
    expect(body.right, card.right - HSpace.x3);

    final TestGesture mouse = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    addTearDown(mouse.removePointer);
    await mouse.addPointer(location: close.center);
    await tester.pump();
    await tester.pump(HMotion.hoverLabel);
    expect(
      find.descendant(
        of: find.byType(HoverLabel),
        matching: find.text('Dismiss'),
      ),
      findsWidgets,
    );

    await tester.tap(find.byKey(const Key('card-dismiss')));
    expect(closed, 1);
  });

  testWidgets('without_a_notice_the_region_takes_every_pointer', (
    WidgetTester tester,
  ) async {
    final FocusNode focus = FocusNode();
    addTearDown(focus.dispose);
    await pumpHost(tester, focusNode: focus, showing: false);
    final Rect region = tester.getRect(find.byKey(const Key('region')));
    // Die Ecke, in der sonst die Karte steht, gehört der Region.
    final Offset corner = region.topRight + const Offset(-40, 20);
    final HitTestResult result = tester.hitTestOnBinding(corner);
    expect(
      result.path.any(
        (HitTestEntry entry) =>
            entry.target ==
            tester.renderObject(find.byKey(const Key('region'))),
      ),
      isTrue,
    );
  });
}
