/// The category Sandbox: the project folder and where the agent writes
/// (HUM-293, HUM-357, HUM-404).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/work_dir_picker.dart' show directoryChooserProvider;
import '../../../l10n/l10n.dart';
import '../widgets/setting_controls.dart';
import '../widgets/setting_row.dart';

/// `sandbox.work_dir`: the folder, the chooser, and the way back to the
/// folder Humanitl starts in.
class WorkDirRow extends ConsumerWidget {
  /// Creates the row for [config].
  const WorkDirRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  Future<void> _choose(BuildContext context, WidgetRef ref) async {
    final String? dir = await ref.read(directoryChooserProvider)();
    if (dir != null && context.mounted) {
      writeSetting(context, FormKeys.sandboxWorkDir, dir);
    }
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String workDir = config.sandboxWorkDir;
    return SettingRow(
      settingId: FormKeys.sandboxWorkDir,
      refusalKey: const Key('settings-workdir-refusal'),
      fill: true,
      // Der Pfad über den Knöpfen und in der Mitte gekürzt: Anfang und Name
      // des Ordners bleiben lesbar, und „Ordner wählen" mit „Startordner
      // verwenden" passt im Deutschen auch bei doppelter Schrift (HUM-054).
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          HMiddleEllipsis(
            workDir.isEmpty ? l10n.settingsWorkDirNone : workDir,
            textKey: const Key('settings-workdir'),
            style: tokens.typography.mono12.tinted(
              workDir.isEmpty ? tokens.colors.fg2 : tokens.colors.fg0,
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            children: <Widget>[
              if (workDir.isNotEmpty)
                HButton(
                  key: const Key('settings-workdir-reset'),
                  variant: HButtonVariant.ghost,
                  onPressed: () =>
                      writeSetting(context, FormKeys.sandboxWorkDir, '-'),
                  child: Text(l10n.settingsWorkDirReset),
                ),
              HButton(
                key: const Key('settings-workdir-choose'),
                leading: const HGlyphIcon(HGlyph.folder, size: 14),
                onPressed: () => unawaited(_choose(context, ref)),
                child: Text(l10n.settingsWorkDirChoose),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
