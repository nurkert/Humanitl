/// The one modal of this screen (HUM-040).
///
/// Stopping a running agent cannot be taken back, so it is asked for. The
/// question names what is lost and what is not, and the destructive answer is
/// never the preselected one: the focus opens on Cancel, `Escape` and the
/// scrim both cancel, and nothing but a deliberate press on the second button
/// stops anything (BACKLOG.md 5, `docs/UX.md` 5.4).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// Asks before stopping a running agent.
class StopDialog extends StatelessWidget {
  /// Creates the dialog. [onCancel] closes it, [onConfirm] stops the agent.
  const StopDialog({
    required this.onCancel,
    required this.onConfirm,
    super.key,
  });

  /// Closes the dialog and changes nothing.
  final VoidCallback onCancel;

  /// Stops the agent.
  final VoidCallback onConfirm;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SandboxArrive(
      from: ArriveFrom.nowhere,
      child: HModal(
        title: Text(l10n.sandboxStopTitle),
        onDismiss: onCancel,
        scrimSemanticsLabel: l10n.sandboxStopCancel,
        actions: <Widget>[
          HButton(
            key: const Key('sandbox-stop-cancel'),
            variant: HButtonVariant.secondary,
            autofocus: true,
            onPressed: onCancel,
            child: Text(l10n.sandboxStopCancel),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('sandbox-stop-confirm'),
            variant: HButtonVariant.danger,
            onPressed: onConfirm,
            child: Text(l10n.sandboxStopConfirm),
          ),
        ],
        child: Text(
          l10n.sandboxStopBody,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}
