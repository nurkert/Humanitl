import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// The fill of the `HButton` carrying [key]: the colour of the decoration of
/// its animated container, which is the button's background in the state the
/// button is currently in.
Color buttonFill(WidgetTester tester, String key) => tester
    .widget<HAnimatedFill>(
      find.descendant(
        of: find.byKey(Key(key)),
        matching: find.byType(HAnimatedFill),
      ),
    )
    .color;

/// Die Deckkraft der ersten [FadeTransition], die [finder] findet.
double opacityOf(WidgetTester tester, Finder finder) =>
    tester.widget<FadeTransition>(finder.first).opacity.value;

/// The four surfaces of the ladder of [brightness].
List<Color> surfacesOf(Brightness brightness) => brightness == Brightness.dark
    ? HColorDerivation.darkSurfaces
    : HColorDerivation.lightSurfaces;

/// One `HButton` of [variant] per interaction state, keyed `rest`, `hovered`
/// and `pressed`, so a test can read all three fills from one tree.
Widget buttonStates(HButtonVariant variant) => Column(
  mainAxisSize: MainAxisSize.min,
  crossAxisAlignment: CrossAxisAlignment.start,
  children: <Widget>[
    for (final HButtonPreview? preview in <HButtonPreview?>[
      null,
      HButtonPreview.hovered,
      HButtonPreview.pressed,
    ])
      HButton(
        key: Key(preview?.name ?? 'rest'),
        variant: variant,
        preview: preview,
        onPressed: () {},
        child: Text(preview?.name ?? 'rest'),
      ),
  ],
);

void main() {
  for (final Brightness brightness in Brightness.values) {
    group('in ${brightness.name}', () {
      testWidgets('HButton renders and responds to a tap', (
        WidgetTester tester,
      ) async {
        int taps = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                for (final HButtonVariant variant in HButtonVariant.values)
                  HButton(
                    variant: variant,
                    onPressed: () => taps++,
                    child: Text(variant.name),
                  ),
                HButton(onPressed: null, child: const Text('disabled')),
              ],
            ),
          ),
        );
        expect(find.byType(HButton), findsNWidgets(5));
        for (final HButtonVariant variant in HButtonVariant.values) {
          await tester.tap(find.text(variant.name));
        }
        await tester.pump();
        expect(taps, HButtonVariant.values.length);
        await tester.tap(find.text('disabled'));
        await tester.pump();
        expect(taps, HButtonVariant.values.length);
        expect(tester.takeException(), isNull);
      });

      testWidgets(
        'HButton preview overrides the real pointer state',
        (WidgetTester tester) async {
          final HTokens tokens = HTokens.forBrightness(brightness);
          await tester.pumpWidget(
            harness(
              brightness: brightness,
              Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  HButton(
                    key: const Key('plain'),
                    onPressed: () {},
                    child: const Text('plain'),
                  ),
                  HButton(
                    key: const Key('pressed'),
                    preview: HButtonPreview.pressed,
                    onPressed: () {},
                    child: const Text('pressed'),
                  ),
                  HButton(
                    key: const Key('focused'),
                    preview: HButtonPreview.focused,
                    onPressed: () {},
                    child: const Text('focused'),
                  ),
                  HButton(
                    key: const Key('hovered'),
                    preview: HButtonPreview.hovered,
                    onPressed: () {},
                    child: const Text('hovered'),
                  ),
                ],
              ),
            ),
          );
          BoxDecoration decorationOf(String key) {
            final Container container = tester.widget<Container>(
              find.descendant(
                of: find.byKey(Key(key)),
                matching: find.byType(Container),
              ),
            );
            return container.decoration! as BoxDecoration;
          }

          expect(buttonFill(tester, 'plain'), tokens.colors.bg2);
          expect(buttonFill(tester, 'pressed'), tokens.colors.bg3);
          expect(buttonFill(tester, 'hovered'), tokens.colors.bg3);
          // Der Fokus färbt den vorhandenen Rahmen nicht um; er kommt als
          // zwei Pixel Akzent außerhalb (`docs/UX.md` 6).
          expect(decorationOf('focused').border!.top.color, tokens.colors.line);
          expect(decorationOf('plain').border!.top.color, tokens.colors.line);
          bool ringOf(String key) => tester
              .widget<HFocusRing>(
                find.descendant(
                  of: find.byKey(Key(key)),
                  matching: find.byType(HFocusRing),
                ),
              )
              .visible;
          expect(ringOf('focused'), isTrue);
          expect(ringOf('plain'), isFalse);
          expect(ringOf('hovered'), isFalse);

          // A real pointer on a previewed button changes nothing.
          final TestGesture gesture = await tester.createGesture(
            kind: PointerDeviceKind.mouse,
          );
          await gesture.addPointer(location: Offset.zero);
          addTearDown(gesture.removePointer);
          await gesture.moveTo(
            tester.getCenter(find.byKey(const Key('plain'))),
          );
          await tester.pumpAndSettle();
          // Die gemalte Farbe, nicht nur das Ziel: die Füllung ist nach
          // ihrer Dauer wirklich angekommen.
          expect(decorationOf('plain').color, tokens.colors.bg3);
          expect(buttonFill(tester, 'plain'), tokens.colors.bg3);
          await gesture.moveTo(
            tester.getCenter(find.byKey(const Key('focused'))),
          );
          await tester.pumpAndSettle();
          expect(decorationOf('focused').color, tokens.colors.bg2);
          expect(tester.takeException(), isNull);
        },
        // The test platform defaults to Android, where
        // FocusableActionDetector shows no hover highlight until a pointer
        // proves a mouse exists. The product runs on Linux; test it there.
        variant: TargetPlatformVariant.only(TargetPlatform.linux),
      );

      testWidgets('HButton primary hover steps away from the surface', (
        WidgetTester tester,
      ) async {
        final HTokens tokens = HTokens.forBrightness(brightness);
        await tester.pumpWidget(
          harness(brightness: brightness, buttonStates(HButtonVariant.primary)),
        );
        final Color rest = buttonFill(tester, 'rest');
        final Color hover = buttonFill(tester, 'hovered');
        final Color pressed = buttonFill(tester, 'pressed');
        // Die Füllung, nicht der Akzent: Weiß auf dem hellen Akzent misst
        // 3,73:1, und der Ruhezustand ist der Normalfall des einen gefüllten
        // Controls je Bildschirm (`docs/UX.md` 6).
        expect(rest, tokens.colors.accentFill);
        if (brightness == Brightness.light) {
          expect(rest, isNot(tokens.colors.accent));
        }
        expect(hover, isNot(rest));
        expect(pressed, isNot(rest));
        expect(pressed, isNot(hover));

        // Dark lightens on hover, light darkens: each theme moves the accent
        // away from the surfaces it sits on.
        final double restLightness = HSLColor.fromColor(rest).lightness;
        final double hoverLightness = HSLColor.fromColor(hover).lightness;
        if (brightness == Brightness.dark) {
          expect(hoverLightness, greaterThan(restLightness));
        } else {
          expect(hoverLightness, lessThan(restLightness));
          // The case the review named: the light hover fill on the highest
          // light surface.
          expect(
            HColorDerivation.contrast(hover, HColors.lBg3),
            greaterThanOrEqualTo(3.0),
          );
        }
        for (final Color fill in <Color>[hover, pressed]) {
          for (final Color surface in surfacesOf(brightness)) {
            final double ratio = HColorDerivation.contrast(fill, surface);
            expect(
              ratio,
              greaterThanOrEqualTo(3.0),
              reason:
                  '${HColorDerivation.toHex(fill)} on '
                  '${HColorDerivation.toHex(surface)} is '
                  '${ratio.toStringAsFixed(2)}',
            );
          }
        }
        // Die Beschriftung steht auf allen drei Füllungen, der ruhenden
        // zuerst: sie ist der Normalfall, nicht der Sonderfall. Und sie ist
        // Text, also gilt 4,5:1 und nicht die 3:1 einer Fläche
        // (`docs/UX.md` 6).
        for (final Color fill in <Color>[rest, hover, pressed]) {
          final double ratio = HColorDerivation.contrast(
            tokens.colors.onAccent,
            fill,
          );
          expect(
            ratio,
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason:
                'onAccent on ${HColorDerivation.toHex(fill)} is '
                '${ratio.toStringAsFixed(2)}',
          );
        }
        expect(tester.takeException(), isNull);
      });

      testWidgets(
        'HButton danger fills step from rest through hover to press',
        (WidgetTester tester) async {
          final HTokens tokens = HTokens.forBrightness(brightness);
          final Color blocked = tokens.state.blocked;
          await tester.pumpWidget(
            harness(
              brightness: brightness,
              buttonStates(HButtonVariant.danger),
            ),
          );
          final Color rest = buttonFill(tester, 'rest');
          final Color hover = buttonFill(tester, 'hovered');
          final Color pressed = buttonFill(tester, 'pressed');
          // Three fills, one hue, rising alpha; rest obeys the tint cap.
          expect(<int>{
            rest.toARGB32(),
            hover.toARGB32(),
            pressed.toARGB32(),
          }, hasLength(3));
          expect(rest.a, closeTo(HColors.tintAlpha, 1e-6));
          expect(hover.a, greaterThan(rest.a));
          expect(pressed.a, greaterThan(hover.a));
          // Die Farbe, die das Widget wirklich malt, an der Schwelle, die
          // für Text gilt. Vorher stand hier die Flächenfarbe gegen 3,0, und
          // der Test wäre grün geblieben, wenn jemand auf sie zurückfiele
          // (`docs/UX.md` 6).
          final Color label = tokens.stateTextColor(HFlowState.blocked);
          expect(
            tester
                .widget<DefaultTextStyle>(
                  find
                      .descendant(
                        of: find.byKey(const Key('rest')),
                        matching: find.byType(DefaultTextStyle),
                      )
                      .first,
                )
                .style
                .color,
            label,
          );
          for (final Color fill in <Color>[rest, hover, pressed]) {
            expect(fill.r, closeTo(blocked.r, 1e-9));
            expect(fill.g, closeTo(blocked.g, 1e-9));
            expect(fill.b, closeTo(blocked.b, 1e-9));
            for (final Color surface in surfacesOf(brightness)) {
              final double ratio = HColorDerivation.contrast(
                label,
                HColorDerivation.flatten(fill, surface),
              );
              expect(
                ratio,
                greaterThanOrEqualTo(HColorDerivation.textMinContrast),
                reason:
                    'label on alpha ${fill.a.toStringAsFixed(2)} over '
                    '${HColorDerivation.toHex(surface)} is '
                    '${ratio.toStringAsFixed(2)}',
              );
            }
          }
          expect(tester.takeException(), isNull);
        },
      );

      testWidgets('HBadge and HMethodBadge render and can be tapped', (
        WidgetTester tester,
      ) async {
        int taps = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Row(
              children: <Widget>[
                HBadge(text: 'jwt', onTap: () => taps++),
                const HMethodBadge(method: 'delete'),
              ],
            ),
          ),
        );
        expect(find.text('jwt'), findsOneWidget);
        expect(find.text('DELETE'), findsOneWidget);
        await tester.tap(find.text('jwt'));
        expect(taps, 1);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HPill reports left, right and hold', (
        WidgetTester tester,
      ) async {
        int left = 0;
        int right = 0;
        int held = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Row(
              children: <Widget>[
                HPill(
                  left: const Text('Allow'),
                  onLeft: () => left++,
                  onRight: () => right++,
                  onLeftLongPress: () => held++,
                  leftSemanticsLabel: 'allow once',
                  rightSemanticsLabel: 'allow scope',
                ),
              ],
            ),
          ),
        );
        await tester.tap(find.text('Allow'));
        await tester.pump();
        expect(left, 1);
        expect(held, 0);

        await tester.longPress(find.text('Allow'));
        await tester.pump();
        expect(held, 1);
        expect(left, 1);

        await tester.tap(find.byType(HGlyphIcon).last);
        await tester.pump();
        expect(right, 1);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HPill writes label and chevron in the text variant', (
        WidgetTester tester,
      ) async {
        final HTokens tokens = HTokens.forBrightness(brightness);
        final Color area = tokens.state.held;
        final Color label = tokens.stateTextColor(HFlowState.held);
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Row(
              children: <Widget>[
                HPill(
                  accent: area,
                  left: const Text('Hold'),
                  onLeft: () {},
                  onRight: () {},
                  leftSemanticsLabel: 'hold',
                  rightSemanticsLabel: 'scope',
                ),
              ],
            ),
          ),
        );
        await tester.pump();
        // Die Fläche bleibt die Zustandsfarbe, das Wort nimmt ihre
        // Textvariante (`docs/UX.md` 6).
        final DefaultTextStyle style = tester.widget<DefaultTextStyle>(
          find
              .descendant(
                of: find.byType(HPill),
                matching: find.byType(DefaultTextStyle),
              )
              .first,
        );
        expect(style.style.color, label);
        final HGlyphIcon chevron = tester.widget<HGlyphIcon>(
          find.descendant(
            of: find.byType(HPill),
            matching: find.byType(HGlyphIcon),
          ),
        );
        expect(chevron.color, label);
        final BoxDecoration outer =
            tester
                    .widget<DecoratedBox>(
                      find
                          .descendant(
                            of: find.byType(HPill),
                            matching: find.byType(DecoratedBox),
                          )
                          .first,
                    )
                    .decoration
                as BoxDecoration;
        expect(outer.color, HColorDerivation.tint(area, HColors.fillRestAlpha));
        // Der Rahmen trägt denselben Ton wie das Wort. Mit
        // `fade(accent, 0.4)` maß er gegen die eigene Füllung 1,47:1 und war
        // damit keine Kante, sondern ein Hauch (`docs/UX.md` 6).
        expect((outer.border! as Border).top.color, label);
        // Die Ruhefüllung, und die Haltefüllung **über** ihr: zwei Schichten
        // derselben Farbe addieren sich nicht, sie komponieren. 0,20 über
        // 0,06 wären wirksam 0,248, und dort misst die Beschriftung 4,14:1.
        for (final Color surface in surfacesOf(brightness)) {
          final Color rest = HColorDerivation.flatten(
            area.withValues(alpha: HColors.fillRestAlpha),
            surface,
          );
          final Color held = HColorDerivation.flatten(
            area.withValues(
              alpha: HColorDerivation.alphaOver(
                HColors.fillHoldAlpha,
                HColors.fillRestAlpha,
              ),
            ),
            rest,
          );
          // Die zusammengesetzte Fläche ist genau die, für die die
          // Textableitung geradesteht.
          final Color canonical = HColorDerivation.flatten(
            area.withValues(alpha: HColors.fillHoldAlpha),
            surface,
          );
          expect(held.r, closeTo(canonical.r, 1 / 255));
          expect(held.g, closeTo(canonical.g, 1 / 255));
          expect(held.b, closeTo(canonical.b, 1 / 255));
          for (final Color fill in <Color>[rest, held]) {
            final double ratio = HColorDerivation.contrast(label, fill);
            expect(
              ratio,
              greaterThanOrEqualTo(HColorDerivation.textMinContrast),
              reason:
                  'label on ${HColorDerivation.toHex(fill)} over '
                  '${HColorDerivation.toHex(surface)} is '
                  '${ratio.toStringAsFixed(2)}',
            );
            // Und der Rahmen ist eine Fläche: 3:1 gegen die Füllung, die er
            // umschließt.
            expect(
              HColorDerivation.contrast(label, fill),
              greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
            );
          }
        }
        expect(tester.takeException(), isNull);
      });

      testWidgets('HPill fills its hold with what the token says', (
        WidgetTester tester,
      ) async {
        // Nicht gerechnet, sondern gelesen: die Farbe, die die Pille wirklich
        // in ihren Verlauf legt, komponiert über ihrer eigenen Ruhefläche zu
        // `HColors.fillHoldAlpha`.
        final HTokens tokens = HTokens.forBrightness(brightness);
        final Color area = tokens.state.allowed;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Row(
              children: <Widget>[
                HPill(
                  accent: area,
                  left: const Text('Allow'),
                  onLeft: () {},
                  onLeftLongPress: () {},
                  leftSemanticsLabel: 'allow once',
                ),
              ],
            ),
          ),
        );
        final TestGesture gesture = await tester.startGesture(
          tester.getCenter(find.text('Allow')),
        );
        await tester.pump();
        await tester.pump(HMotion.holdToConfirm ~/ 2);
        final LinearGradient gradient =
            tester
                    .widgetList<DecoratedBox>(
                      find.descendant(
                        of: find.byType(HPill),
                        matching: find.byType(DecoratedBox),
                      ),
                    )
                    .map((DecoratedBox box) => box.decoration as BoxDecoration)
                    .firstWhere((BoxDecoration d) => d.gradient != null)
                    .gradient!
                as LinearGradient;
        final Color painted = gradient.colors.first;
        expect(
          painted.a,
          closeTo(
            HColorDerivation.alphaOver(
              HColors.fillHoldAlpha,
              HColors.fillRestAlpha,
            ),
            1e-6,
          ),
        );
        for (final Color surface in surfacesOf(brightness)) {
          final Color rest = HColorDerivation.flatten(
            area.withValues(alpha: HColors.fillRestAlpha),
            surface,
          );
          final Color held = HColorDerivation.flatten(painted, rest);
          final double ratio = HColorDerivation.contrast(
            tokens.stateTextColor(HFlowState.allowed),
            held,
          );
          expect(
            ratio,
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason:
                'label on the composed hold over '
                '${HColorDerivation.toHex(surface)} is '
                '${ratio.toStringAsFixed(2)}',
          );
        }
        await gesture.up();
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      });

      testWidgets('HPill holds with the keyboard, and taps with it too', (
        WidgetTester tester,
      ) async {
        // Press-and-hold hat eine Taste, sonst ist das Signature-Element ohne
        // Maus unbenutzbar (`docs/UX.md` 5.1).
        int left = 0;
        int held = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            keyboard(
              HPill(
                left: const Text('Allow'),
                onLeft: () => left++,
                onLeftLongPress: () => held++,
                leftSemanticsLabel: 'allow once',
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        FocusScope.of(tester.element(find.byType(HPill))).nextFocus();
        await tester.pumpAndSettle();

        // Kurz gedrückt: die einfache Handlung.
        await tester.sendKeyDownEvent(LogicalKeyboardKey.enter);
        await tester.pump(HMotion.holdToConfirm ~/ 4);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.enter);
        await tester.pumpAndSettle();
        expect(left, 1);
        expect(held, 0);

        // Gehalten: die gemerkte Variante, genau einmal.
        await tester.sendKeyDownEvent(LogicalKeyboardKey.space);
        await tester.pump(HMotion.holdToConfirm);
        await tester.pump(HMotion.holdToConfirm);
        await tester.sendKeyUpEvent(LogicalKeyboardKey.space);
        await tester.pumpAndSettle();
        expect(held, 1);
        expect(left, 1, reason: 'ein Halten ist kein zweiter Klick');
        expect(tester.takeException(), isNull);
      });

      testWidgets('HBadge takes its label from the text palette', (
        WidgetTester tester,
      ) async {
        final HTokens tokens = HTokens.forBrightness(brightness);
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                HBadge(text: 'own', color: tokens.state.blocked),
                // Eine Konstante des *anderen* Themes: der Aufrufer, der
                // `HColors.blocked` schreibt statt `tokens.state.blocked`,
                // bekam vorher eine Beschriftung bei rund 2,5:1.
                const HBadge(text: 'foreign', color: HColors.blocked),
              ],
            ),
          ),
        );
        await tester.pump();
        final Color label = tokens.stateTextColor(HFlowState.blocked);
        for (final String text in <String>['own', 'foreign']) {
          expect(
            tester.widget<Text>(find.text(text)).style?.color,
            label,
            reason: text,
          );
        }
        expect(tester.takeException(), isNull);
      });

      testWidgets('HRow keeps its height, reports taps, hover and focus', (
        WidgetTester tester,
      ) async {
        int taps = 0;
        bool hovered = false;
        Widget build({required bool selected}) => harness(
          brightness: brightness,
          HRow(
            state: HFlowState.held,
            selected: selected,
            onTap: () => taps++,
            onHover: (bool value) => hovered = value,
            leading: const HStateGlyph(state: HFlowState.held, progress: 0.4),
            title: const Text('registry.npmjs.org'),
            subtitle: const Text('/left-pad'),
            trailing: const HMethodBadge(method: 'GET'),
            semanticsLabel: 'held flow',
          ),
        );

        await tester.pumpWidget(build(selected: false));
        await tester.pumpAndSettle();
        expect(tester.getSize(find.byType(HRow)).height, HSize.row);
        expect(find.text('/left-pad'), findsNothing);

        await tester.tap(find.byType(HRow));
        await tester.pump();
        expect(taps, 1);

        await tester.pumpWidget(build(selected: true));
        await tester.pumpAndSettle();
        // Die Zeile wächst nicht, weil sich ihr Zustand ändert: 36 px in
        // jedem Zustand (`docs/UX.md` 2.9 und 3.4).
        expect(tester.getSize(find.byType(HRow)).height, HSize.row);
        expect(find.text('/left-pad'), findsOneWidget);

        final TestGesture gesture = await tester.createGesture(
          kind: PointerDeviceKind.mouse,
        );
        await gesture.addPointer(location: Offset.zero);
        addTearDown(gesture.removePointer);
        await gesture.moveTo(tester.getCenter(find.byType(HRow)));
        await tester.pump();
        expect(hovered, isTrue);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HRow fills on hover and on selection, never with bg1', (
        WidgetTester tester,
      ) async {
        final HTokens tokens = HTokens.forBrightness(brightness);
        Widget build({required bool selected}) => harness(
          brightness: brightness,
          HRow(
            state: HFlowState.held,
            selected: selected,
            onTap: () {},
            title: const Text('registry.npmjs.org'),
            semanticsLabel: 'held flow',
          ),
        );
        Color fill() => tester
            .widget<HAnimatedFill>(
              find.descendant(
                of: find.byType(HRow),
                matching: find.byType(HAnimatedFill),
              ),
            )
            .color;

        await tester.pumpWidget(build(selected: false));
        await tester.pump();
        expect(fill().a, 0, reason: 'a row at rest is transparent');

        // The pointer starts outside the row, enters it, and leaves again.
        const Offset outside = Offset(700, 500);
        final TestGesture gesture = await tester.createGesture(
          kind: PointerDeviceKind.mouse,
        );
        await gesture.addPointer(location: outside);
        addTearDown(gesture.removePointer);
        await tester.pump();
        expect(fill().a, 0);
        await gesture.moveTo(tester.getCenter(find.byType(HRow)));
        await tester.pumpAndSettle();
        // Hover bg2, Auswahl bg3, nie dieselbe Farbe (`docs/UX.md` 3.4).
        expect(fill(), tokens.colors.bg2);
        expect(fill(), isNot(tokens.colors.bg3));
        // bg1 is the panel colour; a hover in it would be invisible.
        expect(fill(), isNot(tokens.colors.bg1));
        await gesture.moveTo(outside);
        await tester.pumpAndSettle();
        expect(fill().a, 0, reason: 'the fill leaves with the pointer');

        await tester.pumpWidget(build(selected: true));
        await tester.pumpAndSettle();
        expect(fill(), tokens.colors.bg3);
        expect(
          find.descendant(
            of: find.byType(HRow),
            matching: find.byWidgetPredicate(
              (Widget widget) =>
                  widget is ColoredBox && widget.color == tokens.colors.accent,
            ),
          ),
          findsOneWidget,
          reason: 'selection is told apart by the accent rail',
        );
        expect(tester.takeException(), isNull);
      });

      testWidgets('HPanel shows its title and its actions', (
        WidgetTester tester,
      ) async {
        int taps = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            HPanel(
              title: const Text('Isolation'),
              actions: <Widget>[
                HIconButton(
                  glyph: HGlyph.close,
                  onPressed: () => taps++,
                  semanticsLabel: 'dismiss',
                ),
              ],
              child: const Text('three guarantees'),
            ),
          ),
        );
        expect(find.text('Isolation'), findsOneWidget);
        expect(find.text('three guarantees'), findsOneWidget);
        await tester.tap(find.byType(HIconButton));
        expect(taps, 1);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HModal dismisses on the scrim', (WidgetTester tester) async {
        int dismissed = 0;
        int confirmed = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            SizedBox(
              height: 400,
              child: HModal(
                title: const Text('Delete forever rule?'),
                onDismiss: () => dismissed++,
                scrimSemanticsLabel: 'dismiss modal',
                actions: <Widget>[
                  HButton(
                    variant: HButtonVariant.danger,
                    onPressed: () => confirmed++,
                    child: const Text('Delete'),
                  ),
                ],
                child: const Text('This cannot be undone.'),
              ),
            ),
          ),
        );
        expect(find.text('Delete forever rule?'), findsOneWidget);
        await tester.tap(find.text('Delete'));
        expect(confirmed, 1);
        await tester.tapAt(const Offset(4, 4));
        expect(dismissed, 1);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HSheet closes', (WidgetTester tester) async {
        int closed = 0;
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            SizedBox(
              height: 300,
              child: HSheet(
                title: const Text('Rule from request'),
                onClose: () => closed++,
                closeSemanticsLabel: 'close sheet',
                child: const Text('allow · GET · **.npmjs.org · session'),
              ),
            ),
          ),
        );
        expect(find.text('Rule from request'), findsOneWidget);
        await tester.tap(find.byType(HIconButton));
        expect(closed, 1);
        expect(tester.takeException(), isNull);
      });

      testWidgets('HStateGlyph paints every state', (
        WidgetTester tester,
      ) async {
        final HTokens tokens = HTokens.forBrightness(brightness);
        // Disposed at the end of the body: the framework checks for live
        // handles before the teardowns run.
        final SemanticsHandle semantics = tester.ensureSemantics();
        const List<double> rings = <double>[1.0, 0.5, 0.15];
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            Wrap(
              children: <Widget>[
                for (final HFlowState state in HFlowState.values)
                  HStateGlyph(state: state, semanticsLabel: state.l10nKey),
                for (final double progress in rings)
                  HStateGlyph(state: HFlowState.held, progress: progress),
              ],
            ),
          ),
        );
        await tester.pump();
        expect(
          find.byType(HStateGlyph),
          findsNWidgets(HFlowState.values.length + rings.length),
        );

        Finder glyphOf(HFlowState state, {double? progress}) =>
            find.byWidgetPredicate(
              (Widget widget) =>
                  widget is HStateGlyph &&
                  widget.state == state &&
                  widget.progress == progress,
            );
        Finder inside(Finder glyph, Type type) =>
            find.descendant(of: glyph, matching: find.byType(type));

        for (final HFlowState state in HFlowState.values) {
          final Finder glyph = glyphOf(state);
          expect(glyph, findsOneWidget, reason: state.name);
          expect(
            tester.getSemantics(glyph).label,
            state.l10nKey,
            reason: state.name,
          );
          final HGlyphIcon icon = tester.widget<HGlyphIcon>(
            inside(glyph, HGlyphIcon),
          );
          expect(icon.glyph, state.glyph, reason: state.name);
          // Das Glyph ist eine Grafik und nimmt die Flächenfarbe: seine
          // Grenze ist die 3:1, und die Textvariante nähme `autoRule` den
          // Ton, den das Glyph gerade verdoppeln soll (`docs/UX.md` 3.3, 6).
          expect(icon.color, tokens.stateColor(state), reason: state.name);
          expect(icon.accentColor, tokens.colors.accent, reason: state.name);
          for (final Color surface in surfacesOf(brightness)) {
            final double ratio = HColorDerivation.contrast(
              HColorDerivation.flatten(icon.color!, surface),
              surface,
            );
            expect(
              ratio,
              greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
              reason:
                  '${state.name} glyph on '
                  '${HColorDerivation.toHex(surface)} is '
                  '${ratio.toStringAsFixed(2)}',
            );
          }
          expect(icon.size, HSize.glyph, reason: state.name);
          // Without a countdown there is one CustomPaint: the icon itself.
          expect(
            inside(glyph, CustomPaint),
            findsOneWidget,
            reason: state.name,
          );
          expect(
            opacityOf(tester, inside(glyph, FadeTransition)),
            1.0,
            reason: '${state.name} stands at full opacity',
          );
        }

        for (final double progress in rings) {
          final Finder glyph = glyphOf(HFlowState.held, progress: progress);
          expect(glyph, findsOneWidget, reason: 'ring at $progress');
          // The ring is a second CustomPaint, the size of the whole glyph,
          // painted before the icon, which shrinks to make room for it.
          final Finder paints = inside(glyph, CustomPaint);
          expect(
            paints,
            findsNWidgets(2),
            reason: 'ring and icon at $progress',
          );
          final CustomPaint ring = tester.widget<CustomPaint>(paints.first);
          expect(ring.size, const Size.square(HSize.glyph));
          expect(ring.painter, isNotNull);
          // Der Bogen bleibt die Fläche: 3:1 gegen jede Fläche, auf der er
          // liegt, und nicht die Textvariante des Glyphs.
          for (final Color surface in surfacesOf(brightness)) {
            expect(
              HColorDerivation.contrast(
                tokens.stateColor(HFlowState.held),
                surface,
              ),
              greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
            );
          }
          final HGlyphIcon icon = tester.widget<HGlyphIcon>(
            inside(glyph, HGlyphIcon),
          );
          expect(icon.glyph, HFlowState.held.glyph);
          expect(icon.size, lessThan(HSize.glyph));
          // Ein Glyph, das schon unter der Schwelle ankommt, holt die drei
          // Züge nicht nach: der Atem gehört dem Übergang (`docs/UX.md` 2.7).
          expect(
            opacityOf(tester, inside(glyph, FadeTransition)),
            1.0,
            reason: 'no breath without a crossing at $progress',
          );
          // An unlabelled glyph is decorative.
          expect(inside(glyph, ExcludeSemantics), findsWidgets);
        }
        expect(tester.takeException(), isNull);
        // The breathing glyph keeps ticking; a frame later it is still there.
        await tester.pump(const Duration(milliseconds: 600));
        expect(
          find.byType(HStateGlyph),
          findsNWidgets(HFlowState.values.length + rings.length),
        );
        expect(tester.takeException(), isNull);
        semantics.dispose();
      });

      testWidgets('HHairline draws in both directions', (
        WidgetTester tester,
      ) async {
        await tester.pumpWidget(
          harness(
            brightness: brightness,
            SizedBox(
              height: 40,
              child: Row(
                children: const <Widget>[
                  HHairline(vertical: true),
                  Expanded(child: HHairline()),
                  HHairline(vertical: true, strong: true, length: 20),
                ],
              ),
            ),
          ),
        );
        final List<Size> sizes = find
            .byType(HHairline)
            .evaluate()
            .map((Element e) => (e.renderObject! as RenderBox).size)
            .toList();
        expect(sizes[0].width, HSize.hairline);
        expect(sizes[0].height, 40);
        expect(sizes[1].height, HSize.hairline);
        expect(sizes[2].height, 20);
        expect(tester.takeException(), isNull);
      });
    });
  }

  group('der Atem', () {
    /// Die Deckkraft des Glyphs.
    double breath(WidgetTester tester) =>
        opacityOf(tester, find.byType(FadeTransition));

    /// Ein Glyph, dessen Restfrist [progress] beträgt.
    Widget glyph(double progress) =>
        harness(HStateGlyph(state: HFlowState.held, progress: progress));

    testWidgets('ends at full opacity, not at its floor', (
      WidgetTester tester,
    ) async {
      // `repeat(count:)` zählt Halbdurchläufe. Mit einer ungeraden Zahl endet
      // der Controller auf 1,0 — auf der geringsten Deckkraft —, und die
      // dringendste Zeile bleibt dauerhaft die blasseste (`docs/UX.md` 2.7).
      await tester.pumpWidget(glyph(0.5));
      await tester.pump();
      expect(breath(tester), 1.0);
      await tester.pumpWidget(glyph(0.1));
      await tester.pump();
      await tester.pump(HMotion.breathe ~/ 2);
      expect(breath(tester), lessThan(1.0), reason: 'der Atem läuft überhaupt');
      expect(
        breath(tester),
        greaterThanOrEqualTo(HMotion.breatheMinOpacity),
        reason: 'und nie unter seine Untergrenze',
      );
      await tester.pumpAndSettle();
      expect(
        breath(tester),
        greaterThan(0.99),
        reason: 'nach drei Zügen steht das Glyph wieder voll da',
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('the arc is a gap, and reduced motion doubles the ring', (
      WidgetTester tester,
    ) async {
      Finder ring() => find
          .descendant(
            of: find.byType(HStateGlyph),
            matching: find.byType(CustomPaint),
          )
          .first;
      // Nur der verbleibende Bogen, keine Spur darunter
      // (`docs/UX.md` 9, Punkt 6).
      await tester.pumpWidget(glyph(0.5));
      await tester.pump();
      expect(ring(), paintsExactlyCountTimes(#drawArc, 1));

      // Unter reduzierter Bewegung entfällt der Atem, und an seine Stelle
      // tritt der zweite, ruhende Ring (2.10).
      Widget still(double progress) => Directionality(
        textDirection: TextDirection.ltr,
        child: MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: HTheme(
            tokens: HTokens.dark,
            child: Align(
              alignment: Alignment.topLeft,
              child: HStateGlyph(state: HFlowState.held, progress: progress),
            ),
          ),
        ),
      );
      await tester.pumpWidget(still(0.5));
      await tester.pump();
      expect(ring(), paintsExactlyCountTimes(#drawArc, 1));
      await tester.pumpWidget(still(0.1));
      await tester.pump(HMotion.breathe ~/ 2);
      expect(
        breath(tester),
        1.0,
        reason: 'keine Schleife unter reduzierter Bewegung',
      );
      expect(ring(), paintsExactlyCountTimes(#drawArc, 2));
      expect(tester.takeException(), isNull);
    });
  });

  testWidgets('the hold of HPill keeps its time under reduced motion', (
    WidgetTester tester,
  ) async {
    // Ohne `AnimationBehavior.preserve` skaliert Flutter die Dauer auf fünf
    // Prozent, sobald die Plattform `disableAnimations` meldet: die Füllung
    // stünde nach 20 ms voll da, während das Halten noch 380 ms braucht
    // (`docs/UX.md` 2.10 und 5.4).
    tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(
      tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    int held = 0;
    await tester.pumpWidget(
      harness(
        Row(
          children: <Widget>[
            HPill(
              left: const Text('Allow'),
              onLeft: () {},
              onLeftLongPress: () => held++,
              leftSemanticsLabel: 'allow once',
            ),
          ],
        ),
      ),
    );
    double fill() {
      final BoxDecoration decoration = tester
          .widgetList<DecoratedBox>(
            find.descendant(
              of: find.byType(HPill),
              matching: find.byType(DecoratedBox),
            ),
          )
          .map((DecoratedBox box) => box.decoration as BoxDecoration)
          .firstWhere(
            (BoxDecoration d) => d.gradient != null,
            orElse: () => const BoxDecoration(),
          );
      final Gradient? gradient = decoration.gradient;
      return gradient == null ? 0 : (gradient as LinearGradient).stops![1];
    }

    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(find.text('Allow')),
    );
    await tester.pump();
    await tester.pump(HMotion.holdToConfirm ~/ 4);
    expect(
      fill(),
      closeTo(0.25, 0.1),
      reason: 'die Füllung zeigt die verstrichene Zeit, nicht das Ende',
    );
    expect(held, 0);
    await tester.pump(HMotion.holdToConfirm);
    expect(held, 1);
    await gesture.up();
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
  });

  testWidgets('widgets fall back to the dark tokens without a theme', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      const Directionality(
        textDirection: TextDirection.ltr,
        child: Align(
          alignment: Alignment.topLeft,
          child: HBadge(text: 'no theme'),
        ),
      ),
    );
    expect(find.text('no theme'), findsOneWidget);
    // The fallback is the dark token set, not merely "something renders".
    final Text label = tester.widget<Text>(find.text('no theme'));
    expect(label.style?.color, HTokens.dark.colors.fg1);
    expect(tester.takeException(), isNull);
  });

  testWidgets('HTheme publishes the tokens it was given', (
    WidgetTester tester,
  ) async {
    late HTokens seen;
    await tester.pumpWidget(
      Directionality(
        textDirection: TextDirection.ltr,
        child: HTheme.light(
          child: Builder(
            builder: (BuildContext context) {
              seen = HTheme.of(context);
              return const SizedBox.shrink();
            },
          ),
        ),
      ),
    );
    expect(seen.brightness, Brightness.light);
    expect(HTheme.maybeOf(tester.element(find.byType(SizedBox))), seen);
  });
}
