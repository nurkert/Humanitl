/// Das Mapping unter dem Editor, eingeklappt bis jemand es aufmacht.
///
/// Minimal mit Absicht: Dieses Issue hält die Zuordnung nur im Speicher, und
/// HUM-048 baut daraus das Panel mit Verschlüsselung, Export und Rücktausch.
/// Was hier schon gilt, gilt dort weiter: **Der Originalwert steht maskiert
/// da.** Die Zuordnung ist die eine Stelle, an der Pseudonym und Wert
/// nebeneinanderliegen; sie ganz zu zeigen hieße, den Wert wieder auf den
/// Bildschirm zu holen, den der Mensch gerade daraus entfernt hat.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../model/draft.dart';
import '../model/pseudonym_naming.dart';

/// Die Mapping-Leiste.
class MappingStrip extends StatefulWidget {
  /// Baut die Leiste.
  const MappingStrip({
    required this.replacements,
    required this.headingLabel,
    required this.pseudonymLabel,
    required this.typeLabel,
    required this.originalLabel,
    super.key,
  });

  /// Die angewandten Ersetzungen, in der Reihenfolge, in der sie geschahen.
  final List<Replacement> replacements;

  /// Die Überschrift, mit der Zahl der Einträge.
  final String Function(int count) headingLabel;

  /// Die Spaltenüberschrift für das Pseudonym.
  final String pseudonymLabel;

  /// Die Spaltenüberschrift für die Art.
  final String typeLabel;

  /// Die Spaltenüberschrift für den maskierten Originalwert.
  final String originalLabel;

  @override
  State<MappingStrip> createState() => _MappingStripState();
}

class _MappingStripState extends State<MappingStrip> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    if (widget.replacements.isEmpty) {
      return const SizedBox.shrink();
    }
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(top: BorderSide(color: tokens.colors.line)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Align(
            alignment: Alignment.centerLeft,
            child: HButton(
              key: const Key('editor-mapping-toggle'),
              variant: HButtonVariant.ghost,
              onPressed: () => setState(() => _open = !_open),
              child: Text(widget.headingLabel(widget.replacements.length)),
            ),
          ),
          if (_open)
            Padding(
              padding: EdgeInsets.fromLTRB(
                tokens.spacing.x3,
                0,
                tokens.spacing.x3,
                tokens.spacing.x2,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  _row(
                    tokens,
                    widget.pseudonymLabel,
                    widget.typeLabel,
                    widget.originalLabel,
                    heading: true,
                  ),
                  for (final Replacement done in widget.replacements)
                    _row(
                      tokens,
                      done.pseudonym,
                      PseudonymNaming.typeLabel(_kindOf(done)),
                      done.maskedOriginal,
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }

  /// Die Art einer Ersetzung, aus ihrem Pseudonym gelesen.
  ///
  /// `<EMAIL_1>` sagt sie selbst; ein Alias wie `Client-A` sagt sie nicht, und
  /// dann ist es ein Begriff des Nutzers. Der Umweg über den Namen erspart der
  /// Ersetzung ein Feld, das nur die Anzeige braucht.
  String _kindOf(Replacement done) {
    final RegExpMatch? match = RegExp(r'^<([A-Z0-9_]+)_\d+>$')
        .firstMatch(done.pseudonym);
    if (match == null) {
      return 'user_term';
    }
    final String label = match.group(1) ?? '';
    for (final MapEntry<String, String> entry in pseudonymTypeLabels.entries) {
      if (entry.value == label) {
        return entry.key;
      }
    }
    return 'custom:$label';
  }

  Widget _row(
    HTokens tokens,
    String pseudonym,
    String type,
    String original, {
    bool heading = false,
  }) {
    final TextStyle style = heading
        ? tokens.typography.ui11.tinted(tokens.colors.fg2)
        : tokens.typography.mono12.tinted(tokens.colors.fg0);
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x1),
      child: Row(
        children: <Widget>[
          Expanded(
            flex: 2,
            child: Text(
              pseudonym,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
          Expanded(
            child: Text(
              type,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
          Expanded(
            flex: 2,
            child: Text(
              original,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: heading
                  ? style
                  : tokens.typography.mono12.tinted(tokens.colors.fg1),
            ),
          ),
        ],
      ),
    );
  }
}
