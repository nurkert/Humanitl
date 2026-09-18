/// Die drei Markierungen des Editors, und der Controller, der sie malt
/// (HUM-047).
///
/// Der Diff-Glow ist eines der drei Signature-Elemente der Gestaltungsrichtung
/// (BACKLOG.md 5): Eine Stelle, an der ein Mensch einen Wert durch ein
/// Pseudonym ersetzt hat, bekommt einen Akzent-Unterstrich von einem Pixel und
/// eine Fläche in zehn Prozent Alpha. Das ist die eine Stelle im Programm, an
/// der die Akzentfarbe etwas anderes bedeutet als Fokus — und deshalb steht
/// sie hier, in der Widget-Schicht, und nicht im Feature: Wer die Farbe
/// ändert, ändert sie einmal.
///
/// Zwei Farben trennen, was gefunden wurde, von dem, was ersetzt wurde:
///
/// * [HEditorDecorationKind.secret] — ein prüfsummen-sicheres Geheimnis, in
///   `error/secret` (`#F0784F`). Orange und nicht rot, weil Rot in diesem
///   Programm ausschließlich „blockiert" heißt.
/// * [HEditorDecorationKind.pii] — ein Regex-Treffer oder ein Begriff des
///   Nutzers, in `held` (`#E0B24A`).
/// * [HEditorDecorationKind.replaced] — der Diff-Glow, in der Akzentfarbe.
///
/// # Warum ein Controller und kein `spanBuilder`
///
/// Die Spezifikation nennt `re_editor` und dessen `spanBuilder`. Das Paket ist
/// keine Abhängigkeit dieser Anwendung, und eine hinzuzunehmen heißt, den
/// Lock zu ändern, der genau dafür versioniert ist. Flutter kann dasselbe
/// ohne ein Paket: `TextEditingController.buildTextSpan` ist der Haken, den
/// `EditableText` bei jedem Zeichnen ruft, und er bekommt denselben Text mit
/// denselben Offsets. Der Unterschied zu `re_editor` ist, dass hier der ganze
/// Text in einem `TextSpan`-Baum steht statt einer sichtbaren Zeile — für die
/// Rümpfe, die dieser Editor annimmt (bis `preview.cap_bytes`, Binärdaten und
/// Übergroßes sind ausgeschlossen), ist das dieselbe Arbeit, die das
/// `EditableText` ohnehin tut.
library;

import 'package:flutter/widgets.dart';

import '../src/theme/h_theme.dart';
import '../src/tokens/tokens.dart';

/// Welche der drei Markierungen gemeint ist.
enum HEditorDecorationKind {
  /// Ein Fund, den eine Prüfsumme bestätigt hat: ein Geheimnis.
  secret,

  /// Ein Fund aus einem Muster oder aus der Begriffsliste des Nutzers.
  pii,

  /// Eine Stelle, die durch ein Pseudonym ersetzt wurde: der Diff-Glow.
  replaced,
}

/// Eine Markierung über `[start, end)` des Textes.
///
/// Die Offsets sind UTF-16-Code-Units, also genau das, womit `String`,
/// `TextSpan` und `TextSelection` in Flutter rechnen. Sie sind **nicht** die
/// Byte-Offsets des Daemons; die Umrechnung geschieht einmal beim Laden des
/// Entwurfs.
@immutable
class HEditorDecoration {
  /// Baut eine Markierung.
  const HEditorDecoration({
    required this.start,
    required this.end,
    required this.kind,
  });

  /// Erster Code-Unit-Offset, einschließlich.
  final int start;

  /// Letzter Code-Unit-Offset, ausschließlich.
  final int end;

  /// Welche der drei Markierungen.
  final HEditorDecorationKind kind;

  @override
  bool operator ==(Object other) =>
      other is HEditorDecoration &&
      other.start == start &&
      other.end == end &&
      other.kind == kind;

  @override
  int get hashCode => Object.hash(start, end, kind);

  @override
  String toString() => 'HEditorDecoration($start..$end, ${kind.name})';
}

/// Die Stile der drei Markierungen, aus den Token gebaut.
///
/// Eine reine Funktion über [HTokens]; kein Widget nötig, damit ein Test die
/// Farben prüfen kann, ohne einen Baum zu bauen.
abstract final class HEditorDecorations {
  /// Wie viel Alpha die Fläche des Diff-Glow bekommt: zehn Prozent.
  ///
  /// Die Obergrenze der Gestaltungsrichtung für jede Tönung (BACKLOG.md 5).
  static const double glowAlpha = 0.10;

  /// Die Dicke jedes Unterstrichs: ein Pixel.
  static const double underlineWidth = 1;

  /// Die Farbe einer Markierung.
  static Color colorOf(HTokens tokens, HEditorDecorationKind kind) =>
      switch (kind) {
        HEditorDecorationKind.secret => tokens.state.error,
        HEditorDecorationKind.pii => tokens.state.held,
        HEditorDecorationKind.replaced => tokens.colors.accent,
      };

  /// Der Stil einer Markierung über [base].
  ///
  /// Der Fund bekommt einen welligen Unterstrich, die Ersetzung einen geraden
  /// plus die Fläche: Die Welle sagt „sieh her", die Fläche sagt „erledigt",
  /// und die beiden Aussagen sollen sich auch ohne Farbe unterscheiden lassen
  /// (`docs/UX.md` 6: Farbe ist nie der einzige Träger).
  static TextStyle styleOf(
    HTokens tokens,
    HEditorDecorationKind kind,
    TextStyle base,
  ) {
    final Color color = colorOf(tokens, kind);
    return switch (kind) {
      HEditorDecorationKind.secret ||
      HEditorDecorationKind.pii => base.copyWith(
        decoration: TextDecoration.underline,
        decorationColor: color,
        decorationStyle: TextDecorationStyle.wavy,
        decorationThickness: underlineWidth,
      ),
      HEditorDecorationKind.replaced => base.copyWith(
        backgroundColor: color.withValues(alpha: glowAlpha),
        decoration: TextDecoration.underline,
        decorationColor: color,
        decorationStyle: TextDecorationStyle.solid,
        decorationThickness: underlineWidth,
      ),
    };
  }

  /// Zerlegt [text] in Abschnitte, in denen dieselbe Markierung gilt.
  ///
  /// Überlappungen sind möglich — ein Fund kann in einer Ersetzung liegen,
  /// wenn jemand einen Teil des Pseudonyms wieder markiert —, und die letzte
  /// Markierung in der Liste gewinnt. Die Reihenfolge ist damit bedeutsam und
  /// steht beim Aufrufer: Der Editor reicht erst die Funde, dann die
  /// Ersetzungen, weil eine erledigte Stelle nicht mehr rot leuchten soll.
  ///
  /// Was außerhalb von [text] liegt, wird geklemmt statt zu werfen: Die
  /// Markierungen kommen aus einem Entwurf, der einen Tastendruck älter sein
  /// kann als der Text, den `EditableText` gerade zeichnet, und ein
  /// `RangeError` mitten im Zeichnen nähme dem Menschen den Editor.
  static List<({int start, int end, HEditorDecorationKind? kind})> slice(
    String text,
    List<HEditorDecoration> decorations,
  ) {
    final List<HEditorDecorationKind?> perUnit =
        List<HEditorDecorationKind?>.filled(text.length, null);
    for (final HEditorDecoration decoration in decorations) {
      final int start = decoration.start.clamp(0, text.length);
      final int end = decoration.end.clamp(start, text.length);
      for (int i = start; i < end; i++) {
        perUnit[i] = decoration.kind;
      }
    }
    final List<({int start, int end, HEditorDecorationKind? kind})> out =
        <({int start, int end, HEditorDecorationKind? kind})>[];
    int run = 0;
    for (int i = 1; i <= text.length; i++) {
      if (i == text.length || perUnit[i] != perUnit[run]) {
        out.add((start: run, end: i, kind: perUnit[run]));
        run = i;
      }
    }
    return out;
  }
}

/// Ein `TextEditingController`, der die drei Markierungen malt.
///
/// Er ist der Griff, an dem das Feature die Markierungen wechselt: [update]
/// setzt eine neue Liste und meldet den Wechsel, ohne den Text anzufassen —
/// ein `notifyListeners` ohne Textänderung lässt Cursor und Auswahl stehen, wo
/// sie sind. Das ist die Bedingung dafür, dass ein Klick auf „Alle ersetzen"
/// den Menschen nicht aus seiner Zeile wirft (`docs/UX.md` 2.8).
class HDecoratedTextController extends TextEditingController {
  /// Baut einen Controller über [text] mit [decorations].
  HDecoratedTextController({
    super.text,
    List<HEditorDecoration> decorations = const <HEditorDecoration>[],
  }) : _decorations = List<HEditorDecoration>.of(decorations);

  List<HEditorDecoration> _decorations;

  /// Die Markierungen, die gerade gelten.
  List<HEditorDecoration> get decorations =>
      List<HEditorDecoration>.unmodifiable(_decorations);

  /// Setzt neue Markierungen und zeichnet neu.
  void update(List<HEditorDecoration> decorations) {
    _decorations = List<HEditorDecoration>.of(decorations);
    notifyListeners();
  }

  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle base = style ?? const TextStyle();
    // Während eine Eingabemethode ein Zeichen zusammensetzt — fcitx, ibus,
    // jede CJK-Eingabe —, zeichnet Flutter selbst den Unterstrich unter das,
    // was noch nicht feststeht. Diesen Rahmen gehört er, nicht uns: Die
    // Markierungen setzen ihn sonst außer Kraft, und der Mensch sähe nicht
    // mehr, welche Zeichen er gerade tippt. Er dauert wenige Tastendrücke.
    if (_decorations.isEmpty ||
        (withComposing && value.isComposingRangeValid)) {
      return super.buildTextSpan(
        context: context,
        style: style,
        withComposing: withComposing,
      );
    }
    return TextSpan(
      style: base,
      children: <TextSpan>[
        for (final ({int start, int end, HEditorDecorationKind? kind}) part
            in HEditorDecorations.slice(text, _decorations))
          TextSpan(
            text: text.substring(part.start, part.end),
            style: part.kind == null
                ? base
                : HEditorDecorations.styleOf(tokens, part.kind!, base),
          ),
      ],
    );
  }
}
