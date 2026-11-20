/// One card of the section Agents (HUM-521): source, version, login, size,
/// the write-back switch, the findings, the last run, and the buttons.
///
/// The card decides nothing itself (ADR-018). Whether a button is open comes
/// from `AgentDepot.actions`; a company lock comes from `locked_by` and
/// `actions.source_host`; the run, its URLs and the callback field come from
/// `AgentDepot.maintenance`. Every operation answers with the state
/// afterwards, and that is what the card then shows.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/text/format.dart';
import '../../../../core/ui/ui.dart';
import '../../../../core/ui/untranslated_wording.dart';
import '../../../../l10n/l10n.dart';
import '../../providers/agents_provider.dart';
import '../../settings_scope.dart';
import '../setting_controls.dart';
import '../setting_row.dart';
import 'agent_config_files.dart';
import 'agent_inset.dart';
import 'agent_remove_confirm.dart';
import 'agent_run_panel.dart';
import 'agent_version_picker.dart';

/// The question or choice open under the buttons of a card.
enum _Open { none, install, update, remove }

/// The card of [entry].
class AgentCard extends ConsumerStatefulWidget {
  /// Creates the card.
  const AgentCard({
    required this.entry,
    required this.config,
    this.onOpenSession,
    super.key,
  });

  /// The agent as `Agents` answered.
  final AgentDepot entry;

  /// The configuration, for `agents.<id>.credential_writeback`.
  final ConfigValues config;

  /// Shows the terminal of the running session; null without a shell.
  final VoidCallback? onOpenSession;

  @override
  ConsumerState<AgentCard> createState() => _AgentCardState();
}

class _AgentCardState extends ConsumerState<AgentCard> {
  _Open _open = _Open.none;
  bool _busy = false;
  Diagnostic? _refusal;
  List<AgentConfigChange>? _configFiles;

  AgentDepot get _entry => widget.entry;

  String get _writebackKey => 'agents.${_entry.agent}.credential_writeback';

  /// Sends [op]; true when the daemon took it. A refusal stands at the card.
  Future<bool> _send(AgentsOp op) async {
    setState(() {
      _busy = true;
      _refusal = null;
    });
    final (AgentsStatus? status, Diagnostic? refusal) = await ref
        .read(agentsProvider.notifier)
        .run(op);
    if (!mounted) {
      return status != null;
    }
    setState(() {
      _busy = false;
      _refusal = refusal;
      if (status != null && op.kind == AgentsOpKind.configShow) {
        _configFiles = status.of(_entry.agent)?.configFiles;
      }
      if (status != null && op.kind != AgentsOpKind.configShow) {
        _open = _Open.none;
      }
    });
    return status != null;
  }

  /// Starts a maintenance run. Install, update and configuration show their
  /// terminal like any session; a login stays here, where its URLs and the
  /// callback field are.
  Future<void> _maintain(AgentsOp op) async {
    final bool taken = await _send(op);
    final AgentMaintenance? run = ref
        .read(agentsProvider)
        .value
        ?.of(_entry.agent)
        ?.maintenance;
    if (taken &&
        mounted &&
        op.kind != AgentsOpKind.login &&
        run != null &&
        run.running &&
        run.sandboxId.isNotEmpty) {
      widget.onOpenSession?.call();
    }
  }

  void _toggle(_Open open) => setState(() {
    _open = _open == open ? _Open.none : open;
    _refusal = null;
  });

  Future<void> _toggleConfig() async {
    if (_configFiles != null) {
      setState(() => _configFiles = null);
      return;
    }
    await _send(AgentsOp.configShow(_entry.agent));
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentDepot entry = _entry;
    final AgentMaintenance? run = entry.maintenance;
    final Diagnostic? refusal =
        _refusal ?? SettingsScope.maybeOf(context)?.refusal(_writebackKey);
    final List<AgentConfigChange>? files = _configFiles;
    final AgentActions actions = entry.actions;
    // Was der Daemon für diesen Agenten annimmt, steht da, auch ohne Rezept
    // (Review HUM-521): ein vorhandenes Depot, seine Konfiguration, und der
    // Schlüssel der Rückschrift, der sich immer setzen lässt.
    final bool hasDepot = entry.installed || entry.sizeBytes > 0;
    final bool buttons =
        entry.depotSupported || actions.remove || actions.configShow;
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg2,
        border: Border.all(color: tokens.colors.line),
        borderRadius: BorderRadius.circular(tokens.radii.card),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            _Header(
              entry: entry,
              busy: _busy,
              onSource: (String source) =>
                  unawaited(_send(AgentsOp.setSource(entry.agent, source))),
            ),
            _gap(tokens),
            if (entry.depotSupported || hasDepot) ...<Widget>[
              _Facts(entry: entry),
              SizedBox(height: tokens.spacing.x3),
            ],
            _WritebackRow(
              agent: entry.agent,
              value: entry.credentialWriteback,
              enabled: !_busy && !(run?.running ?? false),
              onChanged: (bool on) =>
                  writeSetting(context, _writebackKey, '$on'),
            ),
            for (final Diagnostic found in entry.diagnostics) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              AgentFinding(diagnostic: found),
            ],
            if (run != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              AgentRunPanel(
                entry: entry,
                run: run,
                busy: _busy,
                onSend: _send,
                onOpenSession: widget.onOpenSession,
              ),
            ],
            if (files != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              AgentInset(
                key: ValueKey<String>('agent-config-view-${entry.agent}'),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    AgentInsetTitle(l10n.settingsAgentsConfigFiles),
                    SizedBox(height: tokens.spacing.x2),
                    if (files.isEmpty)
                      Text(
                        l10n.settingsAgentsConfigEmpty,
                        style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                      )
                    else
                      AgentConfigFiles(
                        agent: entry.agent,
                        files: files,
                        showChange: false,
                      ),
                  ],
                ),
              ),
            ],
            if (refusal != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x2),
              DiagnosticCause(
                diagnostic: refusal,
                textKey: ValueKey<String>('agent-refusal-${entry.agent}'),
                style: tokens.typography.ui12.tinted(tokens.stateText.error),
              ),
            ],
            if (buttons) ...<Widget>[
              _gap(tokens),
              _Buttons(
                entry: entry,
                busy: _busy,
                configShown: files != null,
                onInstall: () => _toggle(_Open.install),
                onUpdate: () => _toggle(_Open.update),
                onLogin: () =>
                    unawaited(_maintain(AgentsOp.login(entry.agent))),
                onConfigure: () =>
                    unawaited(_maintain(AgentsOp.config(entry.agent))),
                onShowConfig: () => unawaited(_toggleConfig()),
                onRemove: () => _toggle(_Open.remove),
              ),
            ],
            ..._question(tokens),
          ],
        ),
      ),
    );
  }

  /// The hairline between the parts of a card, with its room.
  Widget _gap(HTokens tokens) => Padding(
    padding: EdgeInsets.symmetric(vertical: tokens.spacing.x3),
    child: const HHairline(),
  );

  List<Widget> _question(HTokens tokens) {
    final AgentDepot entry = _entry;
    final Widget? open = switch (_open) {
      _Open.none => null,
      _Open.install || _Open.update => AgentVersionPicker(
        entry: entry,
        update: _open == _Open.update,
        busy: _busy,
        onStart: (String version) => unawaited(
          _maintain(
            _open == _Open.update
                ? AgentsOp.update(entry.agent, version: version)
                : AgentsOp.install(entry.agent, version: version),
          ),
        ),
        onCancel: () => _toggle(_open),
      ),
      _Open.remove => AgentRemoveConfirm(
        entry: entry,
        busy: _busy,
        onConfirm: () => unawaited(_send(AgentsOp.remove(entry.agent))),
        onCancel: () => _toggle(_Open.remove),
      ),
    };
    return <Widget>[
      if (open != null) ...<Widget>[SizedBox(height: tokens.spacing.x3), open],
    ];
  }
}

/// Name, the source switch, and what the source means; under a company
/// lock the switch is read only and the policy is named.
class _Header extends StatelessWidget {
  const _Header({
    required this.entry,
    required this.busy,
    required this.onSource,
  });

  final AgentDepot entry;
  final bool busy;
  final ValueChanged<String> onSource;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String agent = entry.agent;
    // Die Sperre sagt der Daemon: `locked_by` und `actions.source_host`
    // (HUM-521 Punkt 3). Die Karte rechnet sie nicht nach.
    final bool locked = entry.locked || !entry.actions.sourceHost;
    final Widget toggle = Semantics(
      label: l10n.settingsAgentsSource(entry.displayName),
      child: HSegmented<String>(
        key: ValueKey<String>('agent-source-$agent'),
        enabled: !busy,
        options: <HSegmentOption<String>>[
          HSegmentOption<String>(
            value: AgentSource.host,
            label: l10n.settingsAgentsSourceHost,
            enabled: !locked,
          ),
          HSegmentOption<String>(
            value: AgentSource.depot,
            label: l10n.settingsAgentsSourceDepot,
            enabled: entry.actions.sourceDepot,
          ),
        ],
        selected: entry.source,
        onSelect: (String source) {
          if (source != entry.source) {
            onSource(source);
          }
        },
      ),
    );
    final String meaning = !entry.depotSupported
        ? (entry.source == AgentSource.depot
              ? l10n.settingsAgentsNoRecipeDepot
              : l10n.settingsAgentsNoRecipe)
        : (entry.source == AgentSource.depot
              ? l10n.settingsAgentsFromDepot
              : l10n.settingsAgentsFromHost);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  ConstrainedBox(
                    constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
                    child: Align(
                      alignment: AlignmentDirectional.centerStart,
                      widthFactor: 1,
                      child: Text(
                        entry.displayName,
                        style: tokens.typography.ui14.semibold.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                    ),
                  ),
                  Text(
                    meaning,
                    key: ValueKey<String>('agent-meaning-$agent'),
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
                ],
              ),
            ),
            SizedBox(width: SettingRowGrid.gutter),
            if (locked)
              ReadOnlySetting(settingId: 'agents.$agent.source', child: toggle)
            else
              toggle,
          ],
        ),
        if (entry.locked) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Row(
            children: <Widget>[
              HGlyphIcon(HGlyph.lock, size: 12, color: tokens.colors.fg1),
              SizedBox(width: tokens.spacing.x1),
              Flexible(
                child: Text(
                  l10n.settingsAgentsLocked(entry.lockedBy),
                  key: ValueKey<String>('agent-lock-$agent'),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
              ),
            ],
          ),
        ],
      ],
    );
  }
}

/// Version and signature, login and last write-back, size: one label
/// column, one value column.
class _Facts extends StatelessWidget {
  const _Facts({required this.entry});

  final AgentDepot entry;

  /// The width of the label column.
  static const double labelWidth = 112;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String agent = entry.agent;
    final TextStyle value = tokens.typography.ui13.tinted(tokens.colors.fg0);
    final TextStyle quiet = tokens.typography.ui13.tinted(tokens.colors.fg1);
    final TextStyle note = tokens.typography.ui12.tinted(tokens.colors.fg2);
    // Name und Wert auf einer Grundlinie, bei jeder Textgröße. Die Zeile der
    // Version steht auf der Mitte: Neben dem Badge der Signatur stünde der
    // Name sonst zu hoch.
    Widget fact(String label, Widget content, {bool middle = false}) => Padding(
      padding: EdgeInsets.symmetric(vertical: tokens.spacing.x1),
      child: Row(
        crossAxisAlignment: middle
            ? CrossAxisAlignment.center
            : CrossAxisAlignment.baseline,
        textBaseline: TextBaseline.alphabetic,
        children: <Widget>[
          SizedBox(
            width: labelWidth,
            child: Text(
              label,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
          ),
          Expanded(child: content),
        ],
      ),
    );
    final bool verified = entry.signatureVerified;
    return Column(
      key: ValueKey<String>('agent-facts-$agent'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        fact(
          l10n.settingsAgentsVersion,
          entry.installed
              ? Row(
                  children: <Widget>[
                    Text(
                      entry.version,
                      key: ValueKey<String>('agent-version-$agent'),
                      style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                    ),
                    SizedBox(width: tokens.spacing.x2),
                    HBadge(
                      key: ValueKey<String>('agent-signature-$agent'),
                      text: verified
                          ? l10n.settingsAgentsSignatureVerified
                          : l10n.settingsAgentsSignatureNone,
                      color: verified
                          ? tokens.state.allowed
                          : tokens.colors.warning,
                    ),
                  ],
                )
              : Text(
                  l10n.settingsAgentsNotInstalled,
                  key: ValueKey<String>('agent-version-$agent'),
                  style: quiet,
                ),
          middle: entry.installed,
        ),
        fact(
          l10n.settingsAgentsLogin,
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                switch ((entry.loggedIn, entry.credentialsEncrypted)) {
                  (true, true) => l10n.settingsAgentsLoggedInEncrypted,
                  (true, false) => l10n.settingsAgentsLoggedIn,
                  (false, _) => l10n.settingsAgentsLoggedOut,
                },
                key: ValueKey<String>('agent-login-$agent'),
                style: entry.loggedIn ? value : quiet,
              ),
              if (entry.lastWriteback.isNotEmpty)
                Text(
                  switch (entry.lastWritebackAt) {
                    final DateTime at => l10n.settingsAgentsLastWriteback(
                      at.toLocal(),
                      at.toLocal(),
                      entry.lastWritebackSession,
                    ),
                    null => l10n.settingsAgentsLastWritebackRaw(
                      entry.lastWriteback,
                    ),
                  },
                  key: ValueKey<String>('agent-writeback-last-$agent'),
                  style: note,
                ),
            ],
          ),
        ),
        if (entry.sizeBytes > 0 || entry.inUse)
          fact(
            l10n.settingsAgentsSize,
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  formatBytes(entry.sizeBytes),
                  key: ValueKey<String>('agent-size-$agent'),
                  style: value,
                ),
                if (entry.inUse) Text(l10n.settingsAgentsInUse, style: note),
              ],
            ),
          ),
      ],
    );
  }
}

/// `agents.<id>.credential_writeback` as On and Off, written through
/// `SetConfig` like every other key (HUM-521 point 2).
class _WritebackRow extends StatelessWidget {
  const _WritebackRow({
    required this.agent,
    required this.value,
    required this.enabled,
    required this.onChanged,
  });

  final String agent;
  final bool value;
  final bool enabled;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Row(
      children: <Widget>[
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                l10n.settingsAgentsWriteback,
                style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
              ),
              Text(
                l10n.settingsAgentsWritebackHelp,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ],
          ),
        ),
        SizedBox(width: SettingRowGrid.gutter),
        Semantics(
          label: l10n.settingsAgentsWriteback,
          child: OnOffControl(
            key: ValueKey<String>('agent-writeback-$agent'),
            value: value,
            enabled: enabled,
            onChanged: onChanged,
          ),
        ),
      ],
    );
  }
}

/// The buttons of a card, each open as `actions` says. One of them is the
/// primary one: the next step of the depot, install first, then the login.
class _Buttons extends StatelessWidget {
  const _Buttons({
    required this.entry,
    required this.busy,
    required this.configShown,
    required this.onInstall,
    required this.onUpdate,
    required this.onLogin,
    required this.onConfigure,
    required this.onShowConfig,
    required this.onRemove,
  });

  final AgentDepot entry;
  final bool busy;
  final bool configShown;
  final VoidCallback onInstall;
  final VoidCallback onUpdate;
  final VoidCallback onLogin;
  final VoidCallback onConfigure;
  final VoidCallback onShowConfig;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentActions actions = entry.actions;
    final String agent = entry.agent;
    final bool loginNext = !actions.install && actions.login && !entry.loggedIn;
    Widget button(
      String id,
      String label,
      bool open,
      VoidCallback onPressed, {
      bool primary = false,
    }) => HButton(
      key: ValueKey<String>('agent-$id-$agent'),
      variant: primary ? HButtonVariant.primary : HButtonVariant.secondary,
      onPressed: open && !busy ? onPressed : null,
      child: Text(label),
    );
    // Eine Reihe für alle, Entfernen zuletzt: Bricht sie um, wandert
    // Entfernen mit und hängt nie allein oben rechts (Review HUM-521).
    return Wrap(
      spacing: tokens.spacing.x2,
      runSpacing: tokens.spacing.x2,
      children: <Widget>[
        button(
          'install',
          l10n.settingsAgentsInstall,
          actions.install,
          onInstall,
          primary: actions.install,
        ),
        button('update', l10n.settingsAgentsUpdate, actions.update, onUpdate),
        button(
          'log-in',
          l10n.settingsAgentsLogIn,
          actions.login,
          onLogin,
          primary: loginNext,
        ),
        button(
          'configure',
          l10n.settingsAgentsConfigure,
          actions.config,
          onConfigure,
        ),
        // Ausblenden geht immer, auch wenn ein Lauf das Zeigen sperrt.
        button(
          'show-config',
          configShown
              ? l10n.settingsAgentsHideConfig
              : l10n.settingsAgentsShowConfig,
          configShown || actions.configShow,
          onShowConfig,
        ),
        HButton(
          key: ValueKey<String>('agent-remove-$agent'),
          leading: const HGlyphIcon(HGlyph.trash, size: 14),
          onPressed: actions.remove && !busy ? onRemove : null,
          child: Text(l10n.settingsAgentsRemove),
        ),
      ],
    );
  }
}
