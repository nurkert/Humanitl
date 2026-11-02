/// The detail view of one saved model server, or of a new one (HUM-405,
/// beside the list since HUM-414).
///
/// The view draws the form of [ServerForm] (fields, what they change, the
/// gestures that reach the daemon) as rows: name, address with "Check",
/// model, key; the footer appears once the form holds a change. Below stand
/// "Make default", who uses the server, and its removal; the default itself
/// is not removed while another server could take its place. Every write is
/// one RPC of `LlmServers` or `LlmKey` (ADR-018).
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
import '../../../../core/shortcuts/intents.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/server_checks.dart';
import '../setting_row.dart';
import 'server_fields.dart';
import 'server_footer.dart';
import 'server_form.dart';
import 'server_header.dart';
import 'server_key_field.dart';
import 'server_notes.dart';
import 'server_remove_confirm.dart';

export 'server_form.dart' show serverNamePattern;

/// The id of the name row; no entry of the register.
const String serverNameRowId = 'llm.server_name';

/// The id of the row "Default server"; no entry of the register.
const String serverDefaultRowId = 'llm.server_default';

/// The id of the row that removes the server; no entry of the register.
const String serverRemoveRowId = 'llm.server_remove';

/// The detail view of [server], or of a new server when it is null.
class ServerDetail extends ConsumerStatefulWidget {
  /// Creates the view.
  const ServerDetail({
    required this.list,
    required this.onClose,
    this.server,
    this.onSaved,
    this.onBack,
    this.entryFocus,
    this.leaving = false,
    this.onLeave,
    this.onStay,
    this.onDirtyChanged,
    this.removedElsewhere = false,
    super.key,
  });

  /// The server shown; null while a new one is added.
  final SavedServer? server;

  /// Every saved server with the profiles and the project that use them.
  final SavedServers list;

  /// The view is done: "Cancel" of a new server, a discarded draft of a
  /// removed server, or the server was removed.
  final VoidCallback onClose;

  /// After a save that gave the server its name: a new server, a rename,
  /// or a draft saved as new.
  final ValueChanged<String>? onSaved;

  /// Back to the list, on a narrow page; null when the list stands beside.
  final VoidCallback? onBack;

  /// The node of the first field, where `Enter` in the list lands.
  final FocusNode? entryFocus;

  /// True while another choice waits on the change: the footer asks whether
  /// to save, discard or keep editing.
  final bool leaving;

  /// Go on to the waiting choice, after a save or a discard.
  final VoidCallback? onLeave;

  /// Stay on this server with the change.
  final VoidCallback? onStay;

  /// Told whenever the form starts or stops holding a change.
  final ValueChanged<bool>? onDirtyChanged;

  /// True when [server] is no longer in the list, removed by another client
  /// while this form held a change: the draft stays, with the choice to
  /// discard it or save it as a new server.
  final bool removedElsewhere;

  @override
  ConsumerState<ServerDetail> createState() => _ServerDetailState();
}

/// `Ctrl+S` in the form.
class _SaveIntent extends Intent {
  const _SaveIntent();
}

class _ServerDetailState extends ConsumerState<ServerDetail> {
  late final ServerForm _form = ServerForm(
    server: widget.server,
    list: widget.list,
  )..addListener(_changed);

  /// The footer, brought into view when it asks.
  final GlobalKey _footerKey = GlobalKey(debugLabel: 'server-footer');

  /// "Keep editing", the preselected answer when another choice waits.
  final FocusNode _keepEditing = FocusNode(debugLabel: 'server-keep-editing');

  bool _removing = false;
  bool? _toldDirty;

  ServerFormDeps get _deps => ServerFormDeps(
    client: ref.read(daemonClientProvider),
    servers: ref.read(llmServersProvider.notifier),
    keys: ref.read(llmKeyProvider.notifier),
    checks: ref.read(serverChecksProvider.notifier),
  );

  @override
  void initState() {
    super.initState();
    unawaited(_form.loadKeyStatus(ref.read(daemonClientProvider)));
  }

  @override
  void didUpdateWidget(ServerDetail oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.leaving && !oldWidget.leaving) {
      // Die Frage steht am Ende des Formulars, oft unter dem sichtbaren Teil
      // der Seite; sie kommt ins Bild, und ihre vorgewählte Antwort nimmt
      // die Tastatur.
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        final BuildContext? footer = _footerKey.currentContext;
        if (!mounted || footer == null) {
          return;
        }
        unawaited(Scrollable.ensureVisible(footer, alignment: 0.5));
        _keepEditing.requestFocus();
      });
    }
    _form.update(widget.server, widget.list);
  }

  void _changed() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void dispose() {
    _keepEditing.dispose();
    _form
      ..removeListener(_changed)
      ..dispose();
    super.dispose();
  }

  Future<bool> _save() async {
    final String? name = await _form.save(
      _deps,
      context.l10n,
      onRenamed: widget.onSaved,
    );
    if (name == null) {
      return false;
    }
    if (_form.isNew || name != widget.server?.name) {
      widget.onSaved?.call(name);
    } else if (mounted) {
      unawaited(_form.loadKeyStatus(ref.read(daemonClientProvider)));
    }
    return true;
  }

  Future<void> _saveAndLeave() async {
    if (await _save() && mounted) {
      widget.onLeave?.call();
    }
  }

  Future<void> _saveAsNew() async {
    final String? name = await _form.saveAsNew(_deps, context.l10n);
    if (name != null) {
      widget.onSaved?.call(name);
    }
  }

  void _cancel() {
    if (_form.isNew) {
      widget.onClose();
    } else {
      _form.cancel();
    }
  }

  void _makeDefault(String name) => unawaited(
    _form.run(() async => ref.read(llmServersProvider.notifier).activate(name)),
  );

  Future<void> _remove(String name, bool deleteKey) async {
    final bool done = await _form.run(
      () async => ref
          .read(llmServersProvider.notifier)
          .remove(name, deleteKey: deleteKey),
    );
    if (done) {
      widget.onClose();
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServer? server = widget.server;
    final bool knownKey = _form.savedWithoutKey != null;
    final bool dirty = _form.dirty;
    if (_toldDirty != dirty) {
      _toldDirty = dirty;
      widget.onDirtyChanged?.call(dirty);
    }
    final bool removed = widget.removedElsewhere;
    final String? nameProblem = _form.nameProblem(l10n, removed: removed);
    return Shortcuts(
      shortcuts: withCommandOnMac(const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.keyS, control: true): _SaveIntent(),
      }),
      child: Actions(
        actions: <Type, Action<Intent>>{
          _SaveIntent: CallbackAction<_SaveIntent>(
            onInvoke: (_SaveIntent _) {
              unawaited(removed ? _saveAsNew() : _save());
              return null;
            },
          ),
        },
        child: Column(
          key: ValueKey<String>(
            'settings-server-detail-${server?.name ?? '+'}',
          ),
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            ServerHeader(
              server: server,
              list: widget.list,
              onBack: widget.onBack,
              removedElsewhere: removed,
            ),
            if (removed)
              Padding(
                padding: EdgeInsets.fromLTRB(
                  SettingRowGrid.inset,
                  tokens.spacing.x3,
                  SettingRowGrid.inset,
                  0,
                ),
                child: Semantics(
                  liveRegion: true,
                  child: ServerNote(
                    key: const Key('settings-server-removed-elsewhere'),
                    glyph: HGlyph.triangleAlert,
                    color: tokens.colors.warning,
                    textColor: tokens.colors.warningText,
                    text: l10n.settingsServerRemovedElsewhere,
                  ),
                ),
              ),
            SizedBox(height: tokens.spacing.x2),
            SettingRow(
              settingId: serverNameRowId,
              title: l10n.settingsServerNameLabel,
              help: l10n.settingsServerNameHelp,
              fill: true,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  HTextField(
                    key: const Key('settings-server-name'),
                    controller: _form.name,
                    focusNode: widget.entryFocus,
                    semanticsLabel: l10n.settingsServerNameLabel,
                    hint: l10n.settingsServerNameHint,
                    autofocus: server == null,
                    enabled: !knownKey && !_form.busy,
                  ),
                  if (nameProblem != null)
                    Padding(
                      padding: EdgeInsets.only(top: tokens.spacing.x1),
                      child: Semantics(
                        liveRegion: true,
                        child: Text(
                          nameProblem,
                          key: const Key('settings-server-name-error'),
                          style: tokens.typography.ui12.tinted(
                            tokens.stateText.error,
                          ),
                        ),
                      ),
                    ),
                ],
              ),
            ),
            const HHairline(),
            SettingRow(
              settingId: FormKeys.llmEndpoint,
              help: l10n.settingsServerAddressHelp,
              fill: true,
              child: ServerAddressField(
                form: _form,
                onCheck: () => _form.check(_deps),
              ),
            ),
            if (_form.probeFailure case final Diagnostic failure)
              ServerInset(
                child: ProbeFinding(
                  diagnostic: failure,
                  findingKey: 'settings-probe-failure',
                ),
              ),
            if (_form.probe case final LlmProbe probe)
              ServerInset(child: ProbeResult(probe: probe)),
            const HHairline(),
            SettingRow(
              settingId: FormKeys.llmModels,
              help: l10n.settingsServerModelsHelp,
              fill: true,
              child: ServerModelField(form: _form),
            ),
            const HHairline(),
            SettingRow(
              settingId: FormKeys.llmApiKeyRef,
              help: l10n.settingsServerKeyHelp,
              refusal: _form.keyStatus?.diagnostics.firstOrNull,
              refusalKey: const Key('settings-key-refusal'),
              fill: true,
              child: ServerKeyField(form: _form),
            ),
            if (dirty ||
                server == null ||
                widget.leaving ||
                _form.refusal != null ||
                removed)
              ServerFooter(
                key: _footerKey,
                form: _form,
                mode: removed
                    ? ServerFooterMode.removed
                    : widget.leaving
                    ? ServerFooterMode.leaving
                    : ServerFooterMode.edit,
                onSave: () => unawaited(_save()),
                onCancel: _cancel,
                onSaveAndLeave: () => unawaited(_saveAndLeave()),
                onDiscard: removed ? widget.onClose : widget.onLeave,
                onStay: widget.onStay,
                onSaveAsNew: () => unawaited(_saveAsNew()),
                keepEditing: _keepEditing,
              ),
            if (server != null && !removed) ...<Widget>[
              SizedBox(height: tokens.spacing.x4),
              const HHairline(),
              _defaultRow(context, server),
              const HHairline(),
              _removeRow(context, server),
            ],
          ],
        ),
      ),
    );
  }

  /// "Make default", or that it is the default, with what that means.
  Widget _defaultRow(BuildContext context, SavedServer server) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return SettingRow(
      settingId: serverDefaultRowId,
      title: l10n.settingsServerDefaultRowTitle,
      help: l10n.settingsServerDefaultExplain,
      child: HButton(
        key: const Key('settings-server-make-default'),
        leading: HGlyphIcon(
          HGlyph.star,
          size: 12,
          color: server.active ? tokens.colors.accent : tokens.colors.fg1,
        ),
        onPressed: server.active || _form.busy
            ? null
            : () => _makeDefault(server.name),
        child: Text(
          server.active
              ? l10n.settingsServerIsDefault
              : l10n.settingsServerMakeDefault,
        ),
      ),
    );
  }

  /// The removal at the end of the view, with its question below it.
  Widget _removeRow(BuildContext context, SavedServer server) {
    final AppLocalizations l10n = context.l10n;
    // Der Standard geht erst, wenn ein anderer ihn ablöst; ist er der
    // letzte, darf er gehen, und danach ist kein Server eingetragen.
    final bool blocked = server.active && widget.list.servers.length > 1;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SettingRow(
          settingId: serverRemoveRowId,
          title: l10n.settingsServerRemoveRowTitle,
          help: blocked
              ? l10n.settingsServerRemoveDefaultBlocked
              : l10n.settingsServerRemoveHelp,
          child: HButton(
            key: const Key('settings-server-remove'),
            variant: HButtonVariant.danger,
            onPressed: blocked || _form.busy || _removing
                ? null
                : () => setState(() => _removing = true),
            child: Text(l10n.settingsServerRemoveConfirm),
          ),
        ),
        if (_removing)
          ServerRemoveConfirm(
            server: server,
            keyStored: _form.keyStored,
            users: serverUsers(l10n, widget.list, server),
            busy: _form.busy,
            onConfirm: (bool deleteKey) =>
                unawaited(_remove(server.name, deleteKey)),
            onCancel: () => setState(() => _removing = false),
          ),
      ],
    );
  }
}
