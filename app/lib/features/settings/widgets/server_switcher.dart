/// The saved model servers in the model group of the settings: list, add,
/// rename, remove with a confirmation, activate (HUM-289).
///
/// # Only the active server gets its key
///
/// Every server keeps its access key in the system keyring under its own
/// name; a row says whether one is stored and never shows it. The fields
/// above this list (endpoint, model, access key) are the active server's:
/// switching here changes what they show. A check of a server that is not
/// active asks the daemon to use that server's own key, which it sends only
/// to that server's own address (`ProbeLlmRequest.server`); a key typed into
/// the form for a new server goes once to the endpoint typed beside it.
///
/// # A switch applies to the next session
///
/// Activation writes `config.toml`; a running session keeps the server it
/// started with. The help under the heading says so, every time.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/pasted_text.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'secret_field.dart';

/// What the switcher is doing besides listing.
sealed class _Mode {
  const _Mode();
}

final class _Listing extends _Mode {
  const _Listing();
}

final class _Adding extends _Mode {
  const _Adding();
}

final class _Renaming extends _Mode {
  const _Renaming(this.name);
  final String name;
}

final class _Removing extends _Mode {
  const _Removing(this.name);
  final String name;
}

/// The list of saved servers with its actions.
class ServerSwitcher extends ConsumerStatefulWidget {
  /// Creates the switcher.
  const ServerSwitcher({super.key});

  @override
  ConsumerState<ServerSwitcher> createState() => _ServerSwitcherState();
}

class _ServerSwitcherState extends ConsumerState<ServerSwitcher> {
  _Mode _mode = const _Listing();
  Diagnostic? _refusal;
  bool _busy = false;

  /// What a check of a saved server answered, by name.
  final Map<String, String> _checked = <String, String>{};

  /// Runs [change] and reports a refusal of the daemon under the list.
  Future<void> _run(Future<void> Function() change) async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    try {
      await change();
      if (mounted) {
        setState(() {
          _refusal = null;
          _mode = const _Listing();
        });
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() => _refusal = error.diagnostic);
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _activate(String name) =>
      _run(() async => ref.read(llmServersProvider.notifier).activate(name));

  Future<void> _rename(String name, String newName) => _run(
    () async => ref.read(llmServersProvider.notifier).rename(name, newName),
  );

  Future<void> _remove(String name, bool deleteKey, String? next) => _run(
    () async => ref
        .read(llmServersProvider.notifier)
        .remove(name, deleteKey: deleteKey, replacement: next),
  );

  /// Checks a saved server with its own key, at its own address.
  Future<void> _check(SavedServer server) async {
    final AppLocalizations l10n = context.l10n;
    try {
      final LlmProbe probe = await ref
          .read(daemonClientProvider)
          .probeLlm(
            server.endpoint,
            useStoredKey: server.hasKey,
            server: server.name,
          );
      final String said = probe.models.isNotEmpty
          ? l10n.settingsModelsFound(probe.models.length)
          : probe.answer.isNotEmpty
          ? l10n.settingsModelNoList(probe.answer)
          : l10n.settingsModelNone;
      if (mounted) {
        setState(() => _checked[server.name] = said);
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(
          () => _checked[server.name] = DiagnosticL10n.resolve(
            error.diagnostic,
            l10n,
          ).cause,
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServers list =
        ref.watch(llmServersProvider).value ?? SavedServers.none;
    final _Mode mode = _mode;
    final bool empty = list.servers.isEmpty;
    return Padding(
      key: const Key('settings-servers'),
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            l10n.settingsServersTitle,
            style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x1),
          // Ohne Server ein Satz und kein Absatz: Wer nur einen Endpunkt hat,
          // braucht die Regeln des Wechsels noch nicht.
          Text(
            empty ? l10n.settingsServersEmpty : l10n.settingsServersHelp,
            key: Key(
              empty ? 'settings-servers-empty' : 'settings-servers-help',
            ),
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x1),
          for (final SavedServer server in list.servers) ...<Widget>[
            _ServerRow(
              server: server,
              busy: _busy,
              checked: _checked[server.name],
              onUse: () => unawaited(_activate(server.name)),
              onCheck: () => unawaited(_check(server)),
              onRename: () => setState(() {
                _refusal = null;
                _mode = _Renaming(server.name);
              }),
              onRemove: () => setState(() {
                _refusal = null;
                _mode = _Removing(server.name);
              }),
            ),
            if (mode case _Renaming(:final String name)
                when name == server.name)
              _RenameForm(
                name: name,
                busy: _busy,
                onSave: (String newName) => unawaited(_rename(name, newName)),
                onCancel: () => setState(() => _mode = const _Listing()),
              ),
            if (mode case _Removing(:final String name)
                when name == server.name)
              _RemoveConfirm(
                server: server,
                next: server.active ? _nextAfter(list, server.name) : null,
                busy: _busy,
                onConfirm: (bool deleteKey, String? next) =>
                    unawaited(_remove(server.name, deleteKey, next)),
                onCancel: () => setState(() => _mode = const _Listing()),
              ),
          ],
          if (mode is _Adding)
            _AddForm(
              onSaved: () => setState(() {
                _refusal = null;
                _mode = const _Listing();
              }),
              onCancel: () => setState(() => _mode = const _Listing()),
            )
          else
            Align(
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: EdgeInsets.only(top: tokens.spacing.x2),
                child: HButton(
                  key: const Key('settings-server-add'),
                  variant: HButtonVariant.secondary,
                  size: HButtonSize.sm,
                  onPressed: _busy
                      ? null
                      : () => setState(() {
                          _refusal = null;
                          _mode = const _Adding();
                        }),
                  child: Text(l10n.settingsServerAdd),
                ),
              ),
            ),
          if (_refusal case final Diagnostic refused) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              DiagnosticL10n.resolve(refused, l10n).cause,
              key: const Key('settings-servers-refusal'),
              style: tokens.typography.ui12.tinted(tokens.stateText.error),
            ),
          ],
        ],
      ),
    );
  }
}

/// The server that becomes active when [name], the active one, is removed:
/// the first other one, or none when it is the last.
String? _nextAfter(SavedServers list, String name) {
  for (final SavedServer server in list.servers) {
    if (server.name != name) {
      return server.name;
    }
  }
  return null;
}

/// One saved server: name, address, key state and its actions.
class _ServerRow extends StatelessWidget {
  const _ServerRow({
    required this.server,
    required this.busy,
    required this.checked,
    required this.onUse,
    required this.onCheck,
    required this.onRename,
    required this.onRemove,
  });

  final SavedServer server;
  final bool busy;
  final String? checked;
  final VoidCallback onUse;
  final VoidCallback onCheck;
  final VoidCallback onRename;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String name = server.name;
    return Padding(
      key: ValueKey<String>('settings-server-$name'),
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Flexible(
                child: Text(
                  name,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.mono12.medium.tinted(
                    tokens.colors.fg0,
                  ),
                ),
              ),
              if (server.active) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HBadge(
                  key: ValueKey<String>('settings-server-active-$name'),
                  text: l10n.settingsServerActive,
                  color: tokens.state.allowed,
                ),
              ],
              SizedBox(width: tokens.spacing.x2),
              Text(
                server.hasKey
                    ? l10n.settingsServerKeyStored
                    : l10n.settingsServerKeyNone,
                style: tokens.typography.ui11.tinted(tokens.colors.fg1),
              ),
            ],
          ),
          Text(
            // Ohne Benutzer, Passwort und Query: Die Zeile zeigt, wohin die
            // Probe ginge, nicht, was in der Adresse steht.
            displayEndpoint(server.endpoint),
            overflow: TextOverflow.ellipsis,
            style: tokens.typography.mono11.tinted(tokens.colors.fg1),
          ),
          // Welche Profile diesen Server wählen (HUM-400): Wer ihn entfernt
          // oder umbenennt, sieht vorher, welche Sitzungen danach nicht mehr
          // starten.
          if (server.profiles.isNotEmpty)
            Text(
              l10n.settingsServerProfiles(server.profiles.join(', ')),
              key: ValueKey<String>('settings-server-profiles-$name'),
              style: tokens.typography.ui11.tinted(tokens.colors.fg1),
            ),
          if (checked case final String said)
            Text(
              said,
              key: ValueKey<String>('settings-server-checked-$name'),
              style: tokens.typography.ui11.tinted(tokens.colors.fg1),
            ),
          SizedBox(height: tokens.spacing.x1),
          Wrap(
            spacing: tokens.spacing.x1,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[
              if (!server.active) ...<Widget>[
                HButton(
                  key: ValueKey<String>('settings-server-use-$name'),
                  size: HButtonSize.sm,
                  onPressed: busy ? null : onUse,
                  child: Text(l10n.settingsServerUse),
                ),
                HButton(
                  key: ValueKey<String>('settings-server-check-$name'),
                  variant: HButtonVariant.ghost,
                  size: HButtonSize.sm,
                  onPressed: busy ? null : onCheck,
                  child: Text(l10n.settingsCheck),
                ),
              ],
              HButton(
                key: ValueKey<String>('settings-server-rename-$name'),
                variant: HButtonVariant.ghost,
                size: HButtonSize.sm,
                onPressed: busy ? null : onRename,
                child: Text(l10n.settingsServerRename),
              ),
              HButton(
                key: ValueKey<String>('settings-server-remove-$name'),
                variant: HButtonVariant.ghost,
                size: HButtonSize.sm,
                onPressed: busy ? null : onRemove,
                child: Text(l10n.settingsServerRemove),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// The endpoint without user, password, query and fragment.
@visibleForTesting
String displayEndpoint(String endpoint) {
  final Uri? uri = Uri.tryParse(endpoint);
  if (uri == null || !uri.hasScheme || uri.host.isEmpty) {
    return endpoint;
  }
  return Uri(
    scheme: uri.scheme,
    host: uri.host,
    port: uri.hasPort ? uri.port : null,
    path: uri.path,
  ).toString();
}

/// The new name of a server.
class _RenameForm extends StatefulWidget {
  const _RenameForm({
    required this.name,
    required this.busy,
    required this.onSave,
    required this.onCancel,
  });

  final String name;
  final bool busy;
  final ValueChanged<String> onSave;
  final VoidCallback onCancel;

  @override
  State<_RenameForm> createState() => _RenameFormState();
}

class _RenameFormState extends State<_RenameForm> {
  late final TextEditingController _name = TextEditingController(
    text: widget.name,
  );

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: Row(
        children: <Widget>[
          Expanded(
            child: HTextField(
              key: const Key('settings-server-rename-field'),
              controller: _name,
              semanticsLabel: l10n.settingsServerNameLabel,
              hint: l10n.settingsServerNameHint,
              mono: true,
              autofocus: true,
              onSubmitted: (String text) => widget.onSave(text.trim()),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('settings-server-rename-save'),
            size: HButtonSize.sm,
            onPressed: widget.busy
                ? null
                : () => widget.onSave(_name.text.trim()),
            child: Text(l10n.settingsServerSaveName),
          ),
          SizedBox(width: tokens.spacing.x1),
          HButton(
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: widget.onCancel,
            child: Text(l10n.settingsServerCancel),
          ),
        ],
      ),
    );
  }
}

/// The question before a server goes, inline under its row.
///
/// Cancel is the preselected answer; the destructive one needs a deliberate
/// press (`docs/UX.md` 5.4). The key is deleted with it unless the box is
/// cleared; the daemon keeps it anyway when another server refers to it.
class _RemoveConfirm extends StatefulWidget {
  const _RemoveConfirm({
    required this.server,
    required this.next,
    required this.busy,
    required this.onConfirm,
    required this.onCancel,
  });

  final SavedServer server;

  /// The server that becomes active, when the active one goes.
  final String? next;
  final bool busy;
  final void Function(bool deleteKey, String? next) onConfirm;
  final VoidCallback onCancel;

  @override
  State<_RemoveConfirm> createState() => _RemoveConfirmState();
}

class _RemoveConfirmState extends State<_RemoveConfirm> {
  bool _deleteKey = true;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String? next = widget.next;
    return Padding(
      key: const Key('settings-server-remove-confirm'),
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border.all(color: tokens.colors.line),
          borderRadius: BorderRadius.circular(tokens.radii.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x2),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                l10n.settingsServerRemoveTitle(widget.server.name),
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              if (next != null)
                Text(
                  l10n.settingsServerRemoveNext(next),
                  key: const Key('settings-server-remove-next'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              if (widget.server.hasKey)
                HCheckbox(
                  key: const Key('settings-server-remove-key'),
                  label: l10n.settingsServerRemoveDeleteKey,
                  value: _deleteKey,
                  onChanged: (bool value) => setState(() => _deleteKey = value),
                ),
              SizedBox(height: tokens.spacing.x1),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    key: const Key('settings-server-remove-cancel'),
                    variant: HButtonVariant.secondary,
                    size: HButtonSize.sm,
                    autofocus: true,
                    onPressed: widget.onCancel,
                    child: Text(l10n.settingsServerCancel),
                  ),
                  HButton(
                    key: const Key('settings-server-remove-yes'),
                    variant: HButtonVariant.danger,
                    size: HButtonSize.sm,
                    onPressed: widget.busy
                        ? null
                        : () => widget.onConfirm(
                            widget.server.hasKey && _deleteKey,
                            next,
                          ),
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

/// The form for a new server: name, endpoint, key, a check with that key
/// at that endpoint, the model.
class _AddForm extends ConsumerStatefulWidget {
  const _AddForm({required this.onSaved, required this.onCancel});

  final VoidCallback onSaved;
  final VoidCallback onCancel;

  @override
  ConsumerState<_AddForm> createState() => _AddFormState();
}

class _AddFormState extends ConsumerState<_AddForm> {
  final TextEditingController _name = TextEditingController();
  final TextEditingController _endpoint = TextEditingController();
  final TextEditingController _model = TextEditingController();
  final TextEditingController _key = TextEditingController();

  bool _busy = false;
  LlmProbe? _probe;
  Diagnostic? _refusal;

  /// The name of a server that is saved already while its key was refused;
  /// the form then only offers the key again (HUM-289).
  String? _savedName;

  /// The endpoint text the typed key belongs to.
  String _keyEndpoint = '';

  @override
  void initState() {
    super.initState();
    _endpoint.addListener(_dropKeyOnEndpointEdit);
  }

  /// A key typed for one endpoint never goes out to the next one, the same
  /// rule as in the model group below.
  void _dropKeyOnEndpointEdit() {
    if (_endpoint.text == _keyEndpoint) {
      return;
    }
    _keyEndpoint = _endpoint.text;
    _key.clear();
    _probe = null;
  }

  @override
  void dispose() {
    _endpoint.removeListener(_dropKeyOnEndpointEdit);
    _name.dispose();
    _endpoint.dispose();
    _model.dispose();
    _key.dispose();
    super.dispose();
  }

  /// Checks the typed endpoint with the typed key, once.
  Future<void> _check() async {
    final String endpoint = stripPasteDebris(_endpoint.text);
    if (endpoint.isEmpty || _busy) {
      return;
    }
    final String typed = stripPasteDebris(_key.text);
    setState(() => _busy = true);
    try {
      final LlmProbe probe = await ref
          .read(daemonClientProvider)
          .probeLlm(endpoint, apiKey: typed.isEmpty ? null : typed);
      if (mounted) {
        setState(() {
          _probe = probe;
          _refusal = null;
        });
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() {
          _probe = null;
          _refusal = error.diagnostic;
        });
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  Future<void> _save() async {
    if (_busy) {
      return;
    }
    final String model = _model.text.trim();
    final String typed = stripPasteDebris(_key.text);
    setState(() => _busy = true);
    try {
      final LlmServersSetting servers = ref.read(llmServersProvider.notifier);
      if (_savedName case final String saved) {
        // Der Server steht schon; nur der Schlüssel fehlt noch.
        if (typed.isNotEmpty) {
          await servers.storeKey(saved, typed);
        }
      } else {
        await servers.add(
          name: _name.text.trim(),
          endpoint: stripPasteDebris(_endpoint.text),
          models: <String>[if (model.isNotEmpty) model],
          key: typed,
        );
      }
      if (mounted) {
        widget.onSaved();
      }
    } on ServerSavedWithoutKey catch (error) {
      if (mounted) {
        setState(() {
          _savedName = error.name;
          _refusal = error.diagnostic;
        });
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() => _refusal = error.diagnostic);
      }
    } finally {
      // Leer nach jedem Versuch, wie im Formular darunter: Ein Schlüssel
      // bleibt nicht im Speicher des Formulars stehen.
      _key.clear();
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final LlmProbe? probe = _probe;
    final TextStyle label = tokens.typography.ui12.medium.tinted(
      tokens.colors.fg0,
    );
    final SizedBox gap = SizedBox(height: tokens.spacing.x2);
    return Padding(
      key: const Key('settings-server-add-form'),
      padding: EdgeInsets.only(top: tokens.spacing.x2),
      child: DecoratedBox(
        decoration: BoxDecoration(
          border: Border.all(color: tokens.colors.line),
          borderRadius: BorderRadius.circular(tokens.radii.card),
        ),
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x2),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(l10n.settingsServerNameLabel, style: label),
              HTextField(
                key: const Key('settings-server-name'),
                controller: _name,
                semanticsLabel: l10n.settingsServerNameLabel,
                hint: l10n.settingsServerNameHint,
                mono: true,
                autofocus: true,
                enabled: _savedName == null,
              ),
              gap,
              Text(l10n.settingsEndpointLabel, style: label),
              Row(
                children: <Widget>[
                  Expanded(
                    child: HTextField(
                      key: const Key('settings-server-endpoint'),
                      controller: _endpoint,
                      semanticsLabel: l10n.settingsEndpointLabel,
                      hint: l10n.settingsEndpointHint,
                      enabled: _savedName == null,
                      inputFormatters: const <TextInputFormatter>[
                        PasteDebrisFormatter(),
                      ],
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('settings-server-add-check'),
                    size: HButtonSize.sm,
                    onPressed: _busy ? null : () => unawaited(_check()),
                    child: Text(l10n.settingsCheck),
                  ),
                ],
              ),
              gap,
              Text(l10n.settingsKeyLabel, style: label),
              SecretField(
                controller: _key,
                semanticsLabel: l10n.settingsKeyLabel,
                hint: l10n.settingsKeyHint,
                enabled: !_busy,
                inputKey: const Key('settings-server-key-input'),
              ),
              if (probe != null) ...<Widget>[
                gap,
                Text(
                  probe.models.isNotEmpty
                      ? l10n.settingsModelsFound(probe.models.length)
                      : probe.answer.isNotEmpty
                      ? l10n.settingsModelNoList(probe.answer)
                      : l10n.settingsModelNone,
                  key: const Key('settings-server-add-probe'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
                Wrap(
                  spacing: tokens.spacing.x1,
                  runSpacing: tokens.spacing.x1,
                  children: <Widget>[
                    for (final String name in probe.shownModels)
                      HButton(
                        key: ValueKey<String>('settings-server-model-$name'),
                        variant: HButtonVariant.ghost,
                        size: HButtonSize.sm,
                        onPressed: () => setState(() => _model.text = name),
                        child: Text(name),
                      ),
                  ],
                ),
              ],
              gap,
              Text(l10n.settingsModelLabel, style: label),
              HTextField(
                key: const Key('settings-server-model'),
                controller: _model,
                semanticsLabel: l10n.settingsModelLabel,
                hint: l10n.settingsModelHint,
              ),
              if (_refusal case final Diagnostic refused) ...<Widget>[
                SizedBox(height: tokens.spacing.x1),
                Text(
                  switch (_savedName) {
                    final String saved => l10n.settingsServerSavedWithoutKey(
                      saved,
                      DiagnosticL10n.resolve(refused, l10n).cause,
                    ),
                    null => DiagnosticL10n.resolve(refused, l10n).cause,
                  },
                  key: const Key('settings-server-add-refusal'),
                  style: tokens.typography.ui12.tinted(tokens.stateText.error),
                ),
              ],
              gap,
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    variant: HButtonVariant.ghost,
                    size: HButtonSize.sm,
                    onPressed: widget.onCancel,
                    child: Text(l10n.settingsServerCancel),
                  ),
                  HButton(
                    key: const Key('settings-server-save'),
                    variant: HButtonVariant.primary,
                    size: HButtonSize.sm,
                    onPressed: _busy ? null : () => unawaited(_save()),
                    child: Text(
                      _savedName == null
                          ? l10n.settingsServerSave
                          : l10n.settingsKeyStore,
                    ),
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
