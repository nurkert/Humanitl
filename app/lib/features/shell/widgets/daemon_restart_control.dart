/// "Restart daemon" next to the version notice of the status bar (HUM-284).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../sandbox/providers/sandbox_status_provider.dart';
import '../daemon_restart.dart';

/// The button that restarts a daemon of another version, or the sentence
/// that says why it cannot right now.
///
/// It rests while an agent session holds the daemon: a restart would end the
/// agent's work. The window's snapshot only decides whether the button is
/// offered; whether the restart happens is decided by the daemon itself at
/// the click (`humanitl daemon retire-if-idle`, see `daemon_restart.dart`).
/// When the daemon says a session holds it, the button rests until a fresh
/// snapshot arrives; a refresh that brings none leaves it resting, because
/// nobody knows then.
class DaemonRestartControl extends ConsumerStatefulWidget {
  /// Creates the control for a daemon that speaks contract minor
  /// [daemonMinor].
  const DaemonRestartControl({required this.daemonMinor, super.key});

  /// `Info.proto_minor` of the daemon; from [daemonRetireMinor] on it knows
  /// `retire_if_idle`.
  final int daemonMinor;

  @override
  ConsumerState<DaemonRestartControl> createState() =>
      _DaemonRestartControlState();
}

class _DaemonRestartControlState extends ConsumerState<DaemonRestartControl> {
  /// True from the click until the restart ended, however it ended.
  bool _busy = false;

  /// True after the daemon said a session holds it, until a fresh snapshot.
  bool _held = false;

  /// The line of the command that failed, until the next click.
  String? _failure;

  /// `Sandbox(Status)` asked afresh: true or false from the first snapshot,
  /// null when none came.
  Future<bool?> _askSession() async {
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      await for (final SandboxUpdate update in client.sandboxStatus()) {
        if (update is SandboxUpdateStatus) {
          return agentSessionHolds(AsyncData<SandboxStatus>(update.status));
        }
      }
    } on DaemonException {
      return null;
    }
    return null;
  }

  Future<void> _restart() async {
    setState(() {
      _busy = true;
      _failure = null;
    });
    try {
      final DaemonRestart outcome = await restartUserDaemon(
        ref.read(daemonRestartRunnerProvider),
        cli: ref.read(daemonCliProvider),
        daemonMinor: widget.daemonMinor,
        askSession: _askSession,
      );
      if (!mounted) {
        return;
      }
      switch (outcome) {
        case DaemonRestarted():
          break;
        case DaemonSessionHolds():
          setState(() => _held = true);
          await ref.read(sandboxStatusProvider.notifier).refresh();
        case DaemonRestartFailed(:final String detail):
          setState(() => _failure = detail);
      }
    } finally {
      if (mounted) {
        setState(() => _busy = false);
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    ref.listen<AsyncValue<SandboxStatus>>(sandboxStatusProvider, (
      AsyncValue<SandboxStatus>? previous,
      AsyncValue<SandboxStatus> next,
    ) {
      // A fresh snapshot replaces the daemon's "a session holds me".
      if (_held && next.hasValue && !identical(previous?.value, next.value)) {
        setState(() => _held = false);
      }
    });
    final bool holds =
        _held || agentSessionHolds(ref.watch(sandboxStatusProvider));
    final TextStyle style = tokens.typography.ui11.tinted(tokens.colors.fg1);
    final String? failure = _failure;
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HButton(
          key: const Key('status-daemon-restart'),
          variant: HButtonVariant.ghost,
          onPressed: holds || _busy ? null : _restart,
          child: Text(l10n.shellStatusRestartDaemon, style: style),
        ),
        if (holds) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          Flexible(
            child: Text(
              l10n.shellStatusRestartAfterSession,
              key: const Key('status-daemon-restart-blocked'),
              overflow: TextOverflow.ellipsis,
              style: style,
            ),
          ),
        ],
        if (failure != null) ...<Widget>[
          SizedBox(width: tokens.spacing.x2),
          Flexible(
            child: Text(
              l10n.shellStatusRestartFailed(failure),
              key: const Key('status-daemon-restart-failed'),
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui11.tinted(tokens.colors.warningText),
            ),
          ),
        ],
      ],
    );
  }
}
