/// The 24 px status bar: connection dot, daemon version, session.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The status bar.
class StatusBar extends StatelessWidget {
  /// Creates the status bar for the connected [info].
  const StatusBar({required this.info, super.key});

  /// What `GetInfo` said.
  final DaemonInfo info;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
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
                label: l10n.shellStatusConnected,
                child: SizedBox.square(
                  dimension: 8,
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: tokens.state.allowed,
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
