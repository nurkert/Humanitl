/// The question before the depot of an agent goes (HUM-521 point 1).
///
/// Cancel is the preselected answer; the destructive one needs a deliberate
/// press (`docs/UX.md` 5.4), as under "Remove server".
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import 'agent_inset.dart';

/// The question for [entry].
class AgentRemoveConfirm extends StatefulWidget {
  /// Creates the question.
  const AgentRemoveConfirm({
    required this.entry,
    required this.busy,
    required this.onConfirm,
    required this.onCancel,
    super.key,
  });

  /// The agent whose depot would go.
  final AgentDepot entry;

  /// A request is under way.
  final bool busy;

  /// Removes the depot.
  final VoidCallback onConfirm;

  /// Keeps it.
  final VoidCallback onCancel;

  @override
  State<AgentRemoveConfirm> createState() => _AgentRemoveConfirmState();
}

class _AgentRemoveConfirmState extends State<AgentRemoveConfirm> {
  /// Cancel takes the focus on arrival, also from a focused button:
  /// `autofocus` acts only while nothing in the scope has the focus.
  final FocusNode _cancel = FocusNode(debugLabel: 'agent-remove-cancel');

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        _cancel.requestFocus();
      }
    });
  }

  @override
  void dispose() {
    _cancel.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String agent = widget.entry.agent;
    return AgentInset(
      key: ValueKey<String>('agent-remove-confirm-$agent'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          AgentInsetTitle(
            l10n.settingsAgentsRemoveTitle(widget.entry.displayName),
          ),
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.settingsAgentsRemoveBody,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          AgentInsetActions(
            children: <Widget>[
              HButton(
                key: ValueKey<String>('agent-remove-cancel-$agent'),
                variant: HButtonVariant.secondary,
                focusNode: _cancel,
                onPressed: widget.onCancel,
                child: Text(l10n.settingsAgentsCancel),
              ),
              HButton(
                key: ValueKey<String>('agent-remove-yes-$agent'),
                variant: HButtonVariant.danger,
                onPressed: widget.busy ? null : widget.onConfirm,
                child: Text(l10n.settingsAgentsRemoveConfirm),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
