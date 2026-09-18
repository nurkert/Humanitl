/// Die Leiste über dem Editor: welche Funde es gibt, nach Art gezählt.
///
/// Sie zählt Arten und nicht Stellen: `[EMAIL ×2] [IBAN ×1]` sagt in einer
/// Zeile, worum es geht, und `[EMAIL ×2]` ist eine Zahl, die ein Mensch prüfen
/// kann, während elf einzelne Chips eine Liste wären, die er überfliegt. Wer
/// zur Stelle will, nimmt „Nächstes" oder `F3`.
///
/// Der Wert selbst steht nie darin. Ein Chip nennt Art und Anzahl, das
/// Popover nennt Art, Sicherheit und die ersten Zeichen; was gefunden wurde,
/// erfährt nur, wer in den Rumpf sieht (`daemon/crates/findings`, Regel 1).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../model/draft.dart';
import '../model/pseudonym_naming.dart';

/// Eine Art mit ihrer Zahl.
typedef FindingGroup = ({String label, int open, int done});

/// Die offenen und erledigten Funde, nach Typ-Kürzel gruppiert.
///
/// Die Reihenfolge ist die des ersten Auftretens und nicht alphabetisch: Sie
/// soll dieselbe sein wie im Rumpf, damit „das erste" dasselbe meint wie
/// „oben".
List<FindingGroup> groupFindings(List<FindingView> findings) {
  final List<String> order = <String>[];
  final Map<String, int> open = <String, int>{};
  final Map<String, int> done = <String, int>{};
  for (final FindingView view in findings) {
    final String label = PseudonymNaming.typeLabel(view.kind);
    if (!order.contains(label)) {
      order.add(label);
    }
    if (view.isOpen) {
      open[label] = (open[label] ?? 0) + 1;
    } else {
      done[label] = (done[label] ?? 0) + 1;
    }
  }
  return <FindingGroup>[
    for (final String label in order)
      (label: label, open: open[label] ?? 0, done: done[label] ?? 0),
  ];
}

/// Die Findings-Leiste.
class FindingsRail extends StatelessWidget {
  /// Baut die Leiste.
  const FindingsRail({
    required this.findings,
    required this.onReplaceAll,
    required this.onNext,
    required this.replaceAllLabel,
    required this.nextLabel,
    required this.cleanLabel,
    required this.chipLabel,
    this.enabled = true,
    super.key,
  });

  /// Die Funde des Entwurfs.
  final List<FindingView> findings;

  /// Ersetzt jeden offenen Fund.
  final VoidCallback onReplaceAll;

  /// Springt zum nächsten offenen Fund.
  final VoidCallback onNext;

  /// Die Beschriftung des Ersetzen-Knopfes.
  final String replaceAllLabel;

  /// Die Beschriftung des Sprung-Knopfes.
  final String nextLabel;

  /// Was dasteht, wenn nichts gefunden wurde.
  final String cleanLabel;

  /// Die Beschriftung eines Chips, aus Kürzel und Anzahl.
  final String Function(String label, int count) chipLabel;

  /// Falsch, solange nichts entschieden werden darf.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final List<FindingGroup> groups = groupFindings(findings);
    final int open = findings.where((FindingView view) => view.isOpen).length;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(bottom: BorderSide(color: tokens.colors.line)),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x2),
        child: Wrap(
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: <Widget>[
            if (groups.isEmpty)
              Text(
                cleanLabel,
                key: const Key('editor-findings-clean'),
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            for (final FindingGroup group in groups)
              HBadge(
                key: Key('editor-finding-chip-${group.label}'),
                text: chipLabel(group.label, group.open + group.done),
                mono: true,
                // Erledigt ist nicht mehr gelb: Ein Chip, der nach einer
                // Ersetzung weiter warnte, hielte den Menschen an einer
                // Stelle fest, die er schon geräumt hat (`docs/UX.md` 3.3).
                color: group.open > 0
                    ? tokens.state.held
                    : tokens.state.allowedEdited,
              ),
            if (open > 0) ...<Widget>[
              HButton(
                key: const Key('editor-replace-all'),
                variant: HButtonVariant.secondary,
                onPressed: enabled ? onReplaceAll : null,
                child: Text(replaceAllLabel),
              ),
              HButton(
                key: const Key('editor-next-finding'),
                variant: HButtonVariant.ghost,
                onPressed: enabled ? onNext : null,
                child: Text(nextLabel),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
