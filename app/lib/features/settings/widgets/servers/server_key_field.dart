/// The access key of a saved model server: where it lies, never the key;
/// the field for a new one; and the one origin it goes to (HUM-405,
/// HUM-414).
///
/// `Enter` in the field does not save: a key is pasted, and a save stays a
/// deliberate press of "Save" or `Ctrl+S`.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../secret_field.dart';
import 'server_form.dart';

/// The key part of [form].
class ServerKeyField extends StatelessWidget {
  /// The key of the server of [form].
  const ServerKeyField({required this.form, super.key});

  /// The form the field belongs to.
  final ServerForm form;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = form.server;
    final bool stored = form.keyStored;
    final bool goes = form.keyGoesWithOrigin;
    final bool warn = form.removeKey || goes;
    final String origin = serverOrigin(form.endpointText);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (server != null) ...<Widget>[
          Row(
            children: <Widget>[
              HGlyphIcon(
                warn ? HGlyph.triangleAlert : HGlyph.lock,
                size: 12,
                color: warn
                    ? tokens.colors.warning
                    : stored
                    ? tokens.stateText.allowed
                    : tokens.colors.fg2,
              ),
              SizedBox(width: tokens.spacing.x1),
              Expanded(
                child: Text(
                  form.removeKey
                      ? l10n.settingsServerKeyRemovePending
                      : goes
                      ? l10n.settingsServerKeyNewOrigin
                      : _keyState(l10n, server, form.keyStatus),
                  key: const Key('settings-key-state'),
                  style: tokens.typography.ui12.tinted(
                    warn
                        ? tokens.colors.warningText
                        : stored
                        ? tokens.colors.fg0
                        : tokens.colors.fg1,
                  ),
                ),
              ),
              // Geht der Schlüssel ohnehin mit der Adresse, gibt es nichts
              // zu entfernen.
              if (stored && !goes)
                HButton(
                  key: const Key('settings-key-remove'),
                  variant: HButtonVariant.ghost,
                  onPressed: form.busy ? null : form.toggleRemoveKey,
                  child: Text(
                    form.removeKey
                        ? l10n.settingsServerKeyKeep
                        : l10n.settingsKeyRemove,
                  ),
                ),
            ],
          ),
          SizedBox(height: tokens.spacing.x2),
        ],
        SecretField(
          controller: form.key,
          semanticsLabel: l10n.settingsKeyLabel,
          hint: stored
              ? l10n.settingsServerKeyReplaceHint
              : l10n.settingsKeyHint,
          enabled: !form.busy && !form.removeKey && !form.probing,
        ),
        if (origin.isNotEmpty) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.settingsServerSentOnlyTo(origin),
            key: const Key('settings-key-origin'),
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
        ],
      ],
    );
  }
}

/// The state line of the key of [server], never the key: where it lies when
/// the daemon said so, otherwise whether a reference stands.
String _keyState(
  AppLocalizations l10n,
  SavedServer server,
  LlmKeyStatus? status,
) {
  if (status == null) {
    return server.hasKey
        ? l10n.settingsServerKeyStoredShort
        : l10n.settingsServerKeyNoneShort;
  }
  if (!status.set) {
    return l10n.settingsServerKeyNoneShort;
  }
  return switch (status.store) {
    LlmKeyStore.file => l10n.settingsKeyStoredFile,
    LlmKeyStore.secretService ||
    LlmKeyStore.none => l10n.settingsKeyStoredKeyring,
  };
}
