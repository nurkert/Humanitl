/// The 24 px status bar: connection dot, daemon version, session.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

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
