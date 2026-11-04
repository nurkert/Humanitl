import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
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

/// Die Tasten eines Griffs als Kappen, etwa `Ctrl` `Shift` `0` in der
/// Übersicht der Tastenkürzel (HUM-420).
///
/// Jede Taste steht in einer eigenen Kappe: `mono11` auf `bg2`, mit einer
/// Haarlinie und dem Radius eines Badges. Mehrere Griffe für dieselbe
/// Handlung trennt [separator] (`/`) in `fg2`. Die Kappen tragen keine
/// Semantik; die Zeile darum nennt die Tasten als Text.
class HKeyCaps extends StatelessWidget {
  /// Creates the caps for [chords], one list of key names per chord.
  const HKeyCaps(this.chords, {this.separator = '/', super.key});

  /// Die Griffe, je Griff die Namen seiner Tasten, Modifikatoren zuerst.
  final List<List<String>> chords;

  /// Das Zeichen zwischen zwei Griffen.
  final String separator;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final double gap = tokens.spacing.x1;
    return ExcludeSemantics(
      child: Wrap(
        alignment: WrapAlignment.end,
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: gap,
        runSpacing: gap,
        children: <Widget>[
          for (int c = 0; c < chords.length; c++) ...<Widget>[
            if (c > 0)
              Text(
                separator,
                style: tokens.typography.mono11.tinted(tokens.colors.fg2),
              ),
            // Die Tasten eines Griffs stehen enger beieinander als zwei
            // Griffe.
            Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                for (int k = 0; k < chords[c].length; k++) ...<Widget>[
                  if (k > 0) SizedBox(width: gap / 2),
                  _HKeyCap(chords[c][k]),
                ],
              ],
            ),
          ],
        ],
      ),
    );
  }
}

class _HKeyCap extends StatelessWidget {
  const _HKeyCap(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      constraints: const BoxConstraints(minWidth: HSize.keyCapMinWidth),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x1 + HSize.hairline,
        vertical: HSize.hairline * 2,
      ),
      decoration: BoxDecoration(
        color: tokens.colors.bg2,
        border: Border.all(color: tokens.colors.line, width: HSize.hairline),
        borderRadius: HRadius.badgeRadius,
      ),
      child: Text(
        text,
        maxLines: 1,
        textAlign: TextAlign.center,
        style: tokens.typography.mono11.tinted(tokens.colors.fg1),
      ),
    );
  }
}
