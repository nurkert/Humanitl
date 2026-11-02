/// "Who uses which server": the default, the current project and each
/// session profile, each with its choice of a saved server (HUM-405).
///
/// Every choice is one RPC: the default through `LlmServers(activate)`, a
/// project or a profile through `LlmServers(assign)`, which writes
/// `llm.server` into its file. The table decides nothing itself; what it
/// shows is what the daemon listed after the last change (ADR-018).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../../core/domain/domain.dart';
import '../../../../core/ipc/config_providers.dart';
import '../../../../core/ipc/daemon_client.dart';
import '../../../../core/ui/ui.dart';
import '../../../../l10n/l10n.dart';
import 'server_picker.dart';

/// The table of assignments over [servers].
class ServerUsage extends ConsumerStatefulWidget {
  /// Creates the table.
  const ServerUsage({required this.servers, super.key});

  /// The saved servers with their profiles and the project.
  final SavedServers servers;

  @override
  ConsumerState<ServerUsage> createState() => _ServerUsageState();
}

class _ServerUsageState extends ConsumerState<ServerUsage> {
  bool _busy = false;
  Diagnostic? _refusal;

  Future<void> _run(
    Future<void> Function(LlmServersSetting servers) change,
  ) async {
    if (_busy) {
      return;
    }
    setState(() => _busy = true);
    try {
      await change(ref.read(llmServersProvider.notifier));
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

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SavedServers list = widget.servers;
    final String fallback = list.active;
    final List<ServerPickerOption> named = <ServerPickerOption>[
      for (final SavedServer server in list.servers)
        ServerPickerOption(value: server.name, label: server.name),
    ];
    final List<ServerPickerOption> withDefault = <ServerPickerOption>[
      ServerPickerOption(
        value: '',
        label: l10n.settingsServerPickDefault(
          fallback.isEmpty ? '-' : fallback,
        ),
      ),
      ...named,
    ];
    final ProjectUse? project = list.project;
    final List<Widget> rows = <Widget>[
      _UsageRow(
        rowKey: 'default',
        label: l10n.settingsServerUsageDefaultLabel,
        note: l10n.settingsServerUsageDefaultNote,
        picker: ServerPicker(
          key: const Key('settings-usage-pick-default'),
          value: fallback,
          options: named,
          enabled: !_busy && list.servers.length > 1,
          semanticsLabel: l10n.settingsServerPickLabel(
            l10n.settingsServerUsageDefaultLabel,
          ),
          onSelected: (String name) => unawaited(
            _run((LlmServersSetting servers) => servers.activate(name)),
          ),
        ),
      ),
      if (project != null)
        _UsageRow(
          rowKey: 'project',
          label: l10n.settingsServerUsageProject(_lastSegment(project.dir)),
          // Die Auswahl schreibt nur die eigene Zeile des Projekts; kommt
          // die Wahl über ein Profil, das das Projekt nennt, sagt die Zeile
          // das, damit niemand das Projekt auf dem Standard glaubt (HUM-405).
          note: project.viaProfile.isNotEmpty
              ? l10n.settingsServerUsageViaProfile(
                  project.viaProfile,
                  project.effectiveServer,
                )
              : l10n.settingsServerUsageFromProject,
          warning: project.chosen.isNotEmpty && !project.confirmed
              ? l10n.settingsServerUsageUnconfirmed
              : null,
          picker: ServerPicker(
            key: const Key('settings-usage-pick-project'),
            value: project.server,
            options: withDefault,
            enabled: !_busy,
            semanticsLabel: l10n.settingsServerPickLabel(
              l10n.settingsServerUsageProject(_lastSegment(project.dir)),
            ),
            onSelected: (String name) => unawaited(
              _run(
                (LlmServersSetting servers) =>
                    servers.assign(server: name, projectDir: project.dir),
              ),
            ),
          ),
        ),
      for (final ProfileUse profile in list.profiles)
        _UsageRow(
          rowKey: 'profile-${profile.name}',
          label: l10n.settingsServerUsageProfile(profile.name),
          note: profile.description.isEmpty ? null : profile.description,
          picker: ServerPicker(
            key: ValueKey<String>(
              'settings-usage-pick-profile-${profile.name}',
            ),
            value: profile.server,
            options: withDefault,
            enabled: !_busy,
            semanticsLabel: l10n.settingsServerPickLabel(
              l10n.settingsServerUsageProfile(profile.name),
            ),
            onSelected: (String name) => unawaited(
              _run(
                (LlmServersSetting servers) =>
                    servers.assign(server: name, profile: profile.name),
              ),
            ),
          ),
        ),
    ];
    return Column(
      key: const Key('settings-server-usage'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.settingsServerUsageTitle,
          style: tokens.typography.ui13.medium.tinted(tokens.colors.fg0),
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.settingsServerUsageHelp,
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
        SizedBox(height: tokens.spacing.x3),
        DecoratedBox(
          decoration: BoxDecoration(
            border: Border.all(color: tokens.colors.line),
            borderRadius: BorderRadius.circular(tokens.radii.card),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              for (int i = 0; i < rows.length; i++) ...<Widget>[
                if (i > 0) const HHairline(),
                rows[i],
              ],
            ],
          ),
        ),
        if (_refusal case final Diagnostic refused) ...<Widget>[
          SizedBox(height: tokens.spacing.x2),
          Text(
            DiagnosticL10n.resolve(refused, l10n).cause,
            key: const Key('settings-usage-refusal'),
            style: tokens.typography.ui12.tinted(tokens.stateText.error),
          ),
        ],
      ],
    );
  }
}

/// The last part of a path, or the path itself.
String _lastSegment(String path) {
  final List<String> parts = path
      .split('/')
      .where((String part) => part.isNotEmpty)
      .toList();
  return parts.isEmpty ? path : parts.last;
}

/// One row: who on the left, the choice at the right edge.
class _UsageRow extends StatelessWidget {
  const _UsageRow({
    required this.rowKey,
    required this.label,
    required this.picker,
    this.note,
    this.warning,
  });

  final String rowKey;
  final String label;
  final String? note;
  final String? warning;
  final Widget picker;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? note = this.note;
    final String? warning = this.warning;
    return Padding(
      key: ValueKey<String>('settings-usage-$rowKey'),
      padding: EdgeInsets.symmetric(
        horizontal: tokens.spacing.x3,
        vertical: tokens.spacing.x2,
      ),
      child: Row(
        children: <Widget>[
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  label,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                ),
                if (note != null)
                  Text(
                    note,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui12.tinted(tokens.colors.fg2),
                  ),
                if (warning != null)
                  Padding(
                    padding: EdgeInsets.only(top: tokens.spacing.x1),
                    child: Row(
                      children: <Widget>[
                        HGlyphIcon(
                          HGlyph.triangleAlert,
                          size: 12,
                          color: tokens.colors.warning,
                        ),
                        SizedBox(width: tokens.spacing.x1),
                        Flexible(
                          child: Text(
                            warning,
                            key: ValueKey<String>(
                              'settings-usage-warning-$rowKey',
                            ),
                            style: tokens.typography.ui12.tinted(
                              tokens.colors.warningText,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            ),
          ),
          SizedBox(width: tokens.spacing.x4),
          picker,
        ],
      ),
    );
  }
}
