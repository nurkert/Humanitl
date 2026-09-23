/// What the filter refused the agent, below the three guarantees (HUM-138).
///
/// The guarantees say the sandbox holds; this section says what the agent
/// tried against it. Blocked traffic outside HTTP never reaches the proxy, so
/// it never becomes a flow, never waits in the queue and never lands in the
/// history. Without this section a person watching the agent would see a
/// calm screen while it knocks on every other door.
///
/// Counted, not logged: one line per family and type, as the daemon sends
/// it. Three states must never look alike: "not reported yet", "reported,
/// nothing refused" and "refused, but not counted here" (`SANDBOX_019`). A
/// refused `connect(2)` is not in here at all, and the section says so: that
/// is the empty routing table of the first guarantee, a state and not an
/// event.
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';

/// The refusals section of the isolation tab.
class RefusalsSection extends StatelessWidget {
  /// Shows the refusals of [status].
  const RefusalsSection({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SandboxRefusals? refusals = status.refusals;
    if (refusals == null && !status.isUp) {
      // Nothing runs and nothing ran: the tab already says so at the top.
      return const SizedBox.shrink();
    }
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text(
            l10n.isolationRefusalsTitle,
            key: const Key('sandbox-refusals-title'),
            style: tokens.typography.ui13.tinted(tokens.colors.fg0),
          ),
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.isolationRefusalsIntro,
            style: tokens.typography.ui11.tinted(tokens.colors.fg2),
          ),
          SizedBox(height: tokens.spacing.x2),
          ..._body(tokens, l10n, refusals),
          SizedBox(height: tokens.spacing.x2),
          Text(
            l10n.isolationRefusalsNetworkNote,
            key: const Key('sandbox-refusals-network'),
            style: tokens.typography.ui11.tinted(tokens.colors.fg2),
          ),
        ],
      ),
    );
  }

  List<Widget> _body(
    HTokens tokens,
    AppLocalizations l10n,
    SandboxRefusals? refusals,
  ) {
    final SandboxRefusalReporting reporting =
        refusals?.reporting ?? SandboxRefusalReporting.unknown;
    final List<SandboxRefusal> entries =
        refusals?.entries ?? const <SandboxRefusal>[];
    return <Widget>[
      if (reporting == SandboxRefusalReporting.off)
        Text(
          l10n.isolationRefusalsOff(
            (refusals?.offReason.isEmpty ?? true) ? '?' : refusals!.offReason,
          ),
          key: const Key('sandbox-refusals-off'),
          style: tokens.typography.ui13.tinted(tokens.stateText.held),
        )
      else if (entries.isEmpty)
        Text(
          reporting == SandboxRefusalReporting.on
              ? l10n.isolationRefusalsNone
              : l10n.isolationRefusalsPending,
          key: Key(
            reporting == SandboxRefusalReporting.on
                ? 'sandbox-refusals-none'
                : 'sandbox-refusals-pending',
          ),
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      for (final SandboxRefusal entry in entries)
        _RefusalLine(
          key: Key('sandbox-refusal-${entry.family}-${entry.socketType}'),
          entry: entry,
        ),
    ];
  }
}

/// One family and type: how often, which call, why, and when last.
class _RefusalLine extends StatelessWidget {
  const _RefusalLine({required this.entry, super.key});

  final SandboxRefusal entry;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DateTime? last = entry.lastAt;
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x1),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          SizedBox(
            width: tokens.spacing.x3 * 6,
            child: Text(
              l10n.isolationRefusalsCount(entry.count),
              style: tokens.typography.ui13.tinted(tokens.stateText.blocked),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  l10n.isolationRefusalsSocket(entry.family, entry.socketType),
                  // Names from <sys/socket.h>, left to right whatever else
                  // the line holds.
                  textDirection: TextDirection.ltr,
                  style: tokens.typography.mono11.tinted(tokens.colors.fg0),
                ),
                Wrap(
                  spacing: tokens.spacing.x2,
                  children: <Widget>[
                    Text(
                      _reason(l10n, entry.reason),
                      style: tokens.typography.ui11.tinted(tokens.colors.fg2),
                    ),
                    if (last != null)
                      Text(
                        l10n.isolationRefusalsLast(_clock(last)),
                        style: tokens.typography.ui11.tinted(tokens.colors.fg2),
                      ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  static String _reason(AppLocalizations l10n, SandboxRefusalReason reason) =>
      switch (reason) {
        SandboxRefusalReason.family => l10n.isolationRefusalsReasonFamily,
        SandboxRefusalReason.type => l10n.isolationRefusalsReasonType,
        SandboxRefusalReason.overflow => l10n.isolationRefusalsReasonOverflow,
        SandboxRefusalReason.unknown => l10n.isolationRefusalsReasonUnknown,
      };

  /// `hh:mm:ss` of [at], local time, like the log tab.
  static String _clock(DateTime at) {
    final DateTime local = at.toLocal();
    String two(int n) => n.toString().padLeft(2, '0');
    return '${two(local.hour)}:${two(local.minute)}:${two(local.second)}';
  }
}
