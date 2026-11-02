/// The question before a saved model server goes (HUM-405, HUM-414).
///
/// Cancel is the preselected answer; the destructive one needs a deliberate
/// press (`docs/UX.md` 5.4). It names the profiles and the project that
/// pick the server, so nobody learns of them at the next start. The key is
/// deleted with it unless the box is cleared; the daemon keeps it anyway
/// when another server refers to it.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../setting_row.dart';

/// The question under the row "Remove server".
class ServerRemoveConfirm extends StatefulWidget {
  /// Asks before [server] goes.
  const ServerRemoveConfirm({
    required this.server,
    required this.keyStored,
    required this.users,
    required this.busy,
    required this.onConfirm,
    required this.onCancel,
    super.key,
  });

  /// The server that would go.
  final SavedServer server;

  /// Whether a key is stored for it.
  final bool keyStored;

  /// The profiles and the project that pick the server, in words.
  final List<String> users;

  /// A write is under way.
  final bool busy;

  /// Removes it; true when its key goes too.
  final ValueChanged<bool> onConfirm;

  /// Keeps it.
  final VoidCallback onCancel;

  @override
  State<ServerRemoveConfirm> createState() => _ServerRemoveConfirmState();
}

class _ServerRemoveConfirmState extends State<ServerRemoveConfirm> {
  bool _deleteKey = true;

  /// Cancel takes the focus on arrival, also from a focused row or button:
  /// `autofocus` only acts while nothing in the scope has the focus.
  final FocusNode _cancel = FocusNode(debugLabel: 'server-remove-cancel');

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
    return Padding(
      key: const Key('settings-server-remove-confirm'),
      padding: EdgeInsets.fromLTRB(
        SettingRowGrid.inset,
        0,
        SettingRowGrid.inset,
        tokens.spacing.x3,
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg2,
          border: Border.all(color: tokens.colors.lineStrong),
          borderRadius: BorderRadius.circular(tokens.radii.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                l10n.settingsServerRemoveTitle(widget.server.name),
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              if (widget.users.isNotEmpty)
                Padding(
                  padding: EdgeInsets.only(top: tokens.spacing.x1),
                  child: Text(
                    l10n.settingsServerRemoveProfiles(widget.users.join(', ')),
                    key: const Key('settings-server-remove-profiles'),
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ),
              if (widget.keyStored)
                HCheckbox(
                  key: const Key('settings-server-remove-key'),
                  label: l10n.settingsServerRemoveDeleteKey,
                  value: _deleteKey,
                  onChanged: (bool value) => setState(() => _deleteKey = value),
                ),
              SizedBox(height: tokens.spacing.x2),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    key: const Key('settings-server-remove-cancel'),
                    variant: HButtonVariant.secondary,
                    focusNode: _cancel,
                    onPressed: widget.onCancel,
                    child: Text(l10n.settingsServerCancel),
                  ),
                  HButton(
                    key: const Key('settings-server-remove-yes'),
                    variant: HButtonVariant.danger,
                    onPressed: widget.busy
                        ? null
                        : () =>
                              widget.onConfirm(widget.keyStored && _deleteKey),
                    child: Text(l10n.settingsServerRemoveConfirm),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
