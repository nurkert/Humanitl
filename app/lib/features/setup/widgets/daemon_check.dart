/// The first row: does the background service answer? (HUM-044)
///
/// This is the one row whose verdict does not come out of the daemon, and it
/// cannot: only a client knows whether it reaches one. The doctor says so
/// itself and sends its own `daemon` line as "not measured"
/// (`daemon/crates/ipc/src/server.rs`); the command line replaces that line
/// with what its own connection attempt found, and so does this row.
///
/// The row does not poll by itself. While the connection is down the shell
/// retries every two seconds (`core/ipc/connection.dart`), so
/// a service that starts in a terminal turns this row green without anybody
/// touching the window.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The daemon row.
class DaemonCheck extends StatelessWidget {
  /// Creates the row for [check].
  const DaemonCheck({required this.check, required this.onRetry, super.key});

  /// What the row says.
  final SetupCheck check;

  /// Tries the connection again, now.
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.daemon),
      control: check.state == SetupCheckState.ok
          ? null
          : HButton(
              key: const Key('setup-daemon-retry'),
              size: HButtonSize.sm,
              onPressed: check.state == SetupCheckState.checking
                  ? null
                  : onRetry,
              child: Text(l10n.setupRetry),
            ),
    );
  }
}
