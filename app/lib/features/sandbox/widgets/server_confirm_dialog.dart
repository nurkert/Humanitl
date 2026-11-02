/// The one question before a start whose project picks a saved model
/// server other than the default (HUM-405, plainer words in HUM-414).
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
    final SavedServers? list = ref.watch(llmServersProvider).value;
    final SavedServer? saved = list?.named(server.name);
    final String origin = saved == null ? '' : serverOrigin(saved.endpoint);
    final String fallback = list?.active ?? '';
    return SandboxArrive(
      from: ArriveFrom.nowhere,
      child: HModal(
        key: const Key('sandbox-server-confirm'),
        title: Text(l10n.sandboxServerAllowTitle),
        onDismiss: onCancel,
        scrimSemanticsLabel: l10n.sandboxServerDontAllow,
        width: 580,
        actions: <Widget>[
          HButton(
            key: const Key('sandbox-server-confirm-cancel'),
            variant: HButtonVariant.secondary,
            autofocus: true,
            onPressed: onCancel,
            child: Text(l10n.sandboxServerDontAllow),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('sandbox-server-confirm-yes'),
            variant: HButtonVariant.primary,
            onPressed: onConfirm,
            child: Text(l10n.sandboxServerAllowStart),
          ),
        ],
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              l10n.sandboxServerAllowBody(server.name),
              key: const Key('sandbox-server-confirm-body'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x2),
            Text(
              l10n.sandboxServerAllowOtherwise(
                fallback.isEmpty ? '-' : fallback,
              ),
              key: const Key('sandbox-server-confirm-otherwise'),
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
            SizedBox(height: tokens.spacing.x3),
            _Fact(
              label: l10n.sandboxServerAllowFolder,
              value: server.projectDir,
            ),
            SizedBox(height: tokens.spacing.x2),
            _Fact(
              key: const Key('sandbox-server-confirm-origin'),
              label: l10n.sandboxServerAllowServer,
              value: origin.isEmpty ? server.name : '${server.name}  $origin',
            ),
          ],
        ),
      ),
    );
  }
}

/// One fact of the question: a small label over a value in mono, the whole
/// value, never cut.
class _Fact extends StatelessWidget {
  const _Fact({required this.label, required this.value, super.key});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(label, style: tokens.typography.ui11.tinted(tokens.colors.fg2)),
        SizedBox(height: tokens.spacing.x1 / 2),
        Text(value, style: tokens.typography.mono12.tinted(tokens.colors.fg0)),
      ],
    );
  }
}
