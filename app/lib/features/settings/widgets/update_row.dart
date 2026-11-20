/// "Check for updates automatically" and "Check now" (HUM-382).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ipc/update_status.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/untranslated_wording.dart';
import '../../../l10n/l10n.dart';
import 'setting_controls.dart';
import 'setting_row.dart';

/// The row of `update.check` with the button that asks right away.
///
/// The switch writes `update.check` through the overlay like every other
/// key; "Check now" asks the daemon with `force`, and the line under the help
/// says what it answered. The status bar follows the same answer through
/// [updateStatusProvider].
class UpdateRow extends ConsumerStatefulWidget {
  /// Creates the row for [config].
  const UpdateRow({required this.config, super.key});

  /// The configuration the overlay shows.
  final ConfigValues config;

  @override
  ConsumerState<UpdateRow> createState() => _UpdateRowState();
}

class _UpdateRowState extends ConsumerState<UpdateRow> {
  bool _checking = false;

  /// The answer of the last check, or null.
  UpdateStatus? _status;

  /// The finding of the last check when it failed, or null. Kept as it came
  /// and put into words in [build], so a change of the interface language
  /// reaches the line too (HUM-569).
  Diagnostic? _failure;

  Future<void> _checkNow() async {
    setState(() {
      _checking = true;
      _status = null;
      _failure = null;
    });
    UpdateStatus? status;
    Diagnostic? failure;
    try {
      status = await ref.read(updateStatusProvider.notifier).check(force: true);
    } on DaemonException catch (error) {
      failure = error.diagnostic;
    }
    if (mounted) {
      setState(() {
        _checking = false;
        _status = status;
        _failure = failure;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final UpdateStatus? status = _status;
    final Diagnostic? failure = _failure;
    final String? result = failure != null
        ? l10n.settingsUpdateCheckFailed(
            DiagnosticL10n.resolve(failure, l10n).cause,
          )
        : status == null
        ? null
        : status.available
        ? l10n.settingsUpdateAvailable(status.latestVersion)
        : l10n.settingsUpdateUpToDate(status.currentVersion);
    return SettingRow(
      settingId: FormKeys.updateCheck,
      refusalKey: const Key('settings-update-check-refusal'),
      status: <Widget>[
        if (_checking)
          Text(
            l10n.settingsUpdateChecking,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        if (result != null)
          Text(
            result,
            key: const Key('settings-update-result'),
            style: tokens.typography.ui12.tinted(
              failure != null ? tokens.stateText.error : tokens.colors.fg1,
            ),
          ),
        if (failure != null)
          if (UntranslatedWording.forDiagnostic(failure, l10n)
              case final Widget original)
            original,
      ],
      // „Jetzt prüfen" vor dem Schalter, beide in einer Reihe an der rechten
      // Kante: Die Frage von Hand gehört zur Zeile, nicht unter sie.
      leading: HButton(
        key: const Key('settings-update-check-now'),
        variant: HButtonVariant.ghost,
        onPressed: _checking ? null : () => unawaited(_checkNow()),
        child: Text(l10n.settingsUpdateCheckNow),
      ),
      child: OnOffControl(
        key: const Key('settings-update-check'),
        value: widget.config.updateCheck,
        onChanged: (bool on) =>
            writeSetting(context, FormKeys.updateCheck, '$on'),
      ),
    );
  }
}
