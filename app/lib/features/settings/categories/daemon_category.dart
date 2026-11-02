/// The category Daemon: the version of the background service, the update
/// check and `config.toml` (HUM-382, HUM-404).
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/file_opener.dart';
import '../registry/setting_entry.dart';
import '../widgets/setting_row.dart';

/// The version `GetInfo` named, as text and not as a control.
class VersionRow extends ConsumerWidget {
  /// Creates the row.
  const VersionRow({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final String? version = ref.watch(daemonInfoProvider).value?.daemonVersion;
    return SettingRow(
      settingId: SettingIds.daemonVersion,
      // Auf der Höhe eines Controls, damit der Wert auf der Linie des Namens
      // steht wie jeder Schalter darunter.
      child: SizedBox(
        height: tokens.sizes.hitMin,
        child: Align(
          alignment: Alignment.centerRight,
          widthFactor: 1,
          child: Text(
            version ?? context.l10n.settingsVersionUnknown,
            key: const Key('settings-version'),
            maxLines: 1,
            style: tokens.typography.mono12.tinted(
              version == null ? tokens.colors.fg2 : tokens.colors.fg0,
            ),
          ),
        ),
      ),
    );
  }
}

/// The restart of the daemon (HUM-404) with the control the shell hands in.
///
/// The flow -- the question during a session, the daemon that decides by
/// itself whether it may go -- is the one of the status bar
/// (`features/shell/daemon_restart.dart`); this row only places it. Without
/// a shell above there is no control, and the row says nothing it cannot do.
class RestartRow extends StatelessWidget {
  /// Creates the row around [control].
  const RestartRow({required this.control, super.key});

  /// The control of the shell, or null.
  final Widget? control;

  @override
  Widget build(BuildContext context) {
    final Widget? control = this.control;
    if (control == null) {
      return const SizedBox.shrink();
    }
    return SettingRow(settingId: SettingIds.daemonRestart, child: control);
  }
}

/// The path of `config.toml`, with Copy path and Open: everything the
/// overlay does not show stays in that file.
class ConfigFileRow extends ConsumerStatefulWidget {
  /// Creates the row for [config].
  const ConfigFileRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  @override
  ConsumerState<ConfigFileRow> createState() => _ConfigFileRowState();
}

class _ConfigFileRowState extends ConsumerState<ConfigFileRow> {
  bool _openFailed = false;

  Future<void> _open() async {
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
    final String path = widget.config.configPath;
    return SettingRow(
      settingId: SettingIds.configFile,
      fill: true,
      status: <Widget>[
        if (_openFailed)
          Text(
            l10n.settingsFileOpenFailed,
            key: const Key('settings-open-failed'),
            style: tokens.typography.ui12.tinted(tokens.stateText.error),
          ),
      ],
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          HMiddleEllipsis(
            path,
            textKey: const Key('settings-config-path'),
            style: tokens.typography.mono12.tinted(tokens.colors.fg1),
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              HButton(
                key: const Key('settings-copy-path'),
                variant: HButtonVariant.ghost,
                onPressed: path.isEmpty
                    ? null
                    : () => unawaited(
                        Clipboard.setData(ClipboardData(text: path)),
                      ),
                child: Text(l10n.settingsFileCopy),
              ),
              HButton(
                key: const Key('settings-open-file'),
                onPressed: path.isEmpty ? null : () => unawaited(_open()),
                child: Text(l10n.settingsFileOpen),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
