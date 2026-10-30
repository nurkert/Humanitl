/// Die Rückfragen des Änderungs-Panels (HUM-351): Übernehmen, Verwerfen und
/// Überschreiben einer Datei mit Konflikt.
///
/// Wie beim Stopp ist die folgenreiche Antwort nie die vorgewählte: Der Fokus
/// steht auf „Abbrechen", `Escape` und die Fläche dahinter brechen ab
/// (`docs/UX.md` 5.4).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// Breiter als die Vorgabe von [HModal]: Pfad und Ordner stehen im Text,
/// und „Diese Datei überschreiben" neben „Abbrechen" braucht den Platz.
const double changesConfirmWidth = 520;

/// Eine Rückfrage mit „Abbrechen" und einer bestätigenden Antwort.
class ChangesConfirm extends StatelessWidget {
  /// Fragt [title] mit [body]; [confirmLabel] beschriftet die bestätigende
  /// Antwort, die [danger] rot zeichnet.
  const ChangesConfirm({
    required this.title,
    required this.body,
    required this.confirmLabel,
    required this.confirmKey,
    required this.onCancel,
    required this.onConfirm,
    this.danger = false,
    super.key,
  });

  /// Die Frage.
  final String title;

  /// Was die Antwort bewirkt; jeder Absatz ein Eintrag.
  final List<String> body;

  /// Die Beschriftung der bestätigenden Antwort.
  final String confirmLabel;

  /// Der Schlüssel der bestätigenden Antwort, für Tests.
  final Key confirmKey;

  /// Schließt die Frage und ändert nichts.
  final VoidCallback onCancel;

  /// Führt aus.
  final VoidCallback onConfirm;

  /// Wahr für eine Antwort, die nicht zurückzunehmen ist.
  final bool danger;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SandboxArrive(
      from: ArriveFrom.nowhere,
      child: HModal(
        title: Text(title),
        width: changesConfirmWidth,
        onDismiss: onCancel,
        scrimSemanticsLabel: l10n.sandboxChangesCancel,
        actions: <Widget>[
          HButton(
            key: const Key('changes-confirm-cancel'),
            variant: HButtonVariant.secondary,
            autofocus: true,
            onPressed: onCancel,
            child: Text(l10n.sandboxChangesCancel),
          ),
          HButton(
            key: confirmKey,
            variant: danger ? HButtonVariant.danger : HButtonVariant.primary,
            onPressed: onConfirm,
            child: Text(confirmLabel),
          ),
        ],
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (final (int place, String paragraph)
                in body.indexed) ...<Widget>[
              if (place > 0) SizedBox(height: tokens.spacing.x2),
              Text(
                paragraph,
                style: tokens.typography.ui13.tinted(tokens.colors.fg1),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
