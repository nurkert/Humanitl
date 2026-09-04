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

/// Die Textpalette, wie sie sich *erwartungsgemäß* ableitet. Eingefroren wie
/// [frozenLightStates]: die Ableitung ist der einzige Erzeuger dieser Werte,
/// also könnte ein Test, der sie nachrechnet, nicht fehlschlagen. Eine
/// geänderte Ableitung erscheint hier als Diff, den jemand ansieht.
const Map<HFlowState, int> frozenDarkStateText = <HFlowState, int>{
  HFlowState.held: 0xFFE0B24A,
  HFlowState.allowed: 0xFF56C291,
  HFlowState.allowedEdited: 0xFF62BEA5,
  HFlowState.blocked: 0xFFEB878F,
  HFlowState.timedOut: 0xFFA4A8B6,
  HFlowState.autoRule: 0x99ECF8F3,
  HFlowState.passthroughLlm: 0xFFC09CF2,
  HFlowState.error: 0xFFF28E6B,
};

/// Dasselbe für das helle Theme.
const Map<HFlowState, int> frozenLightStateText = <HFlowState, int>{
  HFlowState.held: 0xFF735613,
  HFlowState.allowed: 0xFF26684A,
  HFlowState.allowedEdited: 0xFF2B6656,
  HFlowState.blocked: 0xFFA81C27,
  HFlowState.timedOut: 0xFF565B6B,
  HFlowState.autoRule: 0x99020705,
  HFlowState.passthroughLlm: 0xFF7122E3,
  HFlowState.error: 0xFFA2340E,
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

  group('contrast', () {
    // Jede Fläche, auf der eine Zustands- oder Methodenfarbe als Text stehen
    // kann: die vier Flächen der Leiter, dieselben vier mit der Zehn-Prozent-
    // Tönung derselben Farbe, und dieselben vier mit der Hover-, der Druck-
    // und der Haltefüllung eines Controls (`docs/UX.md` 6). Die Liste steht
    // hier ausgeschrieben und nicht als Verweis auf
    // `HColorDerivation.fillAlphas`, damit sie nicht mit der Palette
    // mitwandert; ein eigener Test hält die beiden Listen gleich.
    const List<double> alphas = <double>[
      0,
      HColors.tintAlpha,
      HColors.fillHoverAlpha,
      HColors.fillPressedAlpha,
      HColors.fillHoldAlpha,
    ];
    List<Color> backgrounds(Color area, List<Color> surfaces) {
      final List<Color> out = <Color>[];
      for (final Color surface in surfaces) {
        for (final double alpha in alphas) {
          out.add(
            alpha == 0
                ? surface
                : Color.alphaBlend(area.withValues(alpha: alpha), surface),
          );
        }
      }
      return out;
    }

    double worstText(Color text, Color area, List<Color> surfaces) =>
        worstOf(text, backgrounds(area, surfaces));

    test('the area palette is exactly what it claims: 3:1, not 4.5:1', () {
      // Der Grund, aus dem es die Textpalette gibt. Ohne diese Zeile liest
      // sich der Rest der Gruppe wie eine Verdopplung.
      final Map<HFlowState, double> lightArea = <HFlowState, double>{
        for (final HFlowState state in HFlowState.values)
          state: worstText(
            HStateColors.light.resolve(state),
            HStateColors.light.resolve(state),
            lightSurfaces,
          ),
      };
      for (final MapEntry<HFlowState, double> entry in lightArea.entries) {
        expect(
          entry.value,
          greaterThanOrEqualTo(2.4),
          reason: '${entry.key.name} is ${entry.value.toStringAsFixed(2)}',
        );
        expect(
          entry.value,
          lessThan(4.5),
          reason:
              'if ${entry.key.name} reached 4.5:1 as an area, the text '
              'palette would be pointless',
        );
      }
      // Die gemessenen Zahlen, damit ein Rückschritt hier auffällt und nicht
      // erst auf dem Schirm. Der schlechteste Fall ist seit der Aufnahme der
      // Haltefüllung in `fillAlphas` diese Füllung: `held` misst auf ihr
      // 2,49:1 statt der 2,54:1 auf der Tönung (`docs/UX.md` 6).
      expect(lightArea[HFlowState.held]!, closeTo(2.49, 0.01));
      expect(lightArea[HFlowState.blocked]!, closeTo(2.95, 0.01));
      expect(lightArea[HFlowState.error]!, closeTo(2.49, 0.01));
    });

    test('every fill of the system is a fill the derivation saw', () {
      // Die vierte Füllung, die niemand misst, ist der Fehler, den dieser
      // Test verhindert: `HoldToConfirm` malte 20 % Zustandsfarbe, und die
      // Textableitung wusste nichts davon (`docs/UX.md` 6).
      expect(HColorDerivation.fillAlphas, alphas);
      expect(HColors.fillHoldAlpha, 0.20);
      // Die Haltefüllung ist die dunkelste Fläche, also der strenge Fall.
      expect(HColors.fillHoldAlpha, greaterThan(HColors.fillPressedAlpha));
      expect(HColors.fillPressedAlpha, greaterThan(HColors.fillHoverAlpha));
      expect(HColors.fillHoverAlpha, greaterThan(HColors.tintAlpha));
      // Und sie trägt Text: ohne die Aufnahme in `fillAlphas` misst die
      // Textvariante von `error` dunkel 4,44:1 auf ihr.
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        for (final HFlowState state in HFlowState.values) {
          final Color area = tokens.stateColor(state);
          final Color text = tokens.stateTextColor(state);
          for (final Color surface in tokens.colors.ladder) {
            final Color hold = Color.alphaBlend(
              area.withValues(alpha: HColors.fillHoldAlpha),
              surface,
            );
            final double ratio = wcagContrast(text, hold);
            expect(
              ratio,
              greaterThanOrEqualTo(HColorDerivation.textMinContrast),
              reason:
                  '${brightness.name} ${state.name} on its hold fill is '
                  '${ratio.toStringAsFixed(2)}',
            );
          }
        }
      }
    });

    test('the accent carries text and fills at 4.5:1 in both themes', () {
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        final HSurfaceColors c = tokens.colors;
        // Die Fläche bleibt eine Fläche: 3:1, nicht mehr.
        expect(
          worstOf(c.accent, tokens.colors.ladder),
          greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
        );
        // Das Wort darauf erreicht 4,5:1, auch auf der eigenen Tönung und auf
        // jeder Füllung (`docs/UX.md` 6).
        final double text = worstText(c.accentText, c.accent, c.ladder);
        expect(
          text,
          greaterThanOrEqualTo(HColorDerivation.textMinContrast),
          reason: '${brightness.name} accentText is ${text.toStringAsFixed(2)}',
        );
        // Und der Akzent selbst trägt keines: das ist der Grund, aus dem es
        // die Variante gibt.
        if (brightness == Brightness.light) {
          expect(
            worstText(c.accent, c.accent, c.ladder),
            lessThan(HColorDerivation.textMinContrast),
          );
        }
        // Die gefüllte Variante: das Wort auf ihr erreicht 4,5:1.
        expect(
          wcagContrast(c.onAccent, c.accentFill),
          greaterThanOrEqualTo(HColorDerivation.textMinContrast),
        );
        // Und sie bleibt eine Fläche, die man auf jedem Untergrund sieht.
        expect(
          worstOf(c.accentFill, c.ladder),
          greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
        );
      }
      // Hell weicht die Füllung zurück, dunkel steht sie schon weit genug.
      expect(HTokens.dark.colors.accentFill, HColors.accent);
      expect(HTokens.light.colors.accentFill, isNot(HColors.lAccent));
      expect(
        wcagContrast(HColors.lBg1, HColors.lAccent),
        lessThan(HColorDerivation.textMinContrast),
      );
    });

    test('stateTextOf resolves an area colour of either theme', () {
      final HTokens light = HTokens.light;
      final HTokens dark = HTokens.dark;
      for (final HFlowState state in HFlowState.values) {
        // Die eigene Palette.
        expect(
          light.stateTextOf(light.stateColor(state)),
          light.stateTextColor(state),
          reason: state.name,
        );
        // Und die des anderen Themes: wer `HColors.held` schreibt statt
        // `tokens.state.held`, bekam vorher seine Farbe unverändert zurück
        // und malte bei rund 2,5:1.
        expect(
          light.stateTextOf(dark.stateColor(state)),
          light.stateTextColor(state),
          reason: state.name,
        );
        expect(
          dark.stateTextOf(light.stateColor(state)),
          dark.stateTextColor(state),
          reason: state.name,
        );
      }
      // Der Akzent ist eine Fläche wie die Zustandsfarben.
      expect(light.stateTextOf(HColors.lAccent), light.colors.accentText);
      expect(light.stateTextOf(HColors.accent), light.colors.accentText);
      expect(dark.stateTextOf(HColors.accent), dark.colors.accentText);
      // Die Textleiter kommt unverändert zurück.
      expect(light.stateTextOf(HColors.lFg1), HColors.lFg1);
      expect(dark.stateTextOf(HColors.fg0), HColors.fg0);
    });

    test('a derivation that cannot reach its floor fails loudly', () {
      // Beide Schleifen enden am Rand der Leiter. Ohne Zusicherung käme von
      // dort eine Farbe zurück, die die Grenze verfehlt, und niemand merkte
      // es (`docs/UX.md` 9).
      expect(
        () => HColorDerivation.textVariant(
          const Color(0xFFFFFFFF),
          surfaces: const <Color>[Color(0xFFFFFFFF)],
          minContrast: 21,
        ),
        throwsAssertionError,
      );
      expect(
        () => HColorDerivation.readableFill(
          const Color(0xFFFFFFFF),
          const Color(0xFFFFFFFF),
          minContrast: 21,
        ),
        throwsAssertionError,
      );
      // Der erreichbare Fall kommt durch und erreicht die Grenze wirklich.
      final Color fill = HColorDerivation.readableFill(
        HColors.lAccent,
        HColors.lBg1,
      );
      expect(
        wcagContrast(HColors.lBg1, fill),
        greaterThanOrEqualTo(HColorDerivation.textMinContrast),
      );
      // Ton und Sättigung bleiben; nur die Helligkeit weicht.
      final HSLColor before = HSLColor.fromColor(HColors.lAccent);
      final HSLColor after = HSLColor.fromColor(fill);
      expect(after.hue, closeTo(before.hue, 2.0));
      expect(after.lightness, lessThan(before.lightness));
    });

    test('every state colour carries text at 4.5:1 in both themes', () {
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        final List<Color> surfaces = tokens.colors.ladder;
        for (final HFlowState state in HFlowState.values) {
          final Color area = tokens.stateColor(state);
          final Color text = tokens.stateTextColor(state);
          final double ratio = worstText(text, area, surfaces);
          expect(
            ratio,
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason:
                '${brightness.name} ${state.name} text is '
                '${ratio.toStringAsFixed(2)}:1 on '
                '${HColorDerivation.toHex(area)}',
          );
        }
      }
    });

    test('every method hue carries its verb at 4.5:1 in both themes', () {
      const List<String> methods = <String>[
        'GET',
        'HEAD',
        'POST',
        'PUT',
        'PATCH',
        'DELETE',
        'PROPFIND',
      ];
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        final List<Color> surfaces = tokens.colors.ladder;
        for (final String method in methods) {
          final Color area = tokens.method.of(method);
          final Color text = tokens.methodTextColor(method);
          final double ratio = worstText(text, area, surfaces);
          expect(
            ratio,
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason:
                '${brightness.name} $method is '
                '${ratio.toStringAsFixed(2)}:1',
          );
        }
      }
    });

    test('the area palette is untouched by the text palette', () {
      // Flächen bleiben, wo sie sind: die Rail, der Bogen und die Tönung
      // hätten von 4,5:1 nichts, und `held` bei voller Sättigung wäre eine
      // andere Farbe.
      for (final HFlowState state in HFlowState.values) {
        expect(
          HStateColors.dark.resolve(state).toARGB32(),
          state == HFlowState.autoRule
              ? HColors.autoRule.toARGB32()
              : isNotNull,
          reason: state.name,
        );
        expect(
          HStateColors.light.resolve(state).toARGB32(),
          frozenLightStates[state],
          reason: state.name,
        );
      }
    });

    test('the text palettes match the frozen tables', () {
      for (final HFlowState state in HFlowState.values) {
        expect(
          HStateColors.darkText.resolve(state).toARGB32(),
          frozenDarkStateText[state],
          reason:
              '${state.name} derived to '
              '${HColorDerivation.toHex(HStateColors.darkText.resolve(state))}',
        );
        expect(
          HStateColors.lightText.resolve(state).toARGB32(),
          frozenLightStateText[state],
          reason:
              '${state.name} derived to '
              '${HColorDerivation.toHex(HStateColors.lightText.resolve(state))}',
        );
      }
    });

    test('the text variant keeps hue, saturation and alpha', () {
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        for (final HFlowState state in HFlowState.values) {
          final Color area = tokens.stateColor(state);
          final Color text = tokens.stateTextColor(state);
          expect(text.a, closeTo(area.a, 1e-6), reason: state.name);
          final HSLColor areaHsl = HSLColor.fromColor(area);
          final HSLColor textHsl = HSLColor.fromColor(text);
          // Der Ton bleibt, solange die Farbe ihn tragen kann. Nahe an
          // Schwarz oder Weiß tut sie das nicht mehr: `autoRule` hat 60 %
          // Deckkraft, und die einzige Helligkeit, mit der es hell 4,5:1
          // erreicht, liegt so dicht an Schwarz, dass das Runden auf acht Bit
          // je Kanal den Ton verschiebt. Das ist der Preis dafür, dass die
          // Textvariante das Alpha behält, statt eine zweite Farbe zu
          // erfinden.
          if (areaHsl.saturation > 0.05 &&
              textHsl.lightness > 0.1 &&
              textHsl.lightness < 0.9) {
            expect(textHsl.hue, closeTo(areaHsl.hue, 2.0), reason: state.name);
          }
          // Von den Flächen weg: im hellen Theme dunkler, im dunklen heller.
          if (brightness == Brightness.light) {
            expect(
              textHsl.lightness,
              lessThanOrEqualTo(areaHsl.lightness),
              reason: state.name,
            );
          } else {
            expect(
              textHsl.lightness,
              greaterThanOrEqualTo(areaHsl.lightness),
              reason: state.name,
            );
          }
        }
      }
    });

    test('the countdown arc reaches 3:1 on every surface it lies on', () {
      // Die Entscheidung aus `docs/UX.md` 9, Punkt 6: die verbrauchte Zeit
      // ist eine Lücke, keine Spur. Eine Spur bräuchte eine eigene Farbe, die
      // auf jeder Panelfläche zu sehen ist — die Haarlinie war das nicht, sie
      // misst hell 1,01:1 bis 1,19:1 gegen die vier Flächen — und gegen die
      // der Bogen trotzdem 3:1 erreicht. Der Bogen allein schafft seine 3:1
      // auf jeder Fläche, und die Lücke braucht kein Token.
      for (final Color surface in lightSurfaces) {
        expect(
          wcagContrast(HColors.lLine, surface),
          lessThan(1.27),
          reason:
              'die alte Spur war auf ${HColorDerivation.toHex(surface)} '
              'nicht zu sehen',
        );
      }
      for (final Brightness brightness in Brightness.values) {
        final HTokens tokens = HTokens.forBrightness(brightness);
        for (final HFlowState state in HFlowState.values) {
          final double ratio = worstOf(
            tokens.stateColor(state),
            tokens.colors.ladder,
          );
          expect(
            ratio,
            greaterThanOrEqualTo(HColorDerivation.areaMinContrast),
            reason:
                '${brightness.name} ${state.name} arc is '
                '${ratio.toStringAsFixed(2)}',
          );
        }
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
      expect(HColors.fillHoverAlpha, 0.14);
      expect(HColors.fillPressedAlpha, 0.18);
      expect(HColors.fillHoldAlpha, 0.20);
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
        HSize.rowHistory,
        HSize.rowBody,
        HSize.rowActionSlot,
        HSize.hitMin,
        HSize.hitDecision.width,
        HSize.hitDecision.height,
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
      // Die drei Dichten von `docs/UX.md` 3.2, jede als eigenes Token, damit
      // kein Screen eine 28 in eine Feature-Datei schreibt.
      expect(HSize.row, 36);
      expect(HSize.rowHistory, 28);
      expect(HSize.rowBody, 24);
      expect(HSize.rowActionSlot, 28);
      expect(HSize.rowActionSlot, HSize.hitMin);
      expect(HSize.hitDecision, const Size(120, 32));
      // Das Maß ist eine Zeichenzahl, keine Pixelbreite.
      expect(HSize.measureChars, 90);
      expect(HSize.measureWidth(12), closeTo(90 * 0.6 * 12, 1e-9));
      expect(HTokens.dark.sizes.measureChars, 90);
      expect(HTokens.dark.sizes.rowHistory, HSize.rowHistory);
      expect(HTokens.dark.sizes.rowBody, HSize.rowBody);
      expect(HTokens.dark.sizes.rowActionSlot, HSize.rowActionSlot);
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

    test('motion vocabulary of docs/UX.md', () {
      // Leaving must read as a longer travel than arriving.
      expect(HMotion.arriveOffset, 8);
      expect(HMotion.leaveOffset, 12);
      expect(HMotion.leaveOffset, HMotion.arriveOffset * 1.5);
      // A burst of arrivals is fully in place after five staggered rows.
      expect(HMotion.stagger.inMilliseconds, 30);
      expect(HMotion.staggerMax, 5);
      expect(
        HMotion.stagger * HMotion.staggerMax,
        const Duration(milliseconds: 150),
      );
      // The two phases of an exit overlap; together they cover more than one.
      expect(HMotion.leaveGlideFraction, 0.6);
      expect(HMotion.leaveGlideFraction * 2, greaterThan(1.0));
      // Deliberateness scales with reach, so a single block holds shorter than
      // the release valve.
      expect(HMotion.holdToBlock.inMilliseconds, 250);
      expect(HMotion.holdToBlock, lessThan(HMotion.holdToConfirm));
      // Policy windows.
      expect(HMotion.confirm, const Duration(seconds: 3));
      expect(HMotion.undoWindow, const Duration(seconds: 10));
      expect(HMotion.freezeAfterKey, const Duration(seconds: 2));
      expect(HMotion.freezeAfterPointer, const Duration(milliseconds: 500));
      expect(HMotion.clockTick, const Duration(seconds: 1));
      // Die beiden Fristen, die vorher als Literale in `core/ui` standen
      // (`docs/UX.md` 2.1 und 9).
      expect(HMotion.hoverLabel, const Duration(milliseconds: 350));
      expect(HMotion.copyFeedback, const Duration(seconds: 2));
      // Erlauben ist unumkehrbar und wird deshalb neu armiert, Blockieren
      // nicht; die Frist bleibt unter der Halte-Bestaetigung.
      expect(HMotion.rearm, const Duration(milliseconds: 350));
      expect(HMotion.rearm, lessThan(HMotion.holdToConfirm));
      // Warten: erst zugeben, dann lange genug stehen bleiben.
      expect(HMotion.waitVisible, const Duration(milliseconds: 150));
      expect(HMotion.waitMinVisible, const Duration(milliseconds: 400));
      expect(HMotion.waitVisible, lessThan(HMotion.waitMinVisible));
      // Der Ring bekommt erst unter einer Minute einen eigenen Controller.
      expect(HMotion.ringSmoothBelow, const Duration(seconds: 60));
      expect(HMotion.ringSmoothBelow, greaterThan(HMotion.clockTick));
      // The breath is bounded, brightening, and never hides the glyph.
      expect(HMotion.breatheBelow, 0.2);
      expect(HMotion.breatheBelowUrgent, 0.05);
      expect(HMotion.breatheBelowUrgent, lessThan(HMotion.breatheBelow));
      expect(HMotion.breatheCycles, 3);
      expect(HMotion.breatheMinOpacity, 0.72);
      expect(HMotion.reducedRingAlpha, 0.4);
    });

    test('the splitter has a width and a keyboard step', () {
      // Kein Literal mehr im Splitter, und ein Schritt, den man sieht
      // (`docs/UX.md` 2.1 und 5.1).
      expect(HSize.splitterActive, HSize.hairline * 2);
      expect(HSize.splitterStep, HSpace.x2);
      expect(HSize.splitterStep % HSpace.unit, 0);
    });

    testWidgets('reduced motion drops travel and keeps feedback', (
      WidgetTester tester,
    ) async {
      late BuildContext moving;
      late BuildContext still;
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: Column(
            children: <Widget>[
              MediaQuery(
                data: const MediaQueryData(),
                child: Builder(
                  builder: (BuildContext context) {
                    moving = context;
                    return const SizedBox.shrink();
                  },
                ),
              ),
              MediaQuery(
                data: const MediaQueryData(disableAnimations: true),
                child: Builder(
                  builder: (BuildContext context) {
                    still = context;
                    return const SizedBox.shrink();
                  },
                ),
              ),
            ],
          ),
        ),
      );

      expect(HReducedMotion.of(moving), isFalse);
      expect(HReducedMotion.of(still), isTrue);

      expect(HReducedMotion.distance(moving, HMotion.leaveOffset), 12);
      expect(HReducedMotion.distance(still, HMotion.leaveOffset), 0);
      expect(HReducedMotion.displace(moving, HMotion.leave), HMotion.leave);
      expect(HReducedMotion.displace(still, HMotion.leave), Duration.zero);
      expect(HReducedMotion.cycles(moving, HMotion.breatheCycles), 3);
      expect(HReducedMotion.cycles(still, HMotion.breatheCycles), 0);
    });

    testWidgets('reduced motion without a MediaQuery is off', (
      WidgetTester tester,
    ) async {
      late BuildContext bare;
      await tester.pumpWidget(
        Builder(
          builder: (BuildContext context) {
            bare = context;
            return const SizedBox.shrink();
          },
        ),
      );
      expect(HReducedMotion.of(bare), isFalse);
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
