import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// The four light surfaces a state colour is painted on: the application
/// background, the panel, and the two raised surfaces a selected row, a
/// secondary button and a modal use.
const List<Color> lightSurfaces = <Color>[
  HColors.lBg0,
  HColors.lBg1,
  HColors.lBg2,
  HColors.lBg3,
];

/// The four dark surfaces a state colour is painted on.
const List<Color> darkSurfaces = <Color>[
  HColors.bg0,
  HColors.bg1,
  HColors.bg2,
  HColors.bg3,
];

/// The light palette as it is *expected* to derive. Frozen on purpose: the
/// derivation in `HColorDerivation.lightState` is the only producer of these
/// values, so a test that recomputed them could not fail. A change to the
/// derivation shows up here as a diff to review, not as a silent re-statement.
const Map<HFlowState, int> frozenLightStates = <HFlowState, int>{
  HFlowState.held: 0xFFAC801D,
  HFlowState.allowed: 0xFF36956A,
  HFlowState.allowedEdited: 0xFF3E957E,
  HFlowState.blocked: 0xFFDC303D,
  HFlowState.timedOut: 0xFF6A7185,
  HFlowState.autoRule: 0x991C4E38,
  HFlowState.passthroughLlm: 0xFF9154E9,
  HFlowState.error: 0xFFEB4D17,
};

/// WCAG 2.1 contrast, written out here so the test does not lean on the
/// package's own implementation of the formula.
double wcagContrast(Color foreground, Color background) {
  final Color flat = Color.alphaBlend(foreground, background);
  final double a = flat.computeLuminance();
  final double b = background.computeLuminance();
  return (a > b ? a + 0.05 : b + 0.05) / (a > b ? b + 0.05 : a + 0.05);
}

/// The lowest contrast [color] reaches over any of [surfaces].
double worstOf(Color color, List<Color> surfaces) => surfaces
    .map((Color surface) => wcagContrast(color, surface))
    .reduce((double a, double b) => a < b ? a : b);

/// The fill of the outermost `ColoredBox` of the gallery: its application
/// background, `bg0` of whichever theme is active.
Color galleryBackground(WidgetTester tester) => tester
    .widget<ColoredBox>(
      find
          .descendant(
            of: find.byType(HGalleryPage),
            matching: find.byType(ColoredBox),
          )
          .first,
    )
    .color;

void main() {
  group('state colours', () {
    test('every_state_has_distinct_color_dark', () {
      final Set<int> seen = HStateColors.dark.all
          .map((Color color) => color.toARGB32())
          .toSet();
      expect(seen, hasLength(HFlowState.values.length));
    });

    test('every_state_has_distinct_color_light', () {
      final Set<int> seen = HStateColors.light.all
          .map((Color color) => color.toARGB32())
          .toSet();
      expect(seen, hasLength(HFlowState.values.length));
    });

    test('every state resolves in both themes', () {
      for (final HFlowState state in HFlowState.values) {
        final Color dark = FlowStateColor.of(state, Brightness.dark);
        final Color light = FlowStateColor.of(state, Brightness.light);
        expect(dark, isNot(light), reason: '${state.name} must be theme aware');
        expect(state.color(Brightness.dark), dark);
        expect(state.color(Brightness.light), light);
        expect(HTokens.dark.stateColor(state), dark);
        expect(HTokens.light.stateColor(state), light);
      }
    });

    test('state_colors_contrast_on_bg1', () {
      // The issue names bg1 and lBg1; the widgets also paint state colours on
      // bg2 and bg3 (selected row, secondary button, modal), so every surface
      // of the ladder is measured. bg1 and lBg1 are asserted on their own too,
      // so the acceptance criterion stays visible as a line of its own.
      for (final HFlowState state in HFlowState.values) {
        final Color dark = FlowStateColor.of(state, Brightness.dark);
        final Color light = FlowStateColor.of(state, Brightness.light);
        expect(
          wcagContrast(dark, HColors.bg1),
          greaterThanOrEqualTo(3.0),
          reason: '${state.name} on bg1',
        );
        expect(
          wcagContrast(light, HColors.lBg1),
          greaterThanOrEqualTo(3.0),
          reason: '${state.name} on lBg1',
        );
        final double darkWorst = worstOf(dark, darkSurfaces);
        expect(
          darkWorst,
          greaterThanOrEqualTo(3.0),
          reason:
              '${state.name} dark contrast is ${darkWorst.toStringAsFixed(2)}',
        );
        final double lightWorst = worstOf(light, lightSurfaces);
        expect(
          lightWorst,
          greaterThanOrEqualTo(3.0),
          reason:
              '${state.name} light contrast is '
              '${lightWorst.toStringAsFixed(2)}',
        );
      }
    });

    test('the derivation clamps against every surface of the ladder', () {
      expect(HColorDerivation.lightSurfaces, lightSurfaces);
      expect(HColorDerivation.darkSurfaces, darkSurfaces);
      // A clamp against the two lightest surfaces only is not enough: held
      // would pass on white and fail on the raised surfaces.
      final Color heldOnWhite = HColorDerivation.lightState(
        HColors.held,
        surfaces: const <Color>[HColors.lBg0, HColors.lBg1],
      );
      expect(worstOf(heldOnWhite, lightSurfaces), lessThan(3.0));
      expect(
        worstOf(HColorDerivation.lightState(HColors.held), lightSurfaces),
        greaterThanOrEqualTo(3.0),
      );
    });

    test('light states match the frozen table', () {
      for (final HFlowState state in HFlowState.values) {
        final Color light = FlowStateColor.of(state, Brightness.light);
        expect(
          light.toARGB32(),
          frozenLightStates[state],
          reason:
              '${state.name} derived to ${HColorDerivation.toHex(light)}; '
              'if the derivation changed on purpose, update the table',
        );
      }
    });

    test('light states are derived, never hand written', () {
      const Map<HFlowState, Color> sources = <HFlowState, Color>{
        HFlowState.held: HColors.held,
        HFlowState.allowed: HColors.allowed,
        HFlowState.allowedEdited: HColors.allowedEdited,
        HFlowState.blocked: HColors.blocked,
        HFlowState.timedOut: HColors.timedOut,
        HFlowState.autoRule: HColors.autoRule,
        HFlowState.passthroughLlm: HColors.passthrough,
        HFlowState.error: HColors.secret,
      };
      sources.forEach((HFlowState state, Color dark) {
        expect(
          HStateColors.light.resolve(state),
          HColorDerivation.lightState(dark),
          reason: state.name,
        );
        // The derivation never lightens and it keeps the alpha.
        expect(
          HSLColor.fromColor(HStateColors.light.resolve(state)).lightness,
          lessThanOrEqualTo(HSLColor.fromColor(dark).lightness - 0.12 + 0.01),
        );
        expect(HStateColors.light.resolve(state).a, closeTo(dark.a, 1e-6));
      });
    });

    test('auto rule is the allowed hue at sixty percent', () {
      expect(HColors.autoRule.a, closeTo(HColors.autoRuleOpacity, 0.01));
      expect(HColors.autoRule.r, closeTo(HColors.allowed.r, 1e-9));
      expect(HColors.autoRule.g, closeTo(HColors.allowed.g, 1e-9));
      expect(HColors.autoRule.b, closeTo(HColors.allowed.b, 1e-9));
    });

    test('allowed edited is allowed pulled towards the accent', () {
      final Color expected = Color.lerp(
        HColors.allowed,
        HColors.accent,
        HColors.allowedEditedBlend,
      )!;
      expect((HColors.allowedEdited.r - expected.r).abs(), lessThan(0.005));
      expect((HColors.allowedEdited.g - expected.g).abs(), lessThan(0.005));
      expect((HColors.allowedEdited.b - expected.b).abs(), lessThan(0.005));
      expect(HFlowState.allowedEdited.hasAccentDot, isTrue);
      expect(HFlowState.allowed.hasAccentDot, isFalse);
    });

    test('every state names an ARB key and a glyph', () {
      final Set<String> keys = HFlowState.values
          .map((HFlowState s) => s.l10nKey)
          .toSet();
      expect(keys, hasLength(HFlowState.values.length));
      const Map<HFlowState, String> expected = <HFlowState, String>{
        HFlowState.held: 'stateHeld',
        HFlowState.allowed: 'stateAllowed',
        HFlowState.allowedEdited: 'stateAllowedEdited',
        HFlowState.blocked: 'stateBlocked',
        HFlowState.timedOut: 'stateTimedOut',
        HFlowState.autoRule: 'stateAutoRule',
        HFlowState.passthroughLlm: 'statePassthroughLlm',
        HFlowState.error: 'stateError',
      };
      for (final HFlowState state in HFlowState.values) {
        expect(state.l10nKey, expected[state]);
        expect(HGlyph.values, contains(state.glyph));
      }
    });
  });

  group('token tables', () {
    test('BACKLOG hexes are literal', () {
      expect(HColors.bg0.toARGB32(), 0xFF0F1115);
      expect(HColors.bg1.toARGB32(), 0xFF151821);
      expect(HColors.bg2.toARGB32(), 0xFF1B1F2A);
      expect(HColors.bg3.toARGB32(), 0xFF232838);
      expect(HColors.line.toARGB32(), 0xFF2A3040);
      expect(HColors.lineStrong.toARGB32(), 0xFF384056);
      expect(HColors.fg0.toARGB32(), 0xFFE6E8EE);
      expect(HColors.fg1.toARGB32(), 0xFFA3A9B8);
      expect(HColors.fg2.toARGB32(), 0xFF6B7186);
      expect(HColors.accent.toARGB32(), 0xFF7C9CF5);
      expect(HColors.held.toARGB32(), 0xFFE0B24A);
      expect(HColors.allowed.toARGB32(), 0xFF4FBF8C);
      expect(HColors.blocked.toARGB32(), 0xFFE5646E);
      expect(HColors.timedOut.toARGB32(), 0xFF8A90A2);
      expect(HColors.passthrough.toARGB32(), 0xFFB48AF0);
      expect(HColors.secret.toARGB32(), 0xFFF0784F);
      expect(HColors.lBg0.toARGB32(), 0xFFFAFBFD);
      expect(HColors.lBg1.toARGB32(), 0xFFFFFFFF);
      expect(HColors.lBg2.toARGB32(), 0xFFF3F5F9);
      expect(HColors.lBg3.toARGB32(), 0xFFE9ECF3);
      expect(HColors.lLine.toARGB32(), 0xFFE1E5EE);
      expect(HColors.lLineStrong.toARGB32(), 0xFFC9CFDC);
      expect(HColors.lFg0.toARGB32(), 0xFF16181F);
      expect(HColors.lFg1.toARGB32(), 0xFF4B5162);
      expect(HColors.lFg2.toARGB32(), 0xFF7C8294);
      expect(HColors.lAccent.toARGB32(), 0xFF5B7FE6);
      expect(HColors.tintAlpha, 0.10);
    });

    test('tokens differ between light and dark', () {
      expect(HTokens.dark.brightness, Brightness.dark);
      expect(HTokens.light.brightness, Brightness.light);
      final HSurfaceColors dark = HTokens.dark.colors;
      final HSurfaceColors light = HTokens.light.colors;
      expect(dark.bg0, isNot(light.bg0));
      expect(dark.bg1, isNot(light.bg1));
      expect(dark.bg2, isNot(light.bg2));
      expect(dark.bg3, isNot(light.bg3));
      expect(dark.line, isNot(light.line));
      expect(dark.lineStrong, isNot(light.lineStrong));
      expect(dark.fg0, isNot(light.fg0));
      expect(dark.fg1, isNot(light.fg1));
      expect(dark.fg2, isNot(light.fg2));
      expect(dark.accent, isNot(light.accent));
      // The dark ladder rises, the light ladder falls.
      expect(
        dark.bg0.computeLuminance(),
        lessThan(dark.bg3.computeLuminance()),
      );
      expect(
        light.bg0.computeLuminance(),
        greaterThan(light.bg3.computeLuminance()),
      );
      // Scales are theme independent.
      expect(HTokens.dark.spacing.unit, HTokens.light.spacing.unit);
      expect(HTokens.dark.typography.ui13, HTokens.light.typography.ui13);
      expect(HTokens.dark.motion.enter, HTokens.light.motion.enter);
    });

    test('primary text clears 4.5:1 on the panel surface', () {
      expect(
        HColorDerivation.contrast(HColors.fg0, HColors.bg1),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        HColorDerivation.contrast(HColors.lFg0, HColors.lBg1),
        greaterThanOrEqualTo(4.5),
      );
    });

    test('method_badge_colors_are_not_state_colors', () {
      // The constants are what the badge paints in the dark theme, so the
      // assertion runs over the code path the widget uses, in both themes.
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        for (final String method in <String>[
          'GET',
          'HEAD',
          'POST',
          'PUT',
          'PATCH',
          'DELETE',
          'PROPFIND',
        ]) {
          final Color color = HMethodBadge.colorFor(method, tokens);
          final String name = '${tokens.brightness.name} $method';
          expect(color, isNot(tokens.state.blocked), reason: name);
          expect(color, isNot(tokens.state.error), reason: name);
          expect(color, isNot(HColors.blocked), reason: name);
          expect(color, isNot(HColors.secret), reason: name);
        }
        // DELETE borrows the blocked hue, but never at full strength: the
        // same hue and saturation at seventy percent alpha. In the light theme
        // its lightness is lower than the state colour's, because a
        // translucent colour needs a darker base than an opaque one to reach
        // 3:1 over the light surfaces.
        final Color delete = HMethodBadge.colorFor('DELETE', tokens);
        final HSLColor deleteHsl = HSLColor.fromColor(delete);
        final HSLColor blockedHsl = HSLColor.fromColor(tokens.state.blocked);
        expect(delete.a, closeTo(0.7, 0.01), reason: tokens.brightness.name);
        expect(deleteHsl.hue, closeTo(blockedHsl.hue, 1.0));
        expect(deleteHsl.saturation, closeTo(blockedHsl.saturation, 0.02));
        expect(deleteHsl.lightness, lessThanOrEqualTo(blockedHsl.lightness));
      }
    });

    test('the light DELETE hue is derived, not faded', () {
      // Fading the light blocked colour to seventy percent composites to
      // 2.8:1 on the raised light surfaces; that is why the table does not do
      // it. The derivation clamps the translucent colour against every light
      // surface instead.
      final Color faded = HColorDerivation.fade(
        HStateColors.light.blocked,
        0.7,
      );
      expect(worstOf(faded, lightSurfaces), lessThan(3.0));
      expect(
        HTokens.light.method.delete,
        HColorDerivation.lightState(HColors.methodDelete),
      );
      expect(
        HTokens.light.method.delete.a,
        closeTo(HColors.methodDelete.a, 1e-6),
      );
      expect(HTokens.dark.method.all, <Color>[
        HColors.methodGet,
        HColors.methodPost,
        HColors.methodPutPatch,
        HColors.methodDelete,
        HColors.fg2,
      ]);
    });

    test('light method and state colours clear 3:1 on every light surface', () {
      const List<String> methodNames = <String>[
        'GET',
        'POST',
        'PUT/PATCH',
        'DELETE',
        'unknown',
      ];
      const List<String> surfaceNames = <String>[
        'lBg0',
        'lBg1',
        'lBg2',
        'lBg3',
      ];
      final List<Color> methods = HTokens.light.method.all;
      expect(methods, hasLength(methodNames.length));
      expect(lightSurfaces, hasLength(surfaceNames.length));
      final Map<String, Color> colours = <String, Color>{
        for (int i = 0; i < methods.length; i++)
          'method ${methodNames[i]}': methods[i],
        for (final HFlowState state in HFlowState.values)
          'state ${state.name}': HTokens.light.stateColor(state),
      };
      expect(colours, hasLength(methodNames.length + HFlowState.values.length));
      colours.forEach((String name, Color color) {
        for (int i = 0; i < lightSurfaces.length; i++) {
          final double ratio = wcagContrast(color, lightSurfaces[i]);
          expect(
            ratio,
            greaterThanOrEqualTo(3.0),
            reason:
                '$name on ${surfaceNames[i]} is '
                '${ratio.toStringAsFixed(2)}',
          );
        }
      });
    });

    test('the dark method table is the HColors constants', () {
      expect(HMethodBadge.colorFor('GET', HTokens.dark), HColors.methodGet);
      expect(HMethodBadge.colorFor('head', HTokens.dark), HColors.methodGet);
      expect(HMethodBadge.colorFor('POST', HTokens.dark), HColors.methodPost);
      expect(
        HMethodBadge.colorFor('PUT', HTokens.dark),
        HColors.methodPutPatch,
      );
      expect(
        HMethodBadge.colorFor('PATCH', HTokens.dark),
        HColors.methodPutPatch,
      );
      expect(
        HMethodBadge.colorFor('DELETE', HTokens.dark),
        HColors.methodDelete,
      );
      expect(HMethodBadge.colorFor('PROPFIND', HTokens.dark), HColors.fg2);
      expect(HTokens.dark.method, same(HMethodColors.dark));
    });

    test('method hues follow the table in both themes', () {
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        expect(HMethodBadge.colorFor('get', tokens), tokens.colors.accent);
        expect(HMethodBadge.colorFor('HEAD', tokens), tokens.colors.accent);
        expect(
          HMethodBadge.colorFor('POST', tokens),
          tokens.state.passthroughLlm,
        );
        expect(HMethodBadge.colorFor('PUT', tokens), tokens.state.held);
        expect(HMethodBadge.colorFor('PATCH', tokens), tokens.state.held);
        expect(
          HMethodBadge.colorFor('DELETE', tokens),
          isNot(tokens.state.blocked),
        );
        expect(HMethodBadge.colorFor('PROPFIND', tokens), tokens.colors.fg2);
      }
      // The light table is theme aware, not the dark one re-used.
      expect(HTokens.light.method.get, isNot(HTokens.dark.method.get));
      expect(
        HTokens.light.method.putPatch,
        isNot(HTokens.dark.method.putPatch),
      );
    });

    test('tint never exceeds ten percent', () {
      for (final HFlowState state in HFlowState.values) {
        final Color tint = HTokens.dark.tint(HTokens.dark.stateColor(state));
        expect(tint.a, lessThanOrEqualTo(HColors.tintAlpha + 1e-9));
      }
      expect(
        HColorDerivation.tint(HColors.accent, 0.9).a,
        closeTo(HColors.tintAlpha, 1e-6),
      );
    });
  });

  group('typography', () {
    test('mono_disables_ligatures', () {
      const List<TextStyle> mono = <TextStyle>[
        HType.mono11,
        HType.mono12,
        HType.mono13,
        HType.mono14,
      ];
      for (final TextStyle style in mono) {
        expect(style.fontFamily, HType.monoFamily);
        expect(style.fontFamilyFallback, HType.monoFallback);
        expect(style.fontFeatures, contains(const FontFeature.disable('liga')));
        expect(
          style.fontFeatures,
          contains(const FontFeature.tabularFigures()),
        );
      }
    });

    test('ui scale is 11/16 12/16 13/20 14/22 16/24 20/28', () {
      const List<(String, TextStyle, double, double)> scale =
          <(String, TextStyle, double, double)>[
            ('ui11', HType.ui11, 11, 16),
            ('ui12', HType.ui12, 12, 16),
            ('ui13', HType.ui13, 13, 20),
            ('ui14', HType.ui14, 14, 22),
            ('ui16', HType.ui16, 16, 24),
            ('ui20', HType.ui20, 20, 28),
            ('mono11', HType.mono11, 11, 16),
            ('mono12', HType.mono12, 12, 16),
            ('mono13', HType.mono13, 13, 20),
            ('mono14', HType.mono14, 14, 22),
          ];
      for (final (String name, TextStyle style, double size, double line)
          in scale) {
        expect(style.fontSize, size, reason: name);
        expect(
          style.fontSize! * style.height!,
          closeTo(line, 1e-9),
          reason: name,
        );
        expect(style.fontWeight, HType.regular, reason: name);
      }
    });

    test('weights are 400, 500 and 600 only', () {
      expect(HType.ui13.fontWeight, FontWeight.w400);
      expect(HType.ui13.medium.fontWeight, FontWeight.w500);
      expect(HType.ui13.semibold.fontWeight, FontWeight.w600);
      expect(HType.semibold.value, lessThan(FontWeight.w700.value));
    });

    test('the ui family carries tabular figures and cv11', () {
      expect(HType.ui13.fontFamily, HType.uiFamily);
      expect(HType.ui13.fontFamilyFallback, HType.uiFallback);
      expect(HType.ui13.fontFeatures, contains(const FontFeature('cv11')));
      expect(
        HType.ui13.fontFeatures,
        contains(const FontFeature.tabularFigures()),
      );
    });
  });

  group('layout tokens', () {
    test('everything is a multiple of the base unit', () {
      expect(HSpace.unit, 4);
      for (final double step in <double>[
        HSpace.x1,
        HSpace.x2,
        HSpace.x3,
        HSpace.x4,
        HSpace.x5,
        HSpace.x6,
        HSpace.x7,
        HSpace.x8,
        HSize.headerBar,
        HSize.statusBar,
        HSize.row,
        HSize.rowSelected,
        HSize.hitMin,
      ]) {
        expect(step % HSpace.unit, 0, reason: '$step is not on the grid');
      }
      expect(HSpace.panelPadding, 12);
      expect(HRadius.control, 4);
      expect(HRadius.card, 6);
      expect(HRadius.panel, 0);
      expect(HSize.row, 36);
      expect(HSize.rowSelected, 56);
      expect(HSize.headerBar, 40);
      expect(HSize.statusBar, 24);
      expect(HSize.paneRatio, (28, 44, 28));
      expect(HSize.paneRatio.$1 + HSize.paneRatio.$2 + HSize.paneRatio.$3, 100);
    });

    test('motion curves are the ones BACKLOG names', () {
      expect(HMotion.enter, const Cubic(0.2, 0, 0, 1));
      expect(HMotion.exit, const Cubic(0.4, 0, 1, 1));
      expect(HMotion.enter, isNot(Curves.easeOut));
      expect(HMotion.arrive.inMilliseconds, 180);
      expect(HMotion.press.inMilliseconds, 120);
      expect(HMotion.sweep.inMilliseconds, 200);
      expect(HMotion.leave.inMilliseconds, 220);
      expect(HMotion.ruleDraw.inMilliseconds, 240);
      expect(HMotion.breathe.inMilliseconds, 1200);
      expect(HMotion.holdToConfirm.inMilliseconds, 400);
    });

    test('theme mode resolves the platform brightness', () {
      expect(HThemeMode.dark.resolve(Brightness.light), HTokens.dark);
      expect(HThemeMode.light.resolve(Brightness.dark), HTokens.light);
      expect(HThemeMode.system.resolve(Brightness.dark), HTokens.dark);
      expect(HThemeMode.system.resolve(Brightness.light), HTokens.light);
      expect(HThemeMode.system.configValue, 'system');
    });
  });

  group('hit targets', () {
    testWidgets('hit_targets_min_28', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              HButton(
                size: HButtonSize.sm,
                onPressed: () {},
                child: const Text('sm'),
              ),
              HButton(onPressed: () {}, child: const Text('md')),
              const HBadge(text: 'tls'),
              const HMethodBadge(method: 'GET'),
              HIconButton(
                glyph: HGlyph.close,
                onPressed: () {},
                semanticsLabel: 'close',
              ),
              HPill(left: const Text('Allow'), onLeft: () {}),
            ],
          ),
        ),
      );
      for (final Type type in <Type>[
        HButton,
        HBadge,
        HMethodBadge,
        HIconButton,
        HPill,
      ]) {
        final Iterable<Element> elements = find.byType(type).evaluate();
        expect(elements, isNotEmpty, reason: '$type was not built');
        for (final Element element in elements) {
          final RenderBox box = element.renderObject! as RenderBox;
          expect(
            box.size.height,
            greaterThanOrEqualTo(HSize.hitMin),
            reason: '$type is ${box.size.height} tall',
          );
        }
      }
    });
  });

  group('gallery', () {
    for (final (HThemeMode mode, Color background) in <(HThemeMode, Color)>[
      (HThemeMode.dark, HColors.bg0),
      (HThemeMode.light, HColors.lBg0),
    ]) {
      testWidgets('builds in ${mode.name}', (WidgetTester tester) async {
        await tester.pumpWidget(HGalleryPage(initialMode: mode));
        await tester.pump();
        expect(tester.takeException(), isNull);
        expect(find.byType(HGalleryPage), findsOneWidget);
        expect(find.byType(HStateGlyph), findsWidgets);
        // The root fill is the application background of the mode, so the
        // two modes render differently and not merely without throwing.
        expect(galleryBackground(tester), background);
      });
    }

    testWidgets('every button variant appears in every interaction state', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(const HGalleryPage());
      await tester.pump();
      for (final HButtonVariant variant in HButtonVariant.values) {
        for (final HButtonSize size in HButtonSize.values) {
          for (final HButtonPreview? preview in <HButtonPreview?>[
            null,
            ...HButtonPreview.values,
          ]) {
            expect(
              find.byWidgetPredicate(
                (Widget widget) =>
                    widget is HButton &&
                    widget.variant == variant &&
                    widget.size == size &&
                    widget.preview == preview &&
                    widget.enabled,
              ),
              findsWidgets,
              reason: '${size.name} ${variant.name} ${preview?.name}',
            );
          }
          expect(
            find.byWidgetPredicate(
              (Widget widget) =>
                  widget is HButton &&
                  widget.variant == variant &&
                  widget.size == size &&
                  !widget.enabled,
            ),
            findsWidgets,
            reason: '${size.name} ${variant.name} disabled',
          );
        }
      }
    });

    testWidgets('the theme toggle swaps the tokens', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(const HGalleryPage());
      await tester.pump();
      expect(find.text('brightness dark · taps 0'), findsOneWidget);
      expect(galleryBackground(tester), HColors.bg0);
      await tester.tap(find.widgetWithText(HButton, 'Light'));
      await tester.pump();
      expect(find.text('brightness light · taps 0'), findsOneWidget);
      expect(galleryBackground(tester), HColors.lBg0);
      expect(tester.takeException(), isNull);
    });

    test('hex formatting drops a full alpha channel', () {
      expect(HColorDerivation.toHex(HColors.bg0), '#0F1115');
      expect(HColorDerivation.toHex(HColors.autoRule), '#994FBF8C');
    });
  });
}
