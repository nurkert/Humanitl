/// The row of `sandbox.backend` in the category Sandbox (HUM-440): automatic
/// or one of the backends the daemon admits, each with its isolation level,
/// and for one that cannot start here the daemon's reason and fix.
///
/// The row only writes the word through `SetConfig`. Which backends exist,
/// which of them could start, their levels and the reasons all come from
/// `SandboxBackends` (`sandboxBackendsProvider`); the row checks nothing of
/// its own (ADR-018).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/sandbox_backends_feed.dart';
import '../../../core/ui/sandbox_backend_choice.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../settings_scope.dart';
import 'setting_controls.dart';
import 'setting_row.dart';

/// The row. While the daemon has not answered it shows name and help only;
/// a daemon that cannot list its backends gets its reason under the help
/// and no control.
class SandboxBackendRow extends ConsumerWidget {
  /// Creates the row for [config].
  const SandboxBackendRow({required this.config, super.key});

  /// The configuration as `GetConfig` answered.
  final ConfigValues config;

  /// The key of the whole row, in every state: a `ChangeSetting` on
  /// `sandbox.backend` lands here even while the daemon has not listed its
  /// backends yet and the row has no control.
  static const Key rowKey = Key('settings-sandbox-backend-row');

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Die Liste für die Einstellung: ohne Backend der Sitzung (HUM-440).
    final AsyncValue<SandboxBackendList> answer = ref.watch(
      sandboxBackendsProvider(null),
    );
    // Die Liste kommt später als ein Sprung hierher (Abhilfe „Einstellungen
    // öffnen“): Kommt sie, erhält das Feld den Fokus, den der Sprung ihm
    // geben wollte.
    ref.listen<AsyncValue<SandboxBackendList>>(sandboxBackendsProvider(null), (
      AsyncValue<SandboxBackendList>? before,
      AsyncValue<SandboxBackendList> now,
    ) {
      if (before?.value == null && now.value != null) {
        final SettingsController? scope = SettingsScope.maybeOf(context);
        WidgetsBinding.instance.addPostFrameCallback((_) {
          // Die Zeile kann bis zum nächsten Bild verschwunden sein.
          if (context.mounted) {
            scope?.refocus(FormKeys.sandboxBackend);
          }
        });
      }
    });
    final SandboxBackendList? list = answer.value;
    if (list == null) {
      final Object? error = answer.error;
      return SettingRow(
        key: rowKey,
        settingId: FormKeys.sandboxBackend,
        status: <Widget>[
          if (error != null)
            Text(
              l10n.sandboxBackendListFailed(
                DiagnosticL10n.resolve(
                  DaemonConnection.diagnosticOf(error),
                  l10n,
                ).cause,
              ),
              key: const Key('settings-sandbox-backend-failed'),
              style: tokens.typography.ui12.tinted(tokens.colors.warningText),
            ),
        ],
        child: const SizedBox.shrink(),
      );
    }
    final String written = config.sandboxBackend.trim();
    return SettingRow(
      key: rowKey,
      settingId: FormKeys.sandboxBackend,
      refusalKey: const Key('settings-sandbox-backend-refusal'),
      status: <Widget>[
        SandboxBackendLines(list: list, keyPrefix: 'settings-sandbox-backend'),
      ],
      child: offersBackendChoice(list)
          ? HSegmented<String>(
              key: const Key('settings-sandbox-backend'),
              options: sandboxBackendOptions(l10n, list),
              selected: written,
              onSelect: (String backend) {
                if (backend != written) {
                  writeSetting(context, FormKeys.sandboxBackend, backend);
                }
              },
            )
          : const SizedBox.shrink(),
    );
  }
}
