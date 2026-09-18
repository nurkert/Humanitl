/// Der Chip „Edited“ an einer Anfrage, die so hinausging, wie ein Mensch sie
/// bearbeitet hat (HUM-047).
///
/// Er steht an drei Stellen — in der entschiedenen Zeile der Warteschlange, in
/// der Zeile der Historie und im Kopf ihres Details — und muss an allen dreien
/// gleich aussehen und gleich heißen. Die Warteschlange und die Historie sind
/// zwei Features, die einander nicht importieren (`docs/ARCHITECTURE.md` 5);
/// deshalb steht die Festlegung hier einmal, neben `flow_visual_state.dart`, statt als
/// drei Kopien, die beim ersten Farbwechsel auseinanderlaufen.
///
/// Kein neues Bauteil: Es ist das `HBadge` der Findings-Chips, in der Farbe
/// des Zustands `allowedEdited`. Die Fläche ist die auf 3:1 geklemmte
/// Zustandsfarbe, das Wort darauf ihre Textvariante (`HBadge.textColor`).
library;

import 'package:flutter/widgets.dart';

import '../../l10n/l10n.dart';
import 'ui.dart';

/// Das `HBadge` „Edited“ im Ton von `HFlowState.allowedEdited`.
class EditedBadge extends StatelessWidget {
  /// Erzeugt den Chip.
  const EditedBadge({super.key});

  @override
  Widget build(BuildContext context) => HBadge(
    text: context.l10n.flowEditedChip,
    color: HTheme.of(context).state.allowedEdited,
  );
}
