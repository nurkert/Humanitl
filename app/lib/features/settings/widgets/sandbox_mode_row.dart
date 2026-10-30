/// The row of `sandbox.mode` in the settings form (HUM-357): automatic,
/// direct or disposable, and under it what the daemon made of it for this
/// system.
///
/// The form only writes the word. Whether `auto` becomes a disposable sandbox
/// on this system, and why not, the daemon decides and says in the snapshot
/// of the sandbox (`SandboxStatus.effectiveSandboxMode`,
/// `sandboxModeNotice`); the shell hands that to `resolvedSandboxModeProvider`
/// in core, and the row shows it and judges nothing itself (ADR-018).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/sandbox_mode_feed.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'setting_row.dart';

/// The row; shown only for a daemon that names `sandbox.disposable`, because
/// elsewhere (macOS) there is only one mode.
class SandboxModeRow extends ConsumerWidget {
  /// Creates the row for [config]; [onSelect] writes the chosen word.
  const SandboxModeRow({
    required this.config,
    required this.onSelect,
    this.refusal,
    this.anchor,
    super.key,
  });

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  /// Writes `auto`, `direct` or `disposable` through `SetConfig`.
  final ValueChanged<String> onSelect;

  /// What the daemon said when it refused the last write, or null.
  final Diagnostic? refusal;

  /// The anchor of a `ChangeSetting` fix on `sandbox.mode` (HUM-275).
  final FocusNode? anchor;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final bool offered =
        ref.watch(daemonInfoProvider).value?.offersDisposable ?? false;
    if (!offered) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SandboxModeResolution resolved = ref.watch(
      resolvedSandboxModeProvider,
    );
    final SandboxMode? effective = resolved.mode;
    final Diagnostic? notice = resolved.notice;
    final String written = config.sandboxModeSetting.trim().toLowerCase();
    final String selected = FormKeys.sandboxModes.contains(written)
        ? written
        : FormKeys.sandboxModes.first;
    return SettingRow(
      label: l10n.settingsSandboxModeLabel,
      help: l10n.settingsSandboxModeHelp,
      refusal: refusal,
      refusalKey: const Key('settings-sandbox-mode-refusal'),
      anchor: anchor,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Align(
            alignment: Alignment.centerLeft,
            child: HSegmented<String>(
              key: const Key('settings-sandbox-mode'),
              options: <HSegmentOption<String>>[
                HSegmentOption<String>(
                  value: 'auto',
                  label: l10n.settingsSandboxModeAuto,
                ),
                HSegmentOption<String>(
                  value: 'direct',
                  label: l10n.settingsSandboxModeDirect,
                ),
                HSegmentOption<String>(
                  value: 'disposable',
                  label: l10n.settingsSandboxModeDisposable,
                ),
              ],
              selected: selected,
              onSelect: onSelect,
            ),
          ),
          if (effective != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              effective == SandboxMode.disposable
                  ? l10n.settingsSandboxModeNowDisposable
                  : l10n.settingsSandboxModeNowDirect,
              key: const Key('settings-sandbox-mode-effective'),
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ],
          // Der Grund, wenn der Modus nicht ist, was `sandbox.mode` verlangt:
          // `SANDBOX_033` für den Rückfall der Vorgabe, `SANDBOX_030` für ein
          // ausdrückliches `disposable`, das hier ablehnen wird (HUM-357).
          if (notice != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            Text(
              DiagnosticL10n.resolve(notice, l10n).cause,
              key: const Key('settings-sandbox-mode-reason'),
              style: tokens.typography.ui12.tinted(tokens.colors.warningText),
            ),
          ],
        ],
      ),
    );
  }
}
