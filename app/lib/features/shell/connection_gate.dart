/// Chooses between splash, setup screen and shell from
/// `connectionStateProvider` (HUM-019 Widget-Baum, HUM-044).
///
/// The setup screen replaces the shell **only on a cold start**, never in the
/// middle of the work: whoever has twelve waiting requests on screen must not
/// lose the screen (`docs/UX.md` 4.2, case 4). Which of the two a failure is
/// gets decided in `connectionStateProvider`: it answers `ConnectionFailed`
/// while no daemon has ever answered, and `ConnectionFrozen` -- with the last
/// `DaemonInfo` -- once one has. The shell stays up for the second, and
/// `ShellScreen` marks the queue in it as a snapshot.
///
/// With a daemon the setup is the sixth section of the shell and no longer its
/// replacement, so the header keeps counting held requests while somebody sets
/// up (HUM-044).
///
/// With a daemon and no session view the gate shows the start window
/// (HUM-537, ADR-0024 point 5): the app never opens the view of a session
/// without a project, also not when `sandbox.work_dir` is set and not when a
/// session already runs. The running one stands under "Running" there.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ui/session_view.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import '../sandbox/widgets/cloud_confirm_dialog.dart';
import '../settings/settings_tab.dart';
import '../start/start_screen.dart';
import 'providers/connection.dart';
import 'shell_screen.dart';
import 'widgets/daemon_restart_control.dart';
import 'widgets/setup_host.dart';
import 'widgets/splash.dart';
import 'widgets/tray_host.dart';
import 'widgets/window_frame.dart';

/// The gate.
class ConnectionGate extends ConsumerWidget {
  /// Creates the gate.
  const ConnectionGate({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final ConnectionStatus status = ref.watch(connectionStateProvider);
    final bool inSession = ref.watch(sessionViewProvider) != null;
    // The host wraps the gate, not the shell: on a cold start there is no
    // shell to hang it in, and a daemon that stops answering while the shell
    // stands is exactly the moment the tray has to say that the number of
    // held requests is unknown (HUM-034).
    return TrayHost(
      child: switch (status) {
        // Splash und Setup fuellen das Fenster ohne Shell und damit ohne
        // deren Kopfleiste. Seit das Toolkit keine Titelleiste mehr zeichnet
        // (HUM-255), traegt die `WindowBar` darueber Griff und Fensterknoepfe;
        // die Shell bringt beides in ihrer eigenen Kopfleiste mit.
        ConnectionConnecting() => const WindowFramed(child: Splash()),
        // Ohne Daemon gibt es keine Shell, in die der Bildschirm passte:
        // jeder Abschnitt waere eine Fehlerkarte. Die erste Zeile traegt dann
        // den Befund mit seinem Vorschlag, und der Rest der Liste sagt, dass
        // nichts davon gemessen werden konnte.
        ConnectionFailed() => const WindowFramed(child: SetupHost()),
        // Ohne Sitzungsansicht das Startfenster (HUM-537): Kein Weg der App
        // öffnet eine Sitzung ohne Projekt, und `sandbox.work_dir` ist kein
        // stilles aktuelles Projekt.
        ConnectionFrozen(:final info) ||
        ConnectionConnected(:final info) when !inSession => WindowFramed(
          child: StartScreen(
            cloudConfirm:
                (
                  CloudInference cloud,
                  VoidCallback onCancel,
                  VoidCallback onConfirm,
                ) => CloudConfirmDialog(
                  cloud: cloud,
                  onCancel: onCancel,
                  onConfirm: onConfirm,
                ),
            settings: SettingsTab(
              daemonRestart: DaemonRestartControl(
                daemonMinor: info.protoMinor,
                size: HButtonSize.sm,
                label: context.l10n.settingsRestartButton,
              ),
            ),
          ),
        ),
        // Eine Verbindung, die stand und brach: Die Shell bleibt stehen, mit
        // dem letzten `GetInfo` in der Statuszeile, und das Banner darin sagt,
        // dass alles darunter ein Schnappschuss ist (`docs/UX.md` 4.2,
        // Fall 4).
        ConnectionFrozen(:final info) ||
        ConnectionConnected(:final info) => ShellScreen(info: info),
      },
    );
  }
}
