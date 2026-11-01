import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// Das Tastenkürzel hinter einer Beschriftung, etwa `Enter`, `E` oder `B` in
/// der Zeile der Entscheidungen (HUM-406).
///
/// Das Kürzel steht in `mono11`, die Beschriftung davor in `ui13`. In einer
/// mittig ausgerichteten Zeile standen die beiden Zeilenkästen mittig
/// übereinander, nicht ihre Grundlinien: das Kürzel saß sichtbar höher als das
/// Wort davor, wie hochgestellt. Dieses Widget nimmt die Höhe einer
/// `ui13`-Zeile an und setzt das Kürzel auf deren Grundlinie, auch bei
/// größerer Textskalierung. Die Zeile darum muss dafür nichts wissen; sie
/// bleibt mittig, und Glyphen darin bleiben es auch.
class HKeyHint extends StatelessWidget {
  /// Creates the hint for [text], in `fg1` unless [color] says otherwise.
  const HKeyHint(this.text, {this.color, super.key});

  /// Das Kürzel, wie es auf der Taste steht.
  final String text;

  /// Die Farbe des Kürzels; `fg1`, wenn null.
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextPainter label = TextPainter(
      // Gemessen wird nur die Zeile, nicht die Glyphe: Höhe und Grundlinie
      // einer ui13-Zeile. Eine Ziffer liefert beide und ist kein Wort, das
      // die Sprachprüfung (HUM-052) als sichtbaren Text läse.
      text: TextSpan(text: '0', style: tokens.typography.ui13),
      textDirection: TextDirection.ltr,
      textScaler: MediaQuery.textScalerOf(context),
    )..layout();
    final double baseline = label.computeDistanceToActualBaseline(
      TextBaseline.alphabetic,
    );
    final double height = label.height;
    label.dispose();
    return SizedBox(
      height: height,
      child: Baseline(
        baseline: baseline,
        baselineType: TextBaseline.alphabetic,
        child: Text(
          text,
          maxLines: 1,
          style: tokens.typography.mono11.tinted(color ?? tokens.colors.fg1),
        ),
      ),
    );
  }
}
