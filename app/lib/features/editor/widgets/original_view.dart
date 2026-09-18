/// Die linke Hälfte des Editors: die Anfrage, wie der Agent sie geschickt hat.
///
/// Sie ist lesend und bleibt es. Ihr Zweck ist der Vergleich: Wer rechts einen
/// Wert ersetzt, will links sehen, was dort stand — nicht als Erinnerung,
/// sondern als Beleg. Deshalb stehen beide Hälften auf denselben Zeilenumbrüchen
/// und in derselben Schrift, und die Funde sind hier markiert wie dort.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../model/draft.dart';

/// Das Original des Rumpfes, lesend, mit unterstrichenen Funden.
class OriginalView extends StatelessWidget {
  /// Baut die Ansicht.
  const OriginalView({
    required this.text,
    required this.findings,
    this.label = '',
    super.key,
  });

  /// Der ursprüngliche Rumpf.
  final String text;

  /// Die Funde, mit den Stellen, die sie im **Original** hatten.
  final List<FindingView> findings;

  /// Die Überschrift über der Hälfte.
  final String label;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (label.isNotEmpty)
          Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x3,
              vertical: tokens.spacing.x2,
            ),
            child: Text(
              label,
              style: tokens.typography.ui12.medium.tinted(tokens.colors.fg1),
            ),
          ),
        const HHairline(),
        Expanded(
          child: SingleChildScrollView(
            padding: EdgeInsets.all(tokens.spacing.x3),
            child: Text.rich(
              TextSpan(
                style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                children: <TextSpan>[
                  for (final ({int start, int end, HEditorDecorationKind? kind})
                      part
                      in HEditorDecorations.slice(text, _decorations()))
                    TextSpan(
                      text: text.substring(part.start, part.end),
                      style: part.kind == null
                          ? null
                          : HEditorDecorations.styleOf(
                              tokens,
                              part.kind!,
                              tokens.typography.mono12.tinted(
                                tokens.colors.fg1,
                              ),
                            ),
                    ),
                ],
              ),
              key: const Key('editor-original-body'),
            ),
          ),
        ),
      ],
    );
  }

  /// Die Markierungen des Originals: jeder Fund, gleich was aus ihm geworden
  /// ist.
  ///
  /// Auch ein ersetzter Fund bleibt links markiert. Links steht, was war, und
  /// was war, war ein Fund — ihn hier verschwinden zu lassen, nähme dem
  /// Vergleich seinen Gegenstand.
  List<HEditorDecoration> _decorations() => <HEditorDecoration>[
    for (final FindingView view in findings)
      if (view.location.kind == FindingLocation.body &&
          view.finding.spanEnd > view.finding.spanStart)
        HEditorDecoration(
          start: view.originalStart,
          end: view.originalEnd,
          kind: view.finding.tier == FindingTier.checksum
              ? HEditorDecorationKind.secret
              : HEditorDecorationKind.pii,
        ),
  ];
}
