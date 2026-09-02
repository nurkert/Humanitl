import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// The fill of the `HButton` carrying [key]: the colour of the decoration of
/// its animated container, which is the button's background in the state the
/// button is currently in.
Color buttonFill(WidgetTester tester, String key) {
  final AnimatedContainer container = tester.widget<AnimatedContainer>(
    find.descendant(
      of: find.byKey(Key(key)),
      matching: find.byType(AnimatedContainer),
    ),
  );
  return (container.decoration! as BoxDecoration).color!;
}

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
            final AnimatedContainer container = tester
                .widget<AnimatedContainer>(
                  find.descendant(
                    of: find.byKey(Key(key)),
                    matching: find.byType(AnimatedContainer),
                  ),
                );
            return container.decoration! as BoxDecoration;
          }

          expect(decorationOf('plain').color, tokens.colors.bg2);
          expect(decorationOf('pressed').color, tokens.colors.bg3);
          expect(decorationOf('hovered').color, tokens.colors.bg3);
          expect(
            decorationOf('focused').border!.top.color,
            tokens.colors.accent,
          );
          expect(decorationOf('plain').border!.top.color, tokens.colors.line);

          // A real pointer on a previewed button changes nothing.
          final TestGesture gesture = await tester.createGesture(
            kind: PointerDeviceKind.mouse,
          );
          await gesture.addPointer(location: Offset.zero);
          addTearDown(gesture.removePointer);
          await gesture.moveTo(
            tester.getCenter(find.byKey(const Key('plain'))),
          );
          await tester.pump();
          expect(decorationOf('plain').color, tokens.colors.bg3);
          await gesture.moveTo(
            tester.getCenter(find.byKey(const Key('focused'))),
          );
          await tester.pump();
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
        expect(rest, tokens.colors.accent);
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
          // The label stays legible on the moved fill.
          expect(
            HColorDerivation.contrast(tokens.colors.onAccent, fill),
            greaterThanOrEqualTo(3.0),
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
          for (final Color fill in <Color>[rest, hover, pressed]) {
            expect(fill.r, closeTo(blocked.r, 1e-9));
            expect(fill.g, closeTo(blocked.g, 1e-9));
            expect(fill.b, closeTo(blocked.b, 1e-9));
            // The label is drawn in the blocked hue; it has to stay legible on
            // every fill over every surface the button can sit on.
            for (final Color surface in surfacesOf(brightness)) {
              final double ratio = HColorDerivation.contrast(
                blocked,
                HColorDerivation.flatten(fill, surface),
              );
              expect(
                ratio,
                greaterThanOrEqualTo(3.0),
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

      testWidgets('HRow grows when selected and reports taps and hover', (
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
        expect(tester.getSize(find.byType(HRow)).height, HSize.rowSelected);
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
        Color fill() {
          final AnimatedContainer container = tester.widget<AnimatedContainer>(
            find.descendant(
              of: find.byType(HRow),
              matching: find.byType(AnimatedContainer),
            ),
          );
          return (container.decoration! as BoxDecoration).color!;
        }

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
        await tester.pump();
        expect(fill(), tokens.colors.bg3);
        // bg1 is the panel colour; a hover in it would be invisible.
        expect(fill(), isNot(tokens.colors.bg1));
        await gesture.moveTo(outside);
        await tester.pump();
        expect(fill().a, 0, reason: 'the fill leaves with the pointer');

        await tester.pumpWidget(build(selected: true));
        await tester.pump();
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
          expect(icon.color, tokens.stateColor(state), reason: state.name);
          expect(icon.size, HSize.glyph, reason: state.name);
          // Without a countdown there is one CustomPaint: the icon itself.
          expect(
            inside(glyph, CustomPaint),
            findsOneWidget,
            reason: state.name,
          );
          expect(
            inside(glyph, AnimatedBuilder),
            findsNothing,
            reason: '${state.name} does not breathe',
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
          final HGlyphIcon icon = tester.widget<HGlyphIcon>(
            inside(glyph, HGlyphIcon),
          );
          expect(icon.glyph, HFlowState.held.glyph);
          expect(icon.size, lessThan(HSize.glyph));
          // Only under the breathing threshold does the glyph breathe.
          expect(
            inside(glyph, AnimatedBuilder),
            progress <= HMotion.breatheBelow ? findsOneWidget : findsNothing,
            reason: 'breathing at $progress',
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
