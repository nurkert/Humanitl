// Kontrast der Aktionsleiste in Zahlen (docs/UX.md 6): Text erreicht 4,5:1
// auf der Fläche, auf der er wirklich steht — auch auf einem Tint und auf
// einer Halte-Füllung —, Flächen und Ringe erreichen 3:1.
//
// Der Test im Design-System prüft nur Farbe auf Fläche; diese Kombinationen
// entstehen erst hier, in den Controls dieses Screens.

import 'dart:math' as math;

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/hold_to_confirm.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/widgets/release_valve.dart';

/// Der Kontrast von [foreground] über [layers] auf [base].
double contrastOn(Color foreground, List<Color> layers, Color base) {
  Color surface = base;
  for (final Color layer in layers) {
    surface = HColorDerivation.flatten(layer, surface);
  }
  return HColorDerivation.contrast(
    HColorDerivation.flatten(foreground, surface),
    surface,
  );
}

/// Der Abstand von [a] und [b] in OKLab (Björn Ottosson, 2020), hier
/// ausgeschrieben und nicht aus `HColorDerivation` geliehen: ein Fehler in
/// der Ableitung verschöbe sonst beide Seiten des Vergleichs zugleich.
double oklabDistance(Color a, Color b) {
  List<double> lab(Color c) {
    double lin(double v) => v <= 0.04045
        ? v / 12.92
        : math.pow((v + 0.055) / 1.055, 2.4).toDouble();
    double cbrt(double x) => math.pow(x, 1 / 3).toDouble();
    final double r = lin(c.r);
    final double g = lin(c.g);
    final double bl = lin(c.b);
    final double l = cbrt(
      0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * bl,
    );
    final double m = cbrt(
      0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * bl,
    );
    final double s = cbrt(
      0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * bl,
    );
    return <double>[
      0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
      1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
      0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    ];
  }

  final List<double> pa = lab(a);
  final List<double> pb = lab(b);
  double sum = 0;
  for (int i = 0; i < 3; i++) {
    sum += (pa[i] - pb[i]) * (pa[i] - pb[i]);
  }
  return math.sqrt(sum);
}

void main() {
  for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
    final String theme = tokens.brightness.name;
    final Color bar = tokens.colors.bg1;
    final Color valveFill = tokens.tint(tokens.colors.accent);

    test('$theme: the valve label stays readable, resting and held', () {
      expect(
        contrastOn(tokens.colors.fg0, <Color>[valveFill], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg0, <Color>[
          valveFill,
          holdFill(tokens.state.allowed),
        ], bar),
        greaterThanOrEqualTo(4.5),
      );
      // Auch die Taste, die neben dem Label steht.
      expect(
        contrastOn(tokens.colors.fg1, <Color>[valveFill], bar),
        greaterThanOrEqualTo(4.5),
      );
      // Und im gedrückten wie im schwebenden Zustand.
      for (final double alpha in <double>[
        valveHoverAlpha(tokens.brightness),
        valvePressAlpha(tokens.brightness),
      ]) {
        expect(
          contrastOn(tokens.colors.fg0, <Color>[
            valveFill,
            tokens.colors.accent.withValues(alpha: alpha),
          ], bar),
          greaterThanOrEqualTo(4.5),
          reason: 'alpha $alpha',
        );
      }
    });

    test('$theme: the valve steps visibly on hover and on press', () {
      // HUM-272: im hellen Theme lagen Ruhe, Hover und Druck des Ventils
      // 0,022 auseinander, kaum über der Sichtbarkeitsschwelle.
      Color layered(double? alpha) => HColorDerivation.flatten(
        alpha == null
            ? const Color(0x00000000)
            : tokens.colors.accent.withValues(alpha: alpha),
        HColorDerivation.flatten(valveFill, bar),
      );
      final Color rest = layered(null);
      final Color hover = layered(valveHoverAlpha(tokens.brightness));
      final Color pressed = layered(valvePressAlpha(tokens.brightness));
      expect(
        oklabDistance(rest, hover),
        greaterThanOrEqualTo(HColorDerivation.interactionStepMin),
      );
      expect(
        oklabDistance(hover, pressed),
        greaterThanOrEqualTo(HColorDerivation.interactionStepMin),
      );
    });

    test('$theme: the block label stays readable, resting and held', () {
      expect(
        contrastOn(tokens.state.blocked, const <Color>[], bar),
        greaterThanOrEqualTo(4.5),
      );
      // Unter der Füllung wechselt das Label auf die neutrale Textfarbe: der
      // Blockier-Ton erreicht auf seiner eigenen Füllung keine 4,5:1.
      expect(
        contrastOn(tokens.colors.fg0, <Color>[
          holdFill(tokens.state.blocked),
        ], bar),
        greaterThanOrEqualTo(4.5),
      );
      // Das Glyph ist Fläche, keine Schrift: 3:1 genügt.
      expect(
        contrastOn(tokens.state.blocked, <Color>[
          holdFill(tokens.state.blocked),
        ], bar),
        greaterThanOrEqualTo(3),
      );
    });

    test('$theme: a chosen segment and the focus ring reach their floor', () {
      expect(
        contrastOn(tokens.colors.fg0, <Color>[valveFill], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg1, const <Color>[], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.accent, const <Color>[], bar),
        greaterThanOrEqualTo(3),
      );
    });

    test('$theme: the warning valve of a finding stays readable', () {
      // Eine Anfrage mit offenem Fund färbt das Ventil in der Warnfarbe
      // (docs/UX.md 4.7, seit HUM-260 `warning` statt `held`); Label und Taste
      // stehen dann auf deren Tint.
      final Color warned = tokens.tint(tokens.colors.warning);
      expect(
        contrastOn(tokens.colors.fg0, <Color>[warned], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg1, <Color>[warned], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg0, <Color>[
          warned,
          holdFill(tokens.state.allowed),
        ], bar),
        greaterThanOrEqualTo(4.5),
      );
    });

    test('$theme: the two lines under the controls are readable', () {
      for (final Color color in <Color>[tokens.colors.fg0, tokens.colors.fg1]) {
        expect(
          contrastOn(color, const <Color>[], bar),
          greaterThanOrEqualTo(4.5),
        );
      }
    });
  }
}
