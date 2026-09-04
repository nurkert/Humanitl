// Kontrast der Aktionsleiste in Zahlen (docs/UX.md 6): Text erreicht 4,5:1
// auf der Fläche, auf der er wirklich steht — auch auf einem Tint und auf
// einer Halte-Füllung —, Flächen und Ringe erreichen 3:1.
//
// Der Test im Design-System prüft nur Farbe auf Fläche; diese Kombinationen
// entstehen erst hier, in den Controls dieses Screens.

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
      for (final double alpha in <double>[valveHoverAlpha, valvePressAlpha]) {
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

    test('$theme: the amber valve of a finding stays readable', () {
      // Eine Anfrage mit offenem Fund färbt das Ventil amber (docs/UX.md 4.7);
      // Label und Taste stehen dann auf dem Amber-Tint statt auf dem Akzent.
      final Color amber = tokens.tint(tokens.state.held);
      expect(
        contrastOn(tokens.colors.fg0, <Color>[amber], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg1, <Color>[amber], bar),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg0, <Color>[
          amber,
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
