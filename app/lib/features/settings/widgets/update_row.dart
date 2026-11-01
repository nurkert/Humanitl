/// "Check for updates automatically" and "Check now" (HUM-382).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/update_status.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import 'setting_row.dart';

/// The row of `update.check` with the button that asks right away.
///
/// The checkbox writes `update.check` through the form like every other
/// key; "Check now" asks the daemon with `force`, and the line under it says
/// what it answered. The status bar follows the same answer through
/// [updateStatusProvider].
class UpdateRow extends ConsumerStatefulWidget {
  /// Creates the row.
  const UpdateRow({
    required this.config,
    required this.onToggle,
    this.refusal,
    this.anchor,
    super.key,
  });

  /// The configuration the form shows.
  final ConfigValues config;

  /// Writes `update.check`.
  final ValueChanged<bool> onToggle;

  /// The daemon's refusal of the last write of `update.check`.
  final Diagnostic? refusal;

  /// The anchor of the row (HUM-275).
  final FocusNode? anchor;

  @override
  ConsumerState<UpdateRow> createState() => _UpdateRowState();
}

class _UpdateRowState extends ConsumerState<UpdateRow> {
  bool _checking = false;

  /// The line under the button, after the last check.
  String? _result;

  /// True when [_result] reports a failure.
  bool _failed = false;

  Future<void> _checkNow() async {
    final AppLocalizations l10n = context.l10n;
    setState(() {
      _checking = true;
      _result = null;
    });
    String line;
    bool failed = false;
    try {
      final UpdateStatus status = await ref
          .read(updateStatusProvider.notifier)
          .check(force: true);
      line = status.available
          ? l10n.settingsUpdateAvailable(status.latestVersion)
          : l10n.settingsUpdateUpToDate(status.currentVersion);
    } on DaemonException catch (error) {
      line = l10n.settingsUpdateCheckFailed(
        DiagnosticL10n.resolve(error.diagnostic, l10n).cause,
      );
      failed = true;
    }
    if (mounted) {
      setState(() {
        _checking = false;
        _result = line;
        _failed = failed;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String? result = _result;
    return SettingRow(
      label: l10n.settingsUpdateCheckLabel,
      help: l10n.settingsUpdateCheckHelp,
      refusal: widget.refusal,
      refusalKey: const Key('settings-update-check-refusal'),
      anchor: widget.anchor,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          HCheckbox(
            key: const Key('settings-update-check'),
            label: l10n.settingsUpdateCheckHint,
            value: widget.config.updateCheck,
            onChanged: widget.onToggle,
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x2,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              HButton(
                key: const Key('settings-update-check-now'),
                onPressed: _checking ? null : () => unawaited(_checkNow()),
                child: Text(l10n.settingsUpdateCheckNow),
              ),
              if (_checking)
                Text(
                  l10n.settingsUpdateChecking,
                  style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                ),
            ],
          ),
          if (result != null)
            Text(
              result,
              key: const Key('settings-update-result'),
              style: tokens.typography.ui12.tinted(
                _failed ? tokens.stateText.error : tokens.colors.fg1,
              ),
            ),
        ],
      ),
    );
  }
}
