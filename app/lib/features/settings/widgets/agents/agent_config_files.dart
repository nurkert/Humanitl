/// Files of an agent's configuration with their keys (HUM-521 point 5): the
/// changes of a configuration run, or the files of `config/` read only.
///
/// The keys that run commands come from the daemon (`command_keys`, the
/// adapter's list) and stand first and highlighted; the app marks nothing by
/// a list of its own.
library;

import 'package:flutter/widgets.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';

/// The list of [files]; [showChange] adds the badge Added, Changed or
/// Removed, which a read-only view of `config/` leaves out.
class AgentConfigFiles extends StatelessWidget {
  /// Creates the list.
  const AgentConfigFiles({
    required this.agent,
    required this.files,
    this.showChange = true,
    super.key,
  });

  /// The agent's id, for the keys of the rows.
  final String agent;

  /// The files.
  final List<AgentConfigChange> files;

  /// Whether each file shows how it changed.
  final bool showChange;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool commands = files.any(
      (AgentConfigChange file) => file.commandKeys.isNotEmpty,
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (int i = 0; i < files.length; i++) ...<Widget>[
          if (i > 0) SizedBox(height: tokens.spacing.x2),
          _FileRow(
            key: ValueKey<String>('agent-config-file-$agent-${files[i].path}'),
            agent: agent,
            file: files[i],
            showChange: showChange,
          ),
        ],
        if (commands) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Row(
            children: <Widget>[
              HGlyphIcon(
                HGlyph.triangleAlert,
                size: 12,
                color: tokens.colors.warningText,
              ),
              SizedBox(width: tokens.spacing.x1),
              Flexible(
                child: Text(
                  l10n.settingsAgentsCommandKeys,
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

class _FileRow extends StatelessWidget {
  const _FileRow({
    required this.agent,
    required this.file,
    required this.showChange,
    super.key,
  });

  final String agent;
  final AgentConfigChange file;
  final bool showChange;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String change = switch (file.change) {
      'removed' => l10n.settingsAgentsChangeRemoved,
      'changed' => l10n.settingsAgentsChangeChanged,
      _ => l10n.settingsAgentsChangeAdded,
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Row(
          children: <Widget>[
            if (showChange) ...<Widget>[
              HBadge(text: change, minLabelWidth: 56),
              SizedBox(width: tokens.spacing.x2),
            ],
            Flexible(
              child: HMiddleEllipsis(
                file.path,
                style: tokens.typography.mono12.tinted(tokens.colors.fg0),
              ),
            ),
          ],
        ),
        if (file.keys.isNotEmpty || file.keysOmitted > 0) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Wrap(
            spacing: tokens.spacing.x1,
            runSpacing: tokens.spacing.x1,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              for (final String key in file.keys)
                if (file.commandKeys.contains(key))
                  HBadge(
                    key: ValueKey<String>('agent-command-key-$agent-$key'),
                    text: key,
                    mono: true,
                    color: tokens.colors.warning,
                  )
                else
                  HBadge(
                    key: ValueKey<String>('agent-key-$agent-$key'),
                    text: key,
                    mono: true,
                  ),
              if (file.keysOmitted > 0)
                Text(
                  l10n.settingsAgentsKeysOmitted(file.keysOmitted),
                  style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                ),
            ],
          ),
        ],
      ],
    );
  }
}
