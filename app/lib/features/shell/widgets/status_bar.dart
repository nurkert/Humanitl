/// The 24 px status bar: connection dot, daemon version, queue pressure,
/// session.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/config_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/text/format.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../intercept/providers/flows.dart';
import '../queue_pressure.dart';

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
  const StatusBar({required this.info, super.key});

  /// What `GetInfo` said. Bei einem Bruch ist es die letzte Auskunft, nicht
  /// die aktuelle; der Punkt links davon sagt, welche von beiden.
  final DaemonInfo info;

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
              if (info.isFake) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                HBadge(
                  text: l10n.shellStatusFake,
                  color: tokens.state.passthroughLlm,
                ),
              ],
              const Spacer(),
              const QueuePressureLabel(),
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
