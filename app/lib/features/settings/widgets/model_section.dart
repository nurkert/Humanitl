/// The model: endpoint, model, access key and the one gesture that contacts
/// the endpoint (HUM-140, in the form of HUM-069).
///
/// # Nothing here goes out on its own
///
/// The endpoint is the one address traffic reaches without passing the queue
/// (`BACKLOG.md` 4.2). Typing contacts nothing; "Check" does, and only it --
/// the same rule as in the setup screen (`core/ui/llm_endpoint_field.dart`).
///
/// # The key is never read back
///
/// The field for the access key starts empty every time and is emptied after
/// every attempt. What the form shows afterwards is the daemon's statement
/// "set, in the system keyring" or "set, in a file" (`LlmKeyStatus`), never
/// the key: the daemon does not send it, and the type has no field for it.
/// "Check" sends a freshly typed key along for that one probe and empties the
/// field afterwards, as storing does; with an empty field it asks the daemon
/// to use the stored one, which it does only for the endpoint the key belongs
/// to. Changing the endpoint empties the field as well: a key typed for one
/// endpoint never goes out to the next one by accident.
///
/// # The fields are the active server's
///
/// Below them stands the list of saved servers (`ServerSwitcher`, HUM-289).
/// Endpoint, model and key above it are the active server's: switching there
/// changes the configuration, and the fields follow it here.
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/pasted_text.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'secret_field.dart';
import 'server_switcher.dart';
import 'setting_row.dart';

/// The model group of the settings form.
class ModelSection extends ConsumerStatefulWidget {
  /// Creates the group for [config] and the key [status].
  const ModelSection({
    required this.config,
    required this.status,
    this.anchors = const <String, FocusNode>{},
    super.key,
  });

  /// The configuration the form was opened with, or last read.
  final ConfigValues config;

  /// What the daemon says about the access key.
  final LlmKeyStatus status;

  /// The anchors of the form, by key, that a `ChangeSetting` fix lands on
  /// (HUM-275); this group places those of the endpoint, the model and the
  /// access key.
  final Map<String, FocusNode> anchors;

  @override
  ConsumerState<ModelSection> createState() => _ModelSectionState();
}

class _ModelSectionState extends ConsumerState<ModelSection> {
  late final TextEditingController _endpoint = TextEditingController(
    text: widget.config.llmEndpoint,
  );
  late final TextEditingController _model = TextEditingController(
    text: widget.config.llmModels.isEmpty ? '' : widget.config.llmModels.first,
  );
  final TextEditingController _key = TextEditingController();

  bool _probing = false;
  LlmProbe? _probe;
  Diagnostic? _probeFailure;
  Diagnostic? _endpointRefusal;
  Diagnostic? _modelRefusal;
  Diagnostic? _keyRefusal;
  bool _keyBusy = false;

  /// The endpoint text the typed key belongs to.
  late String _keyEndpoint = _endpoint.text;

  /// The saved server whose view the fields show, once the list is known
  /// (HUM-289); a write of endpoint or model names it.
  String? _fieldsServer;

  /// What the fields last showed from the daemon; other text is an edit.
  late String _shownEndpoint = _endpoint.text;
  late String _shownModel = _model.text;

  ProviderSubscription<AsyncValue<SavedServers>>? _servers;

  @override
  void initState() {
    super.initState();
    _endpoint.addListener(_dropKeyOnEndpointEdit);
    // Die Felder zeigen die Werte des aktiven Servers, sobald die Liste da
    // ist, und tragen erst dann seinen Namen (HUM-289, Review): Kein Wert
    // eines Servers steht unter dem Namen eines anderen.
    if (ref.read(llmServersProvider).value case final SavedServers list) {
      _syncTo(list);
    }
    _servers = ref.listenManual<AsyncValue<SavedServers>>(llmServersProvider, (
      AsyncValue<SavedServers>? previous,
      AsyncValue<SavedServers> next,
    ) {
      if (next.value case final SavedServers list when mounted) {
        setState(() => _syncTo(list));
      }
    });
  }

  /// Whether the list of saved servers has been answered (or cannot be,
  /// because the daemon is older than HUM-289). Before that the fields take
  /// no input and write nothing: whose view they show is not known yet.
  bool _serversKnown(AsyncValue<SavedServers> servers) =>
      servers.hasValue || servers.hasError;

  /// Nobody has typed since the fields last showed the daemon's values.
  bool get _clean =>
      _endpoint.text == _shownEndpoint && _model.text == _shownModel;

  /// Shows [endpoint] and [model] as the daemon's values.
  void _show(String endpoint, String model) {
    if (_endpoint.text != endpoint) {
      _endpoint.text = endpoint;
      _probe = null;
      _probeFailure = null;
      _endpointRefusal = null;
    }
    if (_model.text != model) {
      _model.text = model;
      _modelRefusal = null;
    }
    _shownEndpoint = endpoint;
    _shownModel = model;
  }

  /// Follows the list of saved servers: while nobody is editing, the fields
  /// show the active server's endpoint and model and carry its name. Text
  /// somebody typed stays, and stays labelled with the server it was typed
  /// for; the daemon refuses it once another one is active.
  void _syncTo(SavedServers list) {
    if (!_clean) {
      return;
    }
    final SavedServer? active = list.activeServer;
    if (active == null) {
      _fieldsServer = null;
      return;
    }
    _fieldsServer = active.name;
    _show(active.endpoint, active.model);
  }

  /// Empties the key field as soon as the endpoint text changes, not only
  /// when it is saved: a key typed for one endpoint never goes out to the
  /// next one, not even through a "Check" before the field loses focus.
  /// Selection changes leave the text alone and the key in place.
  void _dropKeyOnEndpointEdit() {
    if (_endpoint.text == _keyEndpoint) {
      return;
    }
    _keyEndpoint = _endpoint.text;
    _key.clear();
  }

  /// Follows a change of the configuration from elsewhere, such as a switch
  /// of the active server (HUM-289): a field that still shows the old value
  /// shows the new one, and the answer of a check of the old endpoint goes.
  /// A field somebody is editing keeps what they typed.
  @override
  void didUpdateWidget(ModelSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    // With saved servers the list decides what the fields show
    // ([_syncTo]); without, the configuration does.
    final SavedServers? list = ref.read(llmServersProvider).value;
    if (list != null && list.activeServer != null) {
      return;
    }
    final List<String> models = widget.config.llmModels;
    if (_clean) {
      _show(widget.config.llmEndpoint, models.isEmpty ? '' : models.first);
    }
  }

  @override
  void dispose() {
    _servers?.close();
    _endpoint.removeListener(_dropKeyOnEndpointEdit);
    _endpoint.dispose();
    _model.dispose();
    _key.dispose();
    super.dispose();
  }

  /// Writes the endpoint, or removes it for an empty field.
  Future<void> _saveEndpoint() async {
    if (!_serversKnown(ref.read(llmServersProvider))) {
      return;
    }
    // Ohne Leerraum und Anführungszeichen aus der Zwischenablage (HUM-283);
    // das Feld zeigt danach, was geschrieben wird.
    final String text = stripPasteDebris(_endpoint.text);
    if (text != _endpoint.text) {
      _endpoint.text = text;
    }
    if (text == widget.config.llmEndpoint) {
      return;
    }
    await _write(
      FormKeys.llmEndpoint,
      text.isEmpty ? '-' : text,
      (Diagnostic? refusal) => _endpointRefusal = refusal,
    );
  }

  /// Writes [model] as the first entry of `llm.models` and keeps the others:
  /// the first one is the agent's default, the rest stays offered.
  Future<void> _saveModel(String model) async {
    if (!_serversKnown(ref.read(llmServersProvider))) {
      return;
    }
    final String name = model.trim();
    final List<String> others = widget.config.llmModels
        .where((String known) => known != name)
        .toList();
    final List<String> models = <String>[if (name.isNotEmpty) name, ...others];
    if (name.isNotEmpty &&
        widget.config.llmModels.isNotEmpty &&
        widget.config.llmModels.first == name) {
      return;
    }
    _model.text = name;
    await _write(
      FormKeys.llmModels,
      models.isEmpty ? '-' : jsonEncode(models),
      (Diagnostic? refusal) => _modelRefusal = refusal,
    );
  }

  Future<void> _write(
    String key,
    String value,
    void Function(Diagnostic? refusal) report,
  ) async {
    try {
      // Endpunkt und Modell gehören zur Ansicht des Servers, für den sie
      // getippt wurden; ist inzwischen ein anderer aktiv, schreibt der Daemon
      // nichts (HUM-289).
      final String? server = _fieldsServer;
      await ref
          .read(configProvider.notifier)
          .write(
            key,
            value,
            expectedServer: server == null || server.isEmpty ? null : server,
          );
      if (key == FormKeys.llmEndpoint || key == FormKeys.llmModels) {
        // What stands in the fields is the daemon's value now, and the list
        // carries it too.
        _shownEndpoint = _endpoint.text;
        _shownModel = _model.text;
        ref.invalidate(llmServersProvider);
      }
      if (mounted) {
        setState(() => report(null));
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() => report(error.diagnostic));
      }
    }
  }

  /// "Check": the one gesture of this form that reaches the network.
  Future<void> _check() async {
    final String endpoint = stripPasteDebris(_endpoint.text);
    if (endpoint.isEmpty || _probing) {
      return;
    }
    final String typed = stripPasteDebris(_key.text);
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
            useStoredKey: typed.isEmpty && widget.status.set,
          );
      if (mounted) {
        setState(() => _probe = probe);
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() {
          _probe = null;
          _probeFailure = error.diagnostic;
        });
      }
    } finally {
      // Empty after the probe, like after storing: the typed key went out
      // once, for this endpoint, and does not wait in the form for the next
      // gesture. After unmounting the controller is disposed and holds
      // nothing any more.
      if (mounted) {
        _key.clear();
        setState(() => _probing = false);
      }
    }
  }

  /// The server the key buttons act for (HUM-289): the one the form is
  /// bound to. `(true, null)` means no saved server exists, so the flat key
  /// is meant; `(false, _)` means saved servers exist and the form is bound
  /// to none yet, and the buttons do nothing.
  (bool, String?) get _keyServer {
    final SavedServers? list = ref.read(llmServersProvider).value;
    if (list == null || list.implicit || list.servers.isEmpty) {
      return (list != null || ref.read(llmServersProvider).hasError, null);
    }
    final String? bound = _fieldsServer;
    return (bound != null && bound.isNotEmpty, bound);
  }

  Future<void> _storeKey() async {
    // Anführungszeichen und Leerraum aus der Zwischenablage fallen weg, wie
    // im Daemon (`ApiKey::from_pasted`, HUM-283).
    final String typed = stripPasteDebris(_key.text);
    final (bool usable, String? server) = _keyServer;
    if (typed.isEmpty || _keyBusy || !usable) {
      return;
    }
    setState(() => _keyBusy = true);
    try {
      // Der Schlüssel gehört zu dem Server, an den das Formular gebunden
      // ist, nicht zu dem, der inzwischen aktiv sein mag.
      await ref.read(llmKeyProvider.notifier).store(typed, server: server);
      _keyRefusal = null;
    } on DaemonException catch (error) {
      _keyRefusal = error.diagnostic;
    } finally {
      // Leer nach jedem Versuch: Ein Schluessel bleibt nicht im Speicher des
      // Formulars stehen, auch nicht nach einer Absage.
      _key.clear();
      if (mounted) {
        setState(() => _keyBusy = false);
      }
    }
  }

  Future<void> _clearKey() async {
    final (bool usable, String? server) = _keyServer;
    if (!usable) {
      return;
    }
    setState(() => _keyBusy = true);
    try {
      await ref.read(llmKeyProvider.notifier).clear(server: server);
      _keyRefusal = null;
    } on DaemonException catch (error) {
      _keyRefusal = error.diagnostic;
    } finally {
      if (mounted) {
        setState(() => _keyBusy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final bool serversKnown = _serversKnown(ref.watch(llmServersProvider));
    final bool keyUsable = _keyServer.$1;
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final LlmProbe? probe = _probe;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SettingGroupTitle(l10n.settingsSectionModel),
        SettingRow(
          label: l10n.settingsEndpointLabel,
          help: l10n.settingsEndpointHelp,
          refusal: _endpointRefusal,
          refusalKey: const Key('settings-endpoint-refusal'),
          anchor: widget.anchors[FormKeys.llmEndpoint],
          child: Row(
            children: <Widget>[
              Expanded(
                child: Focus(
                  onFocusChange: (bool focused) {
                    if (!focused) {
                      unawaited(_saveEndpoint());
                    }
                  },
                  child: HTextField(
                    key: const Key('settings-endpoint'),
                    controller: _endpoint,
                    enabled: serversKnown,
                    semanticsLabel: l10n.settingsEndpointLabel,
                    hint: l10n.settingsEndpointHint,
                    onSubmitted: (String _) => unawaited(_saveEndpoint()),
                    inputFormatters: const <TextInputFormatter>[
                      PasteDebrisFormatter(),
                    ],
                  ),
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              HButton(
                key: const Key('settings-check'),
                onPressed: _probing ? null : () => unawaited(_check()),
                child: Text(
                  _probing ? l10n.settingsChecking : l10n.settingsCheck,
                ),
              ),
            ],
          ),
        ),
        if (_probeFailure case final Diagnostic failure)
          _ProbeFinding(
            diagnostic: failure,
            findingKey: 'settings-probe-failure',
          ),
        if (probe != null) ..._probeResult(context, probe),
        SettingRow(
          label: l10n.settingsModelLabel,
          refusal: _modelRefusal,
          refusalKey: const Key('settings-model-refusal'),
          anchor: widget.anchors[FormKeys.llmModels],
          child: Focus(
            onFocusChange: (bool focused) {
              if (!focused && _model.text.trim().isNotEmpty) {
                unawaited(_saveModel(_model.text));
              }
            },
            child: HTextField(
              key: const Key('settings-model'),
              controller: _model,
              enabled: serversKnown,
              semanticsLabel: l10n.settingsModelLabel,
              hint: l10n.settingsModelHint,
              onSubmitted: (String text) => unawaited(_saveModel(text)),
            ),
          ),
        ),
        SettingRow(
          label: l10n.settingsKeyLabel,
          help: l10n.settingsKeyHelp,
          refusal: _keyRefusal ?? _firstKeyFinding(widget.status),
          refusalKey: const Key('settings-key-refusal'),
          anchor: widget.anchors[FormKeys.llmApiKeyRef],
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                _keyState(l10n, widget.status),
                key: const Key('settings-key-state'),
                style: tokens.typography.ui12.tinted(
                  widget.status.set
                      ? tokens.stateText.allowed
                      : tokens.colors.fg1,
                ),
              ),
              SizedBox(height: tokens.spacing.x2),
              SecretField(
                controller: _key,
                semanticsLabel: l10n.settingsKeyLabel,
                hint: widget.status.set
                    ? l10n.settingsKeyReplaceHint
                    : l10n.settingsKeyHint,
                enabled: !_keyBusy,
                onSubmitted: (String _) => unawaited(_storeKey()),
              ),
              SizedBox(height: tokens.spacing.x2),
              // Die Knöpfe unter dem Feld in einem `Wrap`: Neben dem Feld
              // liefen „Schlüssel hinterlegen" und „Schlüssel entfernen" im
              // Deutschen über den Rand des Blatts (`docs/UX.md` 6, gefunden
              // von HUM-054).
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                runSpacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    key: const Key('settings-key-store'),
                    onPressed: _keyBusy || !keyUsable
                        ? null
                        : () => unawaited(_storeKey()),
                    child: Text(l10n.settingsKeyStore),
                  ),
                  if (widget.status.set)
                    HButton(
                      key: const Key('settings-key-remove'),
                      variant: HButtonVariant.ghost,
                      onPressed: _keyBusy || !keyUsable
                          ? null
                          : () => unawaited(_clearKey()),
                      child: Text(l10n.settingsKeyRemove),
                    ),
                ],
              ),
            ],
          ),
        ),
        // Die gespeicherten Server unter den Feldern des aktiven (HUM-289):
        // Wer nur einen Endpunkt hat, liest das Formular wie vorher.
        const ServerSwitcher(),
      ],
    );
  }

  /// What a probe answered: the models to pick from, the latency, whether a
  /// key went along, and every finding -- `LLM_006` beside the endpoint, not
  /// in a log (HUM-140).
  List<Widget> _probeResult(BuildContext context, LlmProbe probe) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return <Widget>[
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
                    variant: HButtonVariant.ghost,
                    onPressed: () => unawaited(_saveModel(name)),
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
    ];
  }
}

/// The state line of the access key, never the key.
String _keyState(AppLocalizations l10n, LlmKeyStatus status) {
  if (!status.set) {
    return l10n.settingsKeyNotSet;
  }
  return switch (status.store) {
    LlmKeyStore.file => l10n.settingsKeyStoredFile,
    LlmKeyStore.secretService ||
    LlmKeyStore.none => l10n.settingsKeyStoredKeyring,
  };
}

/// The first finding about the key the daemon reported on its own: the
/// fallback to the file, or a reference without its key (`LLM_009`).
Diagnostic? _firstKeyFinding(LlmKeyStatus status) =>
    status.diagnostics.isEmpty ? null : status.diagnostics.first;

/// One finding of the probe, as a card under the endpoint.
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
