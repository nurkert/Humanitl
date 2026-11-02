/// The footer of a saved model server's form (HUM-405, HUM-414): the
/// refusal and Save and Cancel; while another choice waits, the question
/// whether to save, discard or keep editing; and for a server another client
/// removed, Discard and Save as new.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../setting_row.dart';
import 'server_form.dart';

/// What the footer asks for.
enum ServerFooterMode {
  /// Save and Cancel.
  edit,

  /// Another row, category or the closing overlay waits on the change.
  leaving,

  /// The server was removed elsewhere; the draft stays.
  removed,
}

/// The footer under the fields of [form].
class ServerFooter extends StatelessWidget {
  /// Creates the footer in [mode].
  const ServerFooter({
    required this.form,
    required this.mode,
    required this.onSave,
    required this.onCancel,
    required this.onSaveAndLeave,
    required this.onDiscard,
    required this.onStay,
    required this.onSaveAsNew,
    required this.keepEditing,
    super.key,
  });

  /// The form the footer belongs to.
  final ServerForm form;

  /// What it asks for.
  final ServerFooterMode mode;

  /// "Save".
  final VoidCallback onSave;

  /// "Cancel".
  final VoidCallback onCancel;

  /// "Save" while another choice waits.
  final VoidCallback onSaveAndLeave;

  /// "Discard": drop the change and go on.
  final VoidCallback? onDiscard;

  /// "Keep editing".
  final VoidCallback? onStay;

  /// "Save as new", for a removed server.
  final VoidCallback onSaveAsNew;

  /// The node of "Keep editing", the preselected answer.
  final FocusNode keepEditing;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = form.server;
    final Diagnostic? refused = form.refusal;
    final bool removed = mode == ServerFooterMode.removed;
    final bool canSave =
        !form.busy &&
        form.dirty &&
        form.nameProblem(l10n, removed: removed) == null;
    final Widget body = Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (refused != null)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x2),
            child: Text(
              _refusalText(l10n, refused),
              key: const Key('settings-server-refusal'),
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            ),
          ),
        if (mode == ServerFooterMode.leaving)
          Padding(
            padding: EdgeInsets.only(bottom: tokens.spacing.x2),
            child: Text(
              l10n.settingsServerLeaveQuestion(
                server?.name ?? l10n.settingsServerNewTitle,
              ),
              key: const Key('settings-server-leave'),
              style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
            ),
          ),
        Wrap(
          alignment: WrapAlignment.end,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: switch (mode) {
            ServerFooterMode.removed => <Widget>[
              HButton(
                key: const Key('settings-server-discard'),
                variant: HButtonVariant.ghost,
                onPressed: form.busy ? null : onDiscard,
                child: Text(l10n.settingsServerDiscard),
              ),
              HButton(
                key: const Key('settings-server-save-as-new'),
                variant: HButtonVariant.primary,
                onPressed:
                    !form.busy &&
                        form.nameText.isNotEmpty &&
                        form.nameProblem(l10n, removed: true) == null
                    ? onSaveAsNew
                    : null,
                child: Text(l10n.settingsServerSaveAsNew),
              ),
            ],
            ServerFooterMode.leaving => <Widget>[
              HButton(
                key: const Key('settings-server-keep-editing'),
                variant: HButtonVariant.secondary,
                focusNode: keepEditing,
                onPressed: onStay,
                child: Text(l10n.settingsServerKeepEditing),
              ),
              HButton(
                key: const Key('settings-server-discard'),
                variant: HButtonVariant.ghost,
                onPressed: form.busy ? null : onDiscard,
                child: Text(l10n.settingsServerDiscard),
              ),
              HButton(
                key: const Key('settings-server-save'),
                variant: HButtonVariant.primary,
                onPressed: canSave ? onSaveAndLeave : null,
                child: Text(l10n.settingsServerSaveChanges),
              ),
            ],
            ServerFooterMode.edit => <Widget>[
              if (form.dirty && server != null)
                Text(
                  l10n.settingsServerUnsaved,
                  key: const Key('settings-server-unsaved'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              HButton(
                key: const Key('settings-server-cancel'),
                variant: HButtonVariant.ghost,
                onPressed: form.busy ? null : onCancel,
                child: Text(l10n.settingsServerCancel),
              ),
              HButton(
                key: const Key('settings-server-save'),
                variant: HButtonVariant.primary,
                onPressed: canSave ? onSave : null,
                child: Text(
                  form.savedWithoutKey != null
                      ? l10n.settingsKeyStore
                      : form.isNew
                      ? l10n.settingsServerSave
                      : l10n.settingsServerSaveChanges,
                ),
              ),
            ],
          },
        ),
      ],
    );
    return Padding(
      padding: EdgeInsets.fromLTRB(
        SettingRowGrid.inset,
        tokens.spacing.x3,
        SettingRowGrid.inset,
        0,
      ),
      // Während die Fußzeile fragt, steht sie auf einer eigenen Fläche und
      // liest sich nicht als weitere Zeile des Formulars.
      child: mode == ServerFooterMode.edit
          ? body
          : DecoratedBox(
              decoration: BoxDecoration(
                color: tokens.colors.bg2,
                border: Border.all(color: tokens.colors.lineStrong),
                borderRadius: BorderRadius.circular(tokens.radii.card),
              ),
              child: Padding(
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: body,
              ),
            ),
    );
  }

  String _refusalText(AppLocalizations l10n, Diagnostic refused) {
    final String cause = DiagnosticL10n.resolve(refused, l10n).cause;
    if (form.savedWithoutKey case final String saved) {
      return l10n.settingsServerSavedWithoutKey(saved, cause);
    }
    if (form.partlySaved case final String part) {
      return l10n.settingsServerPartlySaved(part, cause);
    }
    return cause;
  }
}
