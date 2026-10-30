/// The 24 px status bar: connection dot, daemon version, queue pressure,
/// the active model server, session.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/recorder/recorder_health.dart';
import '../../../core/text/format.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../about/about_dialog.dart' as about show appVersion;
import '../../intercept/providers/flows.dart';
import '../queue_pressure.dart';
import 'daemon_restart_control.dart';
import 'llm_server_item.dart';

/// The version of a build nobody stamped (HUM-277).
const String _devBuild = '0.0.0';

/// The status bar.
///
/// Ein Consumer, weil der Punkt die eine Frage stellen muss, die der Rest der
/// Shell stellt: Lebt die Verbindung? Bis HUM-044 war er unbedingt grün und
/// hieß unbedingt „Connected". Das fiel nicht auf, solange ein Bruch die
/// ganze Shell durch den Setup-Bildschirm ersetzte; seit die Shell stehen
/// bleibt (`docs/UX.md` 4.2, Fall 4), stünde er als grüner Punkt über einem
/// Banner, das sagt, dass niemand antwortet.
class StatusBar extends ConsumerWidget {
  /// Creates the status bar for the connected [info].
  const StatusBar({
    required this.info,
    this.appVersion = about.appVersion,
    super.key,
  });

  /// What `GetInfo` said. Bei einem Bruch ist es die letzte Auskunft, nicht
  /// die aktuelle; der Punkt links davon sagt, welche von beiden.
  final DaemonInfo info;

  /// The version of this build; empty where the build did not set it.
  ///
  /// A parameter only so tests can name one: the build stamps it from the
  /// same tag as the daemon (`packaging/release/stamp-version.sh`).
  final String appVersion;

  /// Whether the daemon is another build than this app (HUM-277).
  ///
  /// A package upgrade replaces the files but leaves the running user daemon
  /// in place, and restarting the app does not restart it. Without a known
  /// version of its own the app cannot tell, and the fake daemon is no build
  /// at all.
  bool get _otherBuild {
    // `0.0.0` is the unstamped version of a development build on either side.
    // An empty version on either side is unknown, not different.
    if (appVersion.isEmpty ||
        info.daemonVersion.isEmpty ||
        appVersion == _devBuild ||
        info.daemonVersion == _devBuild ||
        info.isFake) {
      return false;
    }
    return info.daemonVersion != appVersion;
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool live = ref.watch(linkLiveProvider);
    final TextStyle style = tokens.typography.ui11.tinted(tokens.colors.fg1);
    return SizedBox(
      height: tokens.sizes.statusBar,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: tokens.colors.bg1,
          border: Border(top: BorderSide(color: tokens.colors.line)),
        ),
        child: Padding(
          padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
          child: Row(
            children: <Widget>[
              Semantics(
                label: live
                    ? l10n.shellStatusConnected
                    : l10n.shellStatusDisconnected,
                child: SizedBox.square(
                  key: const Key('status-connection-dot'),
                  dimension: 8,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      // Orange und nicht rot: Rot heißt in diesem Programm
                      // blockiert und nichts anderes (`docs/UX.md` 3.3). Es
                      // ist dieselbe Farbe, die das Banner darüber trägt.
                      color: live
                          ? tokens.state.allowed
                          : tokens.stateColor(HFlowState.error),
                      shape: BoxShape.circle,
                    ),
                  ),
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Text(
                l10n.shellStatusDaemon(info.daemonVersion),
                key: const Key('status-daemon-version'),
                style: style,
              ),
              if (_otherBuild) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                Flexible(
                  child: Text(
                    l10n.shellStatusDaemonMismatch(appVersion),
                    key: const Key('status-daemon-mismatch'),
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.ui11.medium.tinted(
                      tokens.colors.warningText,
                    ),
                  ),
                ),
                // HUM-284: the same restart as a button, for this user only
                // and never during an agent session.
                SizedBox(width: tokens.spacing.x2),
                Flexible(
                  flex: 2,
                  child: DaemonRestartControl(daemonMinor: info.protoMinor),
                ),
              ],
              if (info.isFake) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HBadge(
                  text: l10n.shellStatusFake,
                  color: tokens.state.passthroughLlm,
                ),
              ],
              const Spacer(),
              const RecorderStatusLabel(),
              const QueuePressureLabel(),
              // HUM-289: the active saved model server and its quick switch,
              // only when there is a choice.
              const LlmServerItem(),
              Text(
                info.hasSession
                    ? l10n.shellStatusSession(SessionId(info.sessionId).short)
                    : l10n.shellStatusNoSession,
                style: tokens.typography.mono11.tinted(tokens.colors.fg1),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// "Recording disrupted", while the daemon says that nothing is recorded
/// (HUM-250).
///
/// Zeichnet nichts, solange die Aufzeichnung schreibt. Während einer Störung
/// steht hier der Code des ersten Befunds und der Satz in der Farbe des
/// Fehlers: das Rot-Orange, das dieses Programm für einen Fehler hat, nicht
/// das Rot des Blockierens (`docs/UX.md` 3.3). Der Zustand kommt vom Daemon
/// (`recorderHealthProvider`); hier wird nichts gefolgert.
class RecorderStatusLabel extends ConsumerWidget {
  /// Creates the label.
  const RecorderStatusLabel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final RecorderOutage? outage = ref.watch(recorderHealthProvider);
    if (outage == null) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final Color color = tokens.stateTextColor(HFlowState.error);
    return Padding(
      padding: EdgeInsets.only(right: tokens.spacing.x3),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          HBadge(text: outage.diagnostic.code, color: color, mono: true),
          SizedBox(width: tokens.spacing.x2),
          Text(
            context.l10n.shellStatusRecorderDisrupted,
            key: const Key('status-recorder-disrupted'),
            style: tokens.typography.ui11.medium.tinted(color),
          ),
        ],
      ),
    );
  }
}

/// The load of the hold queue, once it gets close to a limit (HUM-057).
///
/// Unter 80 % beider Grenzen zeichnet es nichts: Eine Zahl, die immer dasteht,
/// liest niemand mehr, wenn sie einmal wichtig wird. Ab 80 % steht sie in der Warnfarbe
/// da, etwa „Queue 412/500 · 210 MB", ab 100 % rot und mit dem Code
/// `LIMIT_002` samt Titel, denn dann blockt der Daemon jede neue Anfrage.
class QueuePressureLabel extends ConsumerWidget {
  /// Creates the label.
  const QueuePressureLabel({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final QueueLoad load = QueueLoad.of(
      ref.watch(heldFlowsProvider),
      ref.watch(configProvider).value ?? ConfigValues.empty,
      ref.watch(daemonQueueBytesProvider),
    );
    final QueuePressure pressure = load.pressure;
    if (pressure == QueuePressure.normal) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Color color = pressure == QueuePressure.full
        ? tokens.stateTextColor(HFlowState.blocked)
        : tokens.colors.warningText;
    return Padding(
      padding: EdgeInsets.only(right: tokens.spacing.x3),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (pressure == QueuePressure.full) ...<Widget>[
            HBadge(
              key: const Key('status-queue-limit'),
              text: DiagnosticCodes.queueFull,
              color: color,
              mono: true,
            ),
            SizedBox(width: tokens.spacing.x2),
            Text(
              l10n.diagLIMIT002Title,
              style: tokens.typography.ui11.tinted(color),
            ),
            SizedBox(width: tokens.spacing.x2),
          ],
          Text(
            l10n.shellStatusQueue(
              load.count,
              load.maxFlows,
              formatBytes(load.bytes),
            ),
            key: const Key('status-queue-pressure'),
            style: tokens.typography.mono11.tinted(color),
          ),
        ],
      ),
    );
  }
}
