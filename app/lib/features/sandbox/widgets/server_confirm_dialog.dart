/// The one question before a start whose project picks a saved model
/// server other than the default (HUM-405).
///
/// The project profile lies in the folder the agent writes, so its choice of
/// a server is confirmed once per folder and server. The daemon refuses the
/// start until then (`CONFIG_022`) and keeps the confirmation outside the
/// project; this modal is how the human gives it. Cancel is preselected,
/// `Escape` and the scrim cancel, and only a deliberate press on the second
/// button confirms and starts (`docs/UX.md` 5.4).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'arrive.dart';

/// Asks before a start with the project's server [server].
class ServerConfirmDialog extends ConsumerWidget {
  /// Creates the question. [onCancel] closes it, [onConfirm] confirms and
  /// starts.
  const ServerConfirmDialog({
    required this.server,
    required this.onCancel,
    required this.onConfirm,
    super.key,
  });

  /// The server the project picks, with its folder.
  final LlmServerChoice server;

  /// Closes the question and starts nothing.
  final VoidCallback onCancel;

  /// Confirms the server for this folder and starts.
  final VoidCallback onConfirm;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String fallback = ref.watch(llmServersProvider).value?.active ?? '';
    return SandboxArrive(
      from: ArriveFrom.nowhere,
      child: HModal(
        key: const Key('sandbox-server-confirm'),
        title: Text(l10n.sandboxServerConfirmTitle(server.name)),
        onDismiss: onCancel,
        scrimSemanticsLabel: l10n.sandboxServerConfirmCancel,
        width: 540,
        actions: <Widget>[
          HButton(
            key: const Key('sandbox-server-confirm-cancel'),
            variant: HButtonVariant.secondary,
            autofocus: true,
            onPressed: onCancel,
            child: Text(l10n.sandboxServerConfirmCancel),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('sandbox-server-confirm-yes'),
            variant: HButtonVariant.primary,
            onPressed: onConfirm,
            child: Text(l10n.sandboxServerConfirmYes(server.name)),
          ),
        ],
        child: Text(
          l10n.sandboxServerConfirmBody(
            server.projectDir,
            server.name,
            fallback.isEmpty ? '-' : fallback,
          ),
          key: const Key('sandbox-server-confirm-body'),
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}
