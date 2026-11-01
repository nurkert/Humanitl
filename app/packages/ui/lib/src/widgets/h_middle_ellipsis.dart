import 'package:flutter/widgets.dart';

/// Eine Zeile, die in der Mitte kürzt statt am Ende (HUM-406).
///
/// Für Pfade: Anfang und Ende sagen, wo etwas liegt und wie es heißt, die
/// Mitte ist der Teil, den man am ehesten entbehrt. Am Ende gekürzt stand von
/// `/home/nik/clients/acme` in der Kopfzeile der Sandbox nur
/// `/home/nik/clients/ac…`, und der Name des Projekts war weg.
///
/// Passt der ganze Text, steht er ungekürzt da. Sonst bleibt vom Anfang ein
/// Drittel und vom Ende zwei Drittel dessen, was in die Breite passt, mit `…`
/// dazwischen. Der Text braucht eine begrenzte Breite; ohne steht er ganz da.
class HMiddleEllipsis extends StatelessWidget {
  /// Creates the line for [text] in [style].
  const HMiddleEllipsis(this.text, {this.style, this.textKey, super.key});

  /// Der volle Text.
  final String text;

  /// Die Schrift; ohne erbt sie vom umgebenden `DefaultTextStyle`.
  final TextStyle? style;

  /// Der Schlüssel des gezeichneten `Text`, damit ein Test lesen kann, was
  /// dasteht.
  final Key? textKey;

  /// Das Zeichen in der Mitte.
  static const String ellipsis = '…';

  @override
  Widget build(BuildContext context) => LayoutBuilder(
    builder: (BuildContext context, BoxConstraints constraints) {
      final TextStyle resolved = DefaultTextStyle.of(context).style
          .merge(style);
      final TextScaler scaler = MediaQuery.textScalerOf(context);
      final TextDirection direction = Directionality.of(context);
      double widthOf(String value) {
        final TextPainter painter = TextPainter(
          text: TextSpan(text: value, style: resolved),
          textDirection: direction,
          textScaler: scaler,
          maxLines: 1,
        )..layout();
        final double width = painter.width;
        painter.dispose();
        return width;
      }

      final double room = constraints.maxWidth;
      String shown = text;
      if (room.isFinite && widthOf(text) > room) {
        // Geschnitten wird nach Graphemen, nicht nach UTF-16-Einheiten: Ein
        // Umlaut aus zwei Codepunkten oder ein Emoji im Ordnernamen zerfiele
        // sonst in ein halbes Zeichen.
        final Characters glyphs = text.characters;
        final int length = glyphs.length;
        // Die größte Zahl behaltener Zeichen, die mit `…` noch passt.
        int low = 0;
        int high = length - 1;
        String best = ellipsis;
        while (low <= high) {
          final int keep = (low + high) ~/ 2;
          final int head = keep ~/ 3;
          final int tail = keep - head;
          final String candidate =
              glyphs.take(head).string +
              ellipsis +
              glyphs.skip(length - tail).string;
          if (widthOf(candidate) <= room) {
            best = candidate;
            low = keep + 1;
          } else {
            high = keep - 1;
          }
        }
        shown = best;
      }
      // Ein Bildschirmleser bekommt immer den vollen Text, auch wenn die
      // Zeile ihn kürzt: „…" vorzulesen hilft niemandem.
      return Semantics(
        label: text,
        excludeSemantics: true,
        child: Text(
          shown,
          key: textKey,
          style: style,
          maxLines: 1,
          softWrap: false,
          overflow: TextOverflow.clip,
        ),
      );
    },
  );
}
