/// The settings form: the few settings a person needs in the MVP, written
/// through the daemon (HUM-069 with HUM-140).
///
/// A hand-built form and not the screen generated from the schema: the second
/// scope decision of 2026-09-18 (`backlog/sprint-4.md`) moves the generator,
/// search, tiers, origins, reset per field and live reload behind the MVP. The
/// form shows `hold.timeout_secs`, `hold.ask_mode`, `ui.language`, `ui.theme`,
/// `ui.notifications`, `sandbox.work_dir`, the endpoint, the model, the
/// access key and `tls.trust_system_store` (HUM-257); everything else stays
/// in `config.toml`, one click away.
///
/// Every value comes from `GetConfig` and goes back through `SetConfig` and
/// `LlmKey`; the form decides nothing about what a value means (ADR-018). A
/// change takes effect when the daemon starts next, and the form says so.
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/config_providers.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/ui.dart';
import '../../core/ui/work_dir_picker.dart' show directoryChooserProvider;
import '../../l10n/l10n.dart';
import 'providers/file_opener.dart';
import 'widgets/model_section.dart';
import 'widgets/setting_row.dart';

/// The settings, as a sheet from the right.
class SettingsSheet extends ConsumerWidget {
  /// Creates the sheet; [onClose] closes it.
  const SettingsSheet({
    required this.onClose,
    this.focusKey,
    this.focusSerial = 0,
    super.key,
  });

  /// Closes the sheet.
  final VoidCallback onClose;

  /// The configuration key whose field the sheet scrolls to and focuses, or
  /// null (HUM-275). A diagnostic's `ChangeSetting` fix opens the sheet with
  /// it; one of [FormKeys.shownInSheet].
  final String? focusKey;

  /// Counts the requests for [focusKey]. A second click on the same fix asks
  /// for the same key again and lands on the field again, even if the focus
  /// moved away in between.
  final int focusSerial;

  /// Wide enough for an endpoint and its button side by side.
  static const double sheetWidth = 520;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final AsyncValue<ConfigValues> config = ref.watch(configProvider);
    final LlmKeyStatus status =
        ref.watch(llmKeyProvider).value ?? LlmKeyStatus.unset;
    final ConfigValues? values = config.value;
    return HSheet(
      key: const Key('settings-sheet'),
      width: sheetWidth,
      title: Text(l10n.settingsTitle),
      closeSemanticsLabel: l10n.settingsClose,
      onClose: onClose,
      child: values == null
          ? Padding(
              padding: EdgeInsets.all(tokens.spacing.x3),
              child: Text(
                config.hasError
                    ? l10n.settingsLoadFailed
                    : l10n.settingsLoading,
                key: const Key('settings-loading'),
                style: tokens.typography.ui13.tinted(tokens.colors.fg1),
              ),
            )
          : FocusTraversalGroup(
              child: SingleChildScrollView(
                padding: EdgeInsets.all(tokens.spacing.x3),
                child: _SettingsForm(
                  config: values,
                  status: status,
                  focusKey: focusKey,
                  focusSerial: focusSerial,
                ),
              ),
            ),
    );
  }
}

/// The groups of the form.
class _SettingsForm extends ConsumerStatefulWidget {
  const _SettingsForm({
    required this.config,
    required this.status,
    required this.focusKey,
    required this.focusSerial,
  });

  final ConfigValues config;
  final LlmKeyStatus status;
  final String? focusKey;
  final int focusSerial;

  @override
  ConsumerState<_SettingsForm> createState() => _SettingsFormState();
}

class _SettingsFormState extends ConsumerState<_SettingsForm> {
  late final TextEditingController _timeout = TextEditingController(
    text: '${widget.config.holdTimeoutSecs}',
  );

  /// The daemon's refusal of the last write, by key.
  final Map<String, Diagnostic> _refusals = <String, Diagnostic>{};

  bool _openFailed = false;

  /// One anchor per key the sheet shows, around the control of its row
  /// (HUM-275). The anchor itself takes no focus; [_reveal] scrolls to it and
  /// focuses the first control inside it.
  final Map<String, FocusNode> _anchors = <String, FocusNode>{
    for (final String key in FormKeys.shownInSheet)
      key: FocusNode(
        debugLabel: 'setting-anchor-$key',
        canRequestFocus: false,
        skipTraversal: true,
      ),
  };

  @override
  void initState() {
    super.initState();
    _reveal(widget.focusKey);
  }

  @override
  void didUpdateWidget(_SettingsForm oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.focusKey != oldWidget.focusKey ||
        widget.focusSerial != oldWidget.focusSerial) {
      _reveal(widget.focusKey);
    }
  }

  @override
  void dispose() {
    _timeout.dispose();
    for (final FocusNode anchor in _anchors.values) {
      anchor.dispose();
    }
    super.dispose();
  }

  /// Scrolls to the field of [key] and focuses it, after the frame that
  /// builds it. Without this the fix of `LLM_002` would open a sheet whose
  /// access-key field lies below the fold, and the person would still have to
  /// look for it (HUM-275).
  void _reveal(String? key) {
    final FocusNode? anchor = key == null ? null : _anchors[key];
    if (anchor == null) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      final BuildContext? target = anchor.context;
      if (!mounted || target == null) {
        return;
      }
      // Ohne Dauer: Der Sprung ist die Antwort auf einen Klick, keine
      // Bewegung, die etwas erklärt (`docs/UX.md` 2.1).
      unawaited(Scrollable.ensureVisible(target, alignment: 0.1));
      anchor.traversalDescendants.firstOrNull?.requestFocus();
    });
  }

  Future<void> _write(String key, String value) async {
    try {
      await ref.read(configProvider.notifier).write(key, value);
      if (mounted) {
        setState(() => _refusals.remove(key));
      }
    } on DaemonException catch (error) {
      if (mounted) {
        setState(() => _refusals[key] = error.diagnostic);
      }
    }
  }

  Future<void> _saveTimeout() async {
    final String text = _timeout.text.trim();
    if (text.isEmpty || text == '${widget.config.holdTimeoutSecs}') {
      return;
    }
    await _write(FormKeys.holdTimeoutSecs, text);
  }

  Future<void> _chooseWorkDir() async {
    final String? dir = await ref.read(directoryChooserProvider)();
    if (dir != null) {
      await _write(FormKeys.sandboxWorkDir, dir);
    }
  }

  Future<void> _openFile() async {
    final bool opened = await ref.read(fileOpenerProvider)(
      widget.config.configPath,
    );
    if (mounted) {
      setState(() => _openFailed = !opened);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final ConfigValues config = widget.config;
    final String workDir = config.sandboxWorkDir;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.settingsRestartNote,
          key: const Key('settings-restart-note'),
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
        ModelSection(config: config, status: widget.status, anchors: _anchors),
        SettingGroupTitle(l10n.settingsSectionHolding),
        SettingRow(
          label: l10n.settingsTimeoutLabel,
          help: l10n.settingsTimeoutHelp,
          refusal: _refusals[FormKeys.holdTimeoutSecs],
          refusalKey: const Key('settings-timeout-refusal'),
          anchor: _anchors[FormKeys.holdTimeoutSecs],
          child: Focus(
            onFocusChange: (bool focused) {
              if (!focused) {
                unawaited(_saveTimeout());
              }
            },
            child: HTextField(
              key: const Key('settings-timeout'),
              controller: _timeout,
              semanticsLabel: l10n.settingsTimeoutLabel,
              digitsOnly: true,
              onSubmitted: (String _) => unawaited(_saveTimeout()),
            ),
          ),
        ),
        SettingRow(
          label: l10n.settingsAskModeLabel,
          help: l10n.settingsAskModeHelp,
          refusal: _refusals[FormKeys.holdAskMode],
          anchor: _anchors[FormKeys.holdAskMode],
          child: Align(
            alignment: Alignment.centerLeft,
            child: HSegmented<String>(
              key: const Key('settings-ask-mode'),
              options: <HSegmentOption<String>>[
                HSegmentOption<String>(
                  value: 'ui',
                  label: l10n.settingsAskModeUi,
                ),
                HSegmentOption<String>(
                  value: 'terminal',
                  label: l10n.settingsAskModeTerminal,
                ),
                HSegmentOption<String>(
                  value: 'none',
                  label: l10n.settingsAskModeNone,
                ),
              ],
              selected: config.holdAskMode,
              onSelect: (String mode) =>
                  unawaited(_write(FormKeys.holdAskMode, mode)),
            ),
          ),
        ),
        SettingGroupTitle(l10n.settingsSectionInterface),
        SettingRow(
          label: l10n.settingsLanguageLabel,
          refusal: _refusals[FormKeys.uiLanguage],
          anchor: _anchors[FormKeys.uiLanguage],
          child: Align(
            alignment: Alignment.centerLeft,
            child: HSegmented<String>(
              key: const Key('settings-language'),
              options: <HSegmentOption<String>>[
                for (final AppLanguage language in AppLanguage.values)
                  HSegmentOption<String>(
                    value: language.name,
                    label: language.endonym(l10n),
                  ),
              ],
              selected: config.uiLanguage,
              onSelect: (String language) =>
                  unawaited(_write(FormKeys.uiLanguage, language)),
            ),
          ),
        ),
        SettingRow(
          label: l10n.settingsThemeLabel,
          refusal: _refusals[FormKeys.uiTheme],
          anchor: _anchors[FormKeys.uiTheme],
          child: Align(
            alignment: Alignment.centerLeft,
            child: HSegmented<String>(
              key: const Key('settings-theme'),
              options: <HSegmentOption<String>>[
                HSegmentOption<String>(
                  value: 'dark',
                  label: l10n.settingsThemeDark,
                ),
                HSegmentOption<String>(
                  value: 'light',
                  label: l10n.settingsThemeLight,
                ),
                HSegmentOption<String>(
                  value: 'system',
                  label: l10n.settingsThemeSystem,
                ),
              ],
              selected: config.uiTheme,
              onSelect: (String theme) =>
                  unawaited(_write(FormKeys.uiTheme, theme)),
            ),
          ),
        ),
        SettingRow(
          label: l10n.settingsNotificationsLabel,
          refusal: _refusals[FormKeys.uiNotifications],
          anchor: _anchors[FormKeys.uiNotifications],
          child: HCheckbox(
            key: const Key('settings-notifications'),
            label: l10n.settingsNotificationsHint,
            value: config.uiNotifications,
            onChanged: (bool on) =>
                unawaited(_write(FormKeys.uiNotifications, '$on')),
          ),
        ),
        SettingGroupTitle(l10n.settingsSectionProject),
        SettingRow(
          label: l10n.settingsWorkDirLabel,
          help: l10n.settingsWorkDirHelp,
          refusal: _refusals[FormKeys.sandboxWorkDir],
          refusalKey: const Key('settings-workdir-refusal'),
          anchor: _anchors[FormKeys.sandboxWorkDir],
          // Der Pfad auf einer eigenen Zeile, die Knöpfe darunter in einem
          // `Wrap`: Nebeneinander liefen „Ordner wählen" und „Startordner
          // verwenden" im Deutschen und bei doppelter Schrift über den Rand
          // des Blatts (`docs/UX.md` 6, gefunden von HUM-054).
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                workDir.isEmpty ? l10n.settingsWorkDirNone : workDir,
                key: const Key('settings-workdir'),
                style: tokens.typography.mono12.tinted(
                  workDir.isEmpty ? tokens.colors.fg2 : tokens.colors.fg0,
                ),
                overflow: TextOverflow.ellipsis,
              ),
              SizedBox(height: tokens.spacing.x2),
              Wrap(
                alignment: WrapAlignment.end,
                spacing: tokens.spacing.x2,
                runSpacing: tokens.spacing.x2,
                children: <Widget>[
                  HButton(
                    key: const Key('settings-workdir-choose'),
                    onPressed: () => unawaited(_chooseWorkDir()),
                    child: Text(l10n.settingsWorkDirChoose),
                  ),
                  if (workDir.isNotEmpty)
                    HButton(
                      key: const Key('settings-workdir-reset'),
                      variant: HButtonVariant.ghost,
                      onPressed: () =>
                          unawaited(_write(FormKeys.sandboxWorkDir, '-')),
                      child: Text(l10n.settingsWorkDirReset),
                    ),
                ],
              ),
            ],
          ),
        ),
        // Das Opt-out des Vertrauens in die Zertifikatsablage des Systems
        // (HUM-257). Vorgabe an; was aus bedeutet, steht in der Hilfe.
        SettingGroupTitle(l10n.settingsSectionConnections),
        SettingRow(
          label: l10n.settingsTrustSystemStoreLabel,
          help: l10n.settingsTrustSystemStoreHelp,
          refusal: _refusals[FormKeys.tlsTrustSystemStore],
          refusalKey: const Key('settings-trust-system-store-refusal'),
          anchor: _anchors[FormKeys.tlsTrustSystemStore],
          child: HCheckbox(
            key: const Key('settings-trust-system-store'),
            label: l10n.settingsTrustSystemStoreHint,
            value: config.tlsTrustSystemStore,
            onChanged: (bool on) =>
                unawaited(_write(FormKeys.tlsTrustSystemStore, '$on')),
          ),
        ),
        SettingGroupTitle(l10n.settingsSectionFile),
        SettingRow(
          label: l10n.settingsFileLabel,
          help: l10n.settingsFileHelp,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      config.configPath,
                      key: const Key('settings-config-path'),
                      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('settings-copy-path'),
                    variant: HButtonVariant.ghost,
                    onPressed: config.configPath.isEmpty
                        ? null
                        : () => unawaited(
                            Clipboard.setData(
                              ClipboardData(text: config.configPath),
                            ),
                          ),
                    child: Text(l10n.settingsFileCopy),
                  ),
                  SizedBox(width: tokens.spacing.x2),
                  HButton(
                    key: const Key('settings-open-file'),
                    onPressed: config.configPath.isEmpty
                        ? null
                        : () => unawaited(_openFile()),
                    child: Text(l10n.settingsFileOpen),
                  ),
                ],
              ),
              if (_openFailed)
                Text(
                  l10n.settingsFileOpenFailed,
                  key: const Key('settings-open-failed'),
                  style: tokens.typography.ui12.tinted(tokens.stateText.error),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
