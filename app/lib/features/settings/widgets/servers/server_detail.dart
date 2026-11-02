/// The detail view of one saved model server, or of a new one (HUM-405).
///
/// # Nothing goes out while typing
///
/// Address, model and key are a form: they stay in this view until "Save",
/// and "Cancel" drops them. The one gesture that reaches the server before is
/// "Check" (HUM-069, HUM-140), and it goes to the address in the field.
///
/// # The key goes only to its own server
///
/// The key field starts empty, never shows a stored key and is emptied after
/// every check and every save. A typed key belongs to the address it was
/// typed beside: editing the address empties it. "Check" sends it once to
/// that address, and "Save" stores it under this server's name, so the
/// daemon gives it to this server's origin and no other one. Without a typed
/// key, "Check" asks the daemon for this server's stored key, which it sends
/// only to this server's own origin (`ProbeLlmRequest.server`).
///
/// Every server is edited here, the default and every other one, without
/// becoming the default: `LlmServers(edit)` changes one entry.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ipc/client_providers.dart';
import '../../../../core/ipc/config_providers.dart';
import '../../../../core/ipc/daemon_client.dart';
import '../../../../core/ui/diagnostic_severity.dart';
import '../../../../core/ui/fix_control.dart';
import '../../../../core/ui/h_diagnostic_card.dart';
import '../../../../core/ui/pasted_text.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';
import '../secret_field.dart';
import '../setting_row.dart';

/// The id of the name row of a new server; no entry of the register.
const String serverNameRowId = 'llm.server_name';

/// The detail view of [server], or of a new server when it is null.
class ServerDetail extends ConsumerStatefulWidget {
  /// Creates the view.
  const ServerDetail({
    required this.list,
    required this.onClose,
    this.server,
    this.onRenamed,
    super.key,
  });

  /// The server shown; null while a new one is added.
  final SavedServer? server;

  /// Every saved server, for the server that follows a removed default.
  final SavedServers list;

  /// Back to the cards.
  final VoidCallback onClose;

  /// After a rename, with the new name.
  final ValueChanged<String>? onRenamed;

  @override
  ConsumerState<ServerDetail> createState() => _ServerDetailState();
}

/// What the view does besides showing the form.
enum _Aside { none, renaming, removing }

class _ServerDetailState extends ConsumerState<ServerDetail> {
  late final TextEditingController _name = TextEditingController();
  late final TextEditingController _endpoint = TextEditingController(
    text: widget.server?.endpoint ?? '',
  );
  late final TextEditingController _model = TextEditingController(
    text: widget.server?.model ?? '',
  );
  final TextEditingController _key = TextEditingController();
  final HContextMenuController _more = HContextMenuController();
  final GlobalKey _moreAnchor = GlobalKey();

  /// The address the typed key belongs to.
  late String _keyEndpoint = _endpoint.text;

  bool _busy = false;
  bool _probing = false;
  bool _removeKey = false;
  _Aside _aside = _Aside.none;
  LlmProbe? _probe;
  Diagnostic? _probeFailure;
  Diagnostic? _refusal;

  /// The name of a new server that was saved while its key was refused; the
  /// form then offers only the key again (HUM-289).
  String? _savedWithoutKey;

  /// What the daemon says about this server's key: where it lies, and a
  /// reference without a key (`LLM_009`). Never the key.
  LlmKeyStatus? _keyStatus;

  SavedServer? get _server => widget.server;
  bool get _isNew => _server == null;

  @override
  void initState() {
    super.initState();
    _endpoint.addListener(_dropKeyOnEndpointEdit);
    for (final TextEditingController field in <TextEditingController>[
      _name,
      _endpoint,
      _model,
      _key,
    ]) {
      field.addListener(_changed);
    }
    unawaited(_loadKeyStatus());
  }

  /// Asks the daemon about the key of this server.
  Future<void> _loadKeyStatus() async {
    final SavedServer? server = _server;
    if (server == null) {
      return;
    }
    try {
      final LlmKeyStatus status = await ref
          .read(daemonClientProvider)
          .llmKeyStatus(server: server.name);
      if (mounted) {
        setState(() => _keyStatus = status);
      }
    } on DaemonException {
      // Die Zeile sagt dann nur, ob ein Verweis steht.
    }
  }

  @override
  void didUpdateWidget(ServerDetail oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Kam derselbe Server mit neuen Werten vom Daemon (nach einem Speichern
    // oder einem Wechsel anderswo), folgen die Felder, die niemand geändert
    // hat.
    final SavedServer? before = oldWidget.server;
    final SavedServer? now = widget.server;
    if (before == null || now == null || before.name != now.name) {
      return;
    }
    if (_endpoint.text == before.endpoint && now.endpoint != before.endpoint) {
      _endpoint.text = now.endpoint;
    }
    if (_model.text == before.model && now.model != before.model) {
      _model.text = now.model;
    }
  }

  void _changed() {
    if (mounted) {
      setState(() {});
    }
  }

  /// A key typed for one address never goes out to the next one, not even
  /// through a "Check" before the field loses focus.
  void _dropKeyOnEndpointEdit() {
    if (_endpoint.text == _keyEndpoint) {
      return;
    }
    _keyEndpoint = _endpoint.text;
    _key.clear();
    _probe = null;
    _probeFailure = null;
  }

  @override
  void dispose() {
    _endpoint.removeListener(_dropKeyOnEndpointEdit);
    for (final TextEditingController field in <TextEditingController>[
      _name,
      _endpoint,
      _model,
      _key,
    ]) {
      field
        ..removeListener(_changed)
        ..dispose();
    }
    super.dispose();
  }

  /// The models to save: the default first, the others of the server after.
  List<String> get _models {
    final String chosen = _model.text.trim();
    return <String>[
      if (chosen.isNotEmpty) chosen,
      for (final String known in _server?.models ?? const <String>[])
        if (known != chosen) known,
    ];
  }

  String get _endpointText => stripPasteDebris(_endpoint.text);

  bool get _endpointChanged => _endpointText != (_server?.endpoint ?? '');

  bool get _modelChanged => _model.text.trim() != (_server?.model ?? '');

  bool get _keyTyped => stripPasteDebris(_key.text).isNotEmpty;

  /// Whether Save takes the stored key away: the address gets another
  /// origin (scheme, host, port), and the daemon then drops the key, which
  /// was stored for the old one (HUM-405, review).
  bool get _keyGoesWithOrigin {
    final SavedServer? server = _server;
    return server != null &&
        server.hasKey &&
        !_removeKey &&
        _endpointChanged &&
        !sameOrigin(_endpointText, server.endpoint);
  }

  /// Whether the form holds anything "Save" would write.
  bool get _dirty {
    if (_isNew) {
      return _savedWithoutKey != null
          ? _keyTyped
          : _name.text.trim().isNotEmpty || _endpointText.isNotEmpty;
    }
    return _endpointChanged || _modelChanged || _keyTyped || _removeKey;
  }

  /// "Check": the one gesture of this view that reaches the network.
  Future<void> _check() async {
    final String endpoint = _endpointText;
    if (endpoint.isEmpty || _probing) {
      return;
    }
    final String typed = stripPasteDebris(_key.text);
    final SavedServer? server = _server;
    // Der hinterlegte Schlüssel nur für die gespeicherte Adresse dieses
    // Servers; der Daemon gibt ihn ohnehin nur an deren Herkunft.
    final bool stored =
        typed.isEmpty &&
        server != null &&
        server.hasKey &&
        !_removeKey &&
        endpoint == server.endpoint;
    setState(() {
      _probing = true;
      _probeFailure = null;
    });
    try {
      final LlmProbe probe = await ref
          .read(daemonClientProvider)
          .probeLlm(
            endpoint,
            apiKey: typed.isEmpty ? null : typed,
            useStoredKey: stored,
            server: server?.name,
          );
      if (server != null) {
        ref
            .read(serverChecksProvider.notifier)
            .record(
              server.name,
              ServerCheck.found(
                endpoint: endpoint,
                models: probe.models.length,
              ),
            );
      }
      if (mounted) {
        setState(() => _probe = probe);
      }
    } on DaemonException catch (error) {
      if (server != null) {
        ref
            .read(serverChecksProvider.notifier)
            .record(
              server.name,
              ServerCheck.failed(endpoint: endpoint, failure: error.diagnostic),
            );
      }
      if (mounted) {
        setState(() {
          _probe = null;
          _probeFailure = error.diagnostic;
        });
      }
    } finally {
      // Leer nach der Probe, wie nach dem Speichern: Der Schlüssel ging
      // einmal an diese Adresse und wartet nicht im Formular auf die nächste
      // Geste.
      if (mounted) {
        _key.clear();
        setState(() => _probing = false);
      }
    }
  }

  Future<void> _save() async {
    if (_busy || !_dirty) {
      return;
    }
    final String typed = stripPasteDebris(_key.text);
    setState(() => _busy = true);
    try {
      final LlmServersSetting servers = ref.read(llmServersProvider.notifier);
      final SavedServer? server = _server;
      if (server == null) {
        if (_savedWithoutKey case final String saved) {
          await servers.storeKey(saved, typed);
        } else {
          await servers.add(
            name: _name.text.trim(),
            endpoint: _endpointText,
            models: _models,
            key: typed,
          );
        }
        if (mounted) {
          widget.onClose();
        }
        return;
      }
      if (_endpointChanged || _modelChanged) {
        await servers.edit(
          server.name,
          endpoint: _endpointChanged ? _endpointText : null,
          models: _modelChanged ? _models : null,
        );
      }
      // Nach der Adresse: Der Schlüssel gehört zu der Adresse, die jetzt im
      // Feld steht, und geht unter dem Namen dieses Servers in den Speicher.
      if (typed.isNotEmpty) {
        await servers.storeKey(server.name, typed);
      } else if (_removeKey) {
        await ref.read(llmKeyProvider.notifier).clear(server: server.name);
      }
      if (mounted) {
        setState(() {
          _refusal = null;
          _removeKey = false;
        });
        unawaited(_loadKeyStatus());
      }
    } on ServerSavedWithoutKey catch (error) {
      if (mounted) {
        setState(() {
          _savedWithoutKey = error.name;
          _refusal = error.diagnostic;
        });
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() => _refusal = error.diagnostic);
      }
    } finally {
      // Leer nach jedem Versuch: Ein Schlüssel bleibt nicht im Speicher des
      // Formulars stehen, auch nicht nach einer Absage.
      _key.clear();
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  void _cancel() {
    final SavedServer? server = _server;
    if (server == null || !_dirty) {
      widget.onClose();
      return;
    }
    setState(() {
      _endpoint.text = server.endpoint;
      _model.text = server.model;
      _key.clear();
      _removeKey = false;
      _refusal = null;
      _probe = null;
      _probeFailure = null;
    });
  }

  Future<void> _run(Future<void> Function() change) async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    try {
      await change();
      if (mounted) {
        setState(() => _refusal = null);
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

  void _makeDefault(String name) => unawaited(
    _run(() async => ref.read(llmServersProvider.notifier).activate(name)),
  );

  void _rename(String name, String newName) => unawaited(
    _run(() async {
      await ref.read(llmServersProvider.notifier).rename(name, newName);
      ref.read(serverChecksProvider.notifier).renamed(name, newName);
      if (mounted) {
        setState(() => _aside = _Aside.none);
      }
      widget.onRenamed?.call(newName);
    }),
  );

  void _remove(String name, bool deleteKey, String? next) => unawaited(
    _run(() async {
      await ref
          .read(llmServersProvider.notifier)
          .remove(name, deleteKey: deleteKey, replacement: next);
      widget.onClose();
    }),
  );

  void _openMore() {
    final RenderObject? box = _moreAnchor.currentContext?.findRenderObject();
    if (box is! RenderBox || !box.hasSize) {
      return;
    }
    _more.open(box.localToGlobal(Offset(0, box.size.height + 2)));
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = _server;
    final bool knownKey = _savedWithoutKey != null;
    return Column(
      key: ValueKey<String>('settings-server-detail-${server?.name ?? '+'}'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _header(context),
        if (server != null && _aside == _Aside.renaming)
          _RenameForm(
            name: server.name,
            busy: _busy,
            onSave: (String newName) => _rename(server.name, newName),
            onCancel: () => setState(() => _aside = _Aside.none),
          ),
        if (server != null && _aside == _Aside.removing)
          _RemoveConfirm(
            server: server,
            next: server.active ? _nextAfter(widget.list, server.name) : null,
            busy: _busy,
            onConfirm: (bool deleteKey, String? next) =>
                _remove(server.name, deleteKey, next),
            onCancel: () => setState(() => _aside = _Aside.none),
          ),
        SizedBox(height: tokens.spacing.x2),
        if (server == null) ...<Widget>[
          SettingRow(
            settingId: serverNameRowId,
            title: l10n.settingsServerNameLabel,
            help: l10n.settingsServerNameHelp,
            fill: true,
            child: HTextField(
              key: const Key('settings-server-name'),
              controller: _name,
              semanticsLabel: l10n.settingsServerNameLabel,
              hint: l10n.settingsServerNameHint,
              autofocus: true,
              enabled: !knownKey,
            ),
          ),
          const HHairline(),
        ],
        SettingRow(
          settingId: FormKeys.llmEndpoint,
          help: l10n.settingsServerAddressHelp,
          fill: true,
          child: Row(
            children: <Widget>[
              Expanded(
                child: HTextField(
                  key: const Key('settings-endpoint'),
                  controller: _endpoint,
                  enabled: !knownKey,
                  semanticsLabel: l10n.settingsEndpointLabel,
                  hint: l10n.settingsEndpointHint,
                  inputFormatters: const <TextInputFormatter>[
                    PasteDebrisFormatter(),
                  ],
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              HButton(
                key: const Key('settings-check'),
                onPressed: _probing || _endpointText.isEmpty
                    ? null
                    : () => unawaited(_check()),
                child: Text(
                  _probing ? l10n.settingsChecking : l10n.settingsCheck,
                ),
              ),
            ],
          ),
        ),
        if (_probeFailure case final Diagnostic failure)
          _Inset(
            child: _ProbeFinding(
              diagnostic: failure,
              findingKey: 'settings-probe-failure',
            ),
          ),
        if (_probe case final LlmProbe probe)
          _Inset(child: _probeResult(context, probe)),
        const HHairline(),
        SettingRow(
          settingId: FormKeys.llmModels,
          help: l10n.settingsServerModelsHelp,
          fill: true,
          child: HTextField(
            key: const Key('settings-model'),
            controller: _model,
            enabled: !knownKey,
            semanticsLabel: l10n.settingsModelLabel,
            hint: l10n.settingsModelHint,
          ),
        ),
        const HHairline(),
        SettingRow(
          settingId: FormKeys.llmApiKeyRef,
          help: l10n.settingsServerKeyHelp,
          refusal: _keyStatus?.diagnostics.firstOrNull,
          refusalKey: const Key('settings-key-refusal'),
          fill: true,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (server != null)
                Row(
                  children: <Widget>[
                    HGlyphIcon(
                      _keyGoesWithOrigin ? HGlyph.triangleAlert : HGlyph.lock,
                      size: 12,
                      color: _keyGoesWithOrigin
                          ? tokens.colors.warning
                          : server.hasKey && !_removeKey
                          ? tokens.stateText.allowed
                          : tokens.colors.fg2,
                    ),
                    SizedBox(width: tokens.spacing.x1),
                    Expanded(
                      child: Text(
                        _removeKey
                            ? l10n.settingsServerKeyRemovePending
                            : _keyGoesWithOrigin
                            ? l10n.settingsServerKeyNewOrigin
                            : _keyState(l10n, server, _keyStatus),
                        key: const Key('settings-key-state'),
                        style: tokens.typography.ui12.tinted(
                          _removeKey || _keyGoesWithOrigin
                              ? tokens.colors.warningText
                              : server.hasKey
                              ? tokens.stateText.allowed
                              : tokens.colors.fg1,
                        ),
                      ),
                    ),
                    // Geht der Schlüssel ohnehin mit der Adresse, gibt es
                    // nichts zu entfernen.
                    if (server.hasKey && !_keyGoesWithOrigin)
                      HButton(
                        key: const Key('settings-key-remove'),
                        variant: HButtonVariant.ghost,
                        onPressed: _busy
                            ? null
                            : () => setState(() {
                                _removeKey = !_removeKey;
                                if (_removeKey) {
                                  _key.clear();
                                }
                              }),
                        child: Text(
                          _removeKey
                              ? l10n.settingsServerKeyKeep
                              : l10n.settingsKeyRemove,
                        ),
                      ),
                  ],
                ),
              if (server != null) SizedBox(height: tokens.spacing.x2),
              SecretField(
                controller: _key,
                semanticsLabel: l10n.settingsKeyLabel,
                hint: server != null && server.hasKey
                    ? l10n.settingsServerKeyReplaceHint
                    : l10n.settingsKeyHint,
                enabled: !_busy && !_removeKey,
                onSubmitted: (String _) => unawaited(_save()),
              ),
            ],
          ),
        ),
        const HHairline(),
        SizedBox(height: tokens.spacing.x3),
        _footer(context),
      ],
    );
  }

  Widget _header(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = _server;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: HButton(
              key: const Key('settings-server-back'),
              variant: HButtonVariant.ghost,
              onPressed: widget.onClose,
              leading: RotatedBox(
                quarterTurns: 2,
                child: HGlyphIcon(
                  HGlyph.chevronRight,
                  size: 12,
                  color: tokens.colors.fg1,
                ),
              ),
              child: Text(l10n.settingsServerBack),
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Row(
            children: <Widget>[
              Expanded(
                child: Row(
                  children: <Widget>[
                    Flexible(
                      child: Semantics(
                        header: true,
                        child: Text(
                          server?.name ?? l10n.settingsServerNewTitle,
                          key: const Key('settings-server-title'),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: server == null
                              ? tokens.typography.ui16.semibold.tinted(
                                  tokens.colors.fg0,
                                )
                              : tokens.typography.mono14.semibold.tinted(
                                  tokens.colors.fg0,
                                ),
                        ),
                      ),
                    ),
                    if (server != null && server.active) ...<Widget>[
                      SizedBox(width: tokens.spacing.x2),
                      HGlyphIcon(
                        HGlyph.star,
                        size: 14,
                        color: tokens.colors.accent,
                      ),
                      SizedBox(width: tokens.spacing.x1),
                      Text(
                        l10n.settingsServerDefault,
                        key: const Key('settings-server-detail-default'),
                        style: tokens.typography.ui12.medium.tinted(
                          tokens.colors.accentText,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              if (server != null)
                HContextMenu(
                  controller: _more,
                  semanticsLabel: l10n.settingsServerMore,
                  itemsBuilder: () => <HMenuItem>[
                    HMenuItem(
                      label: l10n.settingsServerRename,
                      enabled: !_busy,
                      onSelected: () =>
                          setState(() => _aside = _Aside.renaming),
                    ),
                    HMenuItem(
                      label: l10n.settingsServerRemove,
                      enabled: !_busy,
                      onSelected: () =>
                          setState(() => _aside = _Aside.removing),
                    ),
                  ],
                  child: KeyedSubtree(
                    key: _moreAnchor,
                    child: HButton(
                      key: const Key('settings-server-more'),
                      variant: HButtonVariant.ghost,
                      onPressed: _busy ? null : _openMore,
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(l10n.settingsServerMore),
                          SizedBox(width: tokens.spacing.x1),
                          RotatedBox(
                            quarterTurns: 1,
                            child: HGlyphIcon(
                              HGlyph.chevronRight,
                              size: 12,
                              color: tokens.colors.fg1,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
          if (server != null)
            Text(
              server.endpoint.isEmpty
                  ? l10n.settingsServerNoAddress
                  : displayEndpoint(server.endpoint),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.mono11.tinted(tokens.colors.fg1),
            ),
        ],
      ),
    );
  }

  Widget _footer(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = _server;
    final Diagnostic? refused = _refusal;
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (refused != null)
            Padding(
              padding: EdgeInsets.only(bottom: tokens.spacing.x2),
              child: Text(
                switch (_savedWithoutKey) {
                  final String saved => l10n.settingsServerSavedWithoutKey(
                    saved,
                    DiagnosticL10n.resolve(refused, l10n).cause,
                  ),
                  null => DiagnosticL10n.resolve(refused, l10n).cause,
                },
                key: const Key('settings-server-refusal'),
                style: tokens.typography.ui12.tinted(tokens.stateText.error),
              ),
            ),
          Wrap(
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              if (server != null && !server.active)
                HButton(
                  key: const Key('settings-server-make-default'),
                  leading: HGlyphIcon(
                    HGlyph.star,
                    size: 12,
                    color: tokens.colors.fg1,
                  ),
                  onPressed: _busy ? null : () => _makeDefault(server.name),
                  child: Text(l10n.settingsServerMakeDefault),
                )
              else
                const SizedBox.shrink(),
              Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (_dirty && server != null) ...<Widget>[
                    Text(
                      l10n.settingsServerUnsaved,
                      key: const Key('settings-server-unsaved'),
                      style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                    ),
                    SizedBox(width: tokens.spacing.x3),
                  ],
                  HButton(
                    key: const Key('settings-server-cancel'),
                    variant: HButtonVariant.ghost,
                    onPressed: _busy ? null : _cancel,
                    child: Text(l10n.settingsServerCancel),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('settings-server-save'),
                    variant: HButtonVariant.primary,
                    onPressed: _busy || !_dirty
                        ? null
                        : () => unawaited(_save()),
                    child: Text(
                      _savedWithoutKey != null
                          ? l10n.settingsKeyStore
                          : _isNew
                          ? l10n.settingsServerSave
                          : l10n.settingsServerSaveChanges,
                    ),
                  ),
                ],
              ),
            ],
          ),
        ],
      ),
    );
  }

  /// What a check answered: the models to pick from, whether a key went
  /// along, and every finding beside the address, not in a log (HUM-140).
  Widget _probeResult(BuildContext context, LlmProbe probe) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.only(bottom: tokens.spacing.x2),
          child: Column(
            key: const Key('settings-probe-result'),
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                probe.models.isNotEmpty
                    ? l10n.settingsModelsFound(probe.models.length)
                    // Was zurückkam, wenn keine Modellliste kam (HUM-283).
                    : probe.answer.isNotEmpty
                    ? l10n.settingsModelNoList(probe.answer)
                    : l10n.settingsModelNone,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
              if (probe.keySent)
                Text(
                  l10n.settingsKeySent,
                  key: const Key('settings-probe-key-sent'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              SizedBox(height: tokens.spacing.x1),
              Wrap(
                spacing: tokens.spacing.x1,
                runSpacing: tokens.spacing.x1,
                children: <Widget>[
                  for (final String name in probe.shownModels)
                    HButton(
                      key: ValueKey<String>('settings-model-$name'),
                      variant: name == _model.text.trim()
                          ? HButtonVariant.secondary
                          : HButtonVariant.ghost,
                      onPressed: () => setState(() => _model.text = name),
                      child: Text(name),
                    ),
                ],
              ),
              if (probe.hiddenModels > 0)
                Text(
                  l10n.settingsModelsHidden(probe.hiddenModels),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                ),
            ],
          ),
        ),
        for (final Diagnostic finding in probe.diagnostics)
          _ProbeFinding(
            diagnostic: finding,
            findingKey: 'settings-probe-${finding.code}',
          ),
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

/// The server that becomes the default when [name], the default, is
/// removed: the first other one, or none when it is the last.
String? _nextAfter(SavedServers list, String name) {
  for (final SavedServer server in list.servers) {
    if (server.name != name) {
      return server.name;
    }
  }
  return null;
}

/// Whether two addresses share their origin: scheme, host and port with
/// the default ports of `http` and `https`. Only to warn before Save; the
/// daemon decides (`humanitl_config::servers::same_origin`).
@visibleForTesting
bool sameOrigin(String a, String b) {
  final Uri? left = Uri.tryParse(a);
  final Uri? right = Uri.tryParse(b);
  if (left == null || right == null || left.host.isEmpty || !left.hasScheme) {
    return false;
  }
  return left.scheme.toLowerCase() == right.scheme.toLowerCase() &&
      left.host.toLowerCase() == right.host.toLowerCase() &&
      left.port == right.port;
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

/// [child] on the left edge of the text of the rows.
class _Inset extends StatelessWidget {
  const _Inset({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.symmetric(horizontal: SettingRowGrid.inset),
    child: child,
  );
}

/// One finding of the check, as a card under the address.
class _ProbeFinding extends StatelessWidget {
  const _ProbeFinding({required this.diagnostic, required this.findingKey});

  final Diagnostic diagnostic;
  final String findingKey;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: HDiagnosticCard(
        key: Key(findingKey),
        code: diagnostic.code,
        severityLabel: severityLabel(l10n, diagnostic.severity),
        color: severityColor(tokens, diagnostic.severity),
        title: text.title,
        why: text.cause,
        fix: FixControl(fix: diagnostic.fix),
        docsUrl: diagnostic.docsUrl,
        width: 480,
      ),
    );
  }
}

/// The new name of a server, under the header.
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
      key: const Key('settings-server-rename-form'),
      padding: EdgeInsets.fromLTRB(
        SettingRowGrid.inset,
        tokens.spacing.x3,
        SettingRowGrid.inset,
        0,
      ),
      child: Row(
        children: <Widget>[
          Expanded(
            child: HTextField(
              key: const Key('settings-server-rename-field'),
              controller: _name,
              semanticsLabel: l10n.settingsServerNameLabel,
              hint: l10n.settingsServerNameHint,
              autofocus: true,
              onSubmitted: (String text) => widget.onSave(text.trim()),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            variant: HButtonVariant.ghost,
            onPressed: widget.onCancel,
            child: Text(l10n.settingsServerCancel),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('settings-server-rename-save'),
            variant: HButtonVariant.primary,
            onPressed: widget.busy
                ? null
                : () => widget.onSave(_name.text.trim()),
            child: Text(l10n.settingsServerSaveName),
          ),
        ],
      ),
    );
  }
}

/// The question before a server goes, under the header.
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

  /// The server that becomes the default, when the default goes.
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
      padding: EdgeInsets.fromLTRB(
        SettingRowGrid.inset,
        tokens.spacing.x3,
        SettingRowGrid.inset,
        0,
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
              if (next != null)
                Text(
                  l10n.settingsServerRemoveNextDefault(next),
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
              SizedBox(height: tokens.spacing.x2),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    key: const Key('settings-server-remove-cancel'),
                    variant: HButtonVariant.secondary,
                    autofocus: true,
                    onPressed: widget.onCancel,
                    child: Text(l10n.settingsServerCancel),
                  ),
                  HButton(
                    key: const Key('settings-server-remove-yes'),
                    variant: HButtonVariant.danger,
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
