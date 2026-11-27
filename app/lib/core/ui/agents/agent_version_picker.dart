/// The version choice before an install or an update (HUM-521 point 2), the
/// counterpart of `humanitl agents install|update <id> --version`.
///
/// "Latest" sends no version, and the daemon takes the vendor's current one.
/// "Specific version" sends what is typed, or a version the depot still
/// holds: the daemon then switches back without a run. The app checks no
/// version; the daemon does and says so at the card.
library;

import 'package:flutter/widgets.dart';

import '../../domain/domain.dart';
import '../ui.dart';
import '../../../l10n/l10n.dart';
import 'agent_inset.dart';

/// The version choice of [entry]; [update] tells an update from an install.
class AgentVersionPicker extends StatefulWidget {
  /// Creates the choice.
  const AgentVersionPicker({
    required this.entry,
    required this.update,
    required this.busy,
    required this.onStart,
    required this.onCancel,
    super.key,
  });

  /// The agent.
  final AgentDepot entry;

  /// True for an update, false for the first install.
  final bool update;

  /// A request is under way.
  final bool busy;

  /// Starts with the chosen version; empty means the vendor's current one.
  final ValueChanged<String> onStart;

  /// Closes the choice.
  final VoidCallback onCancel;

  @override
  State<AgentVersionPicker> createState() => _AgentVersionPickerState();
}

class _AgentVersionPickerState extends State<AgentVersionPicker> {
  bool _pinned = false;
  final TextEditingController _version = TextEditingController();
  final FocusNode _field = FocusNode(debugLabel: 'agent-version');

  @override
  void initState() {
    super.initState();
    _version.addListener(_changed);
  }

  @override
  void dispose() {
    _version
      ..removeListener(_changed)
      ..dispose();
    _field.dispose();
    super.dispose();
  }

  void _changed() => setState(() {});

  String get _chosen => _pinned ? _version.text.trim() : '';

  void _pick(String held) {
    setState(() => _pinned = true);
    _version.text = held;
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentDepot entry = widget.entry;
    final List<String> held = widget.update
        ? entry.olderVersions
        : const <String>[];
    final String chosen = _chosen;
    final bool switching = held.contains(chosen);
    final String start = switching
        ? l10n.settingsAgentsSwitchTo(chosen)
        : (widget.update
              ? l10n.settingsAgentsStartUpdate
              : l10n.settingsAgentsStartInstall);
    final bool ready = !widget.busy && (!_pinned || chosen.isNotEmpty);
    return AgentInset(
      key: ValueKey<String>('agent-version-picker-${entry.agent}'),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          AgentInsetTitle(
            widget.update
                ? l10n.settingsAgentsUpdateTitle(entry.displayName)
                : l10n.settingsAgentsInstallTitle(entry.displayName),
          ),
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.settingsAgentsVersionRun,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x3),
          HSegmented<bool>(
            key: ValueKey<String>('agent-version-mode-${entry.agent}'),
            options: <HSegmentOption<bool>>[
              HSegmentOption<bool>(
                value: false,
                label: l10n.settingsAgentsVersionLatest,
              ),
              HSegmentOption<bool>(
                value: true,
                label: l10n.settingsAgentsVersionPinned,
              ),
            ],
            selected: _pinned,
            onSelect: (bool pinned) {
              setState(() => _pinned = pinned);
              if (pinned) {
                _field.requestFocus();
              }
            },
          ),
          if (_pinned) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            SizedBox(
              width: 240,
              child: HTextField(
                key: ValueKey<String>('agent-version-input-${entry.agent}'),
                controller: _version,
                focusNode: _field,
                semanticsLabel: l10n.settingsAgentsVersionField,
                hint: l10n.settingsAgentsVersionHint,
                onSubmitted: (String _) {
                  if (ready) {
                    widget.onStart(_chosen);
                  }
                },
              ),
            ),
          ],
          if (held.isNotEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            Text(
              l10n.settingsAgentsVersionHeld,
              style: tokens.typography.ui12.tinted(tokens.colors.fg2),
            ),
            SizedBox(height: tokens.spacing.x1),
            Wrap(
              spacing: tokens.spacing.x2,
              runSpacing: tokens.spacing.x2,
              children: <Widget>[
                for (final String version in held)
                  HBadge(
                    key: ValueKey<String>(
                      'agent-version-held-${entry.agent}-$version',
                    ),
                    text: version,
                    mono: true,
                    color: chosen == version
                        ? tokens.colors.accent
                        : tokens.colors.fg1,
                    textColor: chosen == version
                        ? tokens.colors.accentText
                        : null,
                    onTap: () => _pick(version),
                  ),
              ],
            ),
          ],
          AgentInsetActions(
            children: <Widget>[
              HButton(
                key: ValueKey<String>('agent-version-cancel-${entry.agent}'),
                variant: HButtonVariant.secondary,
                onPressed: widget.onCancel,
                child: Text(l10n.settingsAgentsCancel),
              ),
              HButton(
                key: ValueKey<String>('agent-version-start-${entry.agent}'),
                variant: HButtonVariant.primary,
                onPressed: ready ? () => widget.onStart(_chosen) : null,
                child: Text(start),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
