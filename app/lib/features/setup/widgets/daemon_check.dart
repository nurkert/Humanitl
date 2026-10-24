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
///
/// The line under the title counts those attempts (HUM-058, row 1). A red row
/// that stands still reads as a window that gave up; a number that grows says
/// the window is still asking, without a spinner for a wait nobody can
/// measure (`docs/UX.md` 2.11).
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/fix_control.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The daemon row.
class DaemonCheck extends StatelessWidget {
  /// Creates the row for [check].
  const DaemonCheck({
    required this.check,
    required this.onRetry,
    this.attempts = 0,
    this.view = SetupStepView.focused,
    super.key,
  });

  /// Open or one line, in front or not (HUM-287).
  final SetupStepView view;

  /// What the row says.
  final SetupCheck check;

  /// How many attempts failed so far; 0 hides the line.
  final int attempts;

  /// Tries the connection again, now.
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    final HTokens tokens = HTheme.of(context);
    final bool down = check.state != SetupCheckState.ok;
    // Bietet der Befund einen eigenen Knopf -- den Installer der Unit --,
    // ist der die eine Aktion dieses Schritts, und „Erneut verbinden" tritt
    // daneben zurück: zwei Primärknöpfe in einem Schritt sagen nicht, welcher
    // zuerst kommt (HUM-287).
    final bool fixIsPrimary = FixControl.offersButton(
      context,
      check.diagnostic?.fix,
    );
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.daemon),
      view: view,
      primaryFix: fixIsPrimary,
      body: down && attempts > 0
          ? Text(
              l10n.setupDaemonAttempts(attempts),
              key: const Key('setup-daemon-attempts'),
              style: tokens.typography.mono12.tinted(tokens.colors.fg1),
            )
          : null,
      actions: <Widget>[
        if (down)
          HButton(
            key: const Key('setup-daemon-retry'),
            variant: view.current && !fixIsPrimary
                ? HButtonVariant.primary
                : HButtonVariant.secondary,
            autofocus: view.current && !fixIsPrimary,
            onPressed: check.state == SetupCheckState.checking ? null : onRetry,
            child: Text(l10n.setupRetry),
          ),
      ],
      lineAction: down
          ? HButton(
              key: const Key('setup-daemon-retry'),
              size: HButtonSize.sm,
              onPressed: check.state == SetupCheckState.checking
                  ? null
                  : onRetry,
              child: Text(l10n.setupRetry),
            )
          : null,
    );
  }
}
