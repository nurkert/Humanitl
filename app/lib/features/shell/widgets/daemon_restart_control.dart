/// "Restart daemon" next to the version notice of the status bar (HUM-284).
library;

import 'package:flutter/services.dart';
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
/// A restart would end the agent's work, so while a session runs the button
/// first asks, right here in the bar (HUM-367): the destructive answer is
/// never the preselected one, and only that answer stops the session. It
/// rests while a start or stop is under way or nobody knows. The window's
/// snapshot only decides what is offered; whether the restart happens is
/// decided by the daemon itself (`humanitl daemon retire-if-idle`, see
/// `daemon_restart.dart`).
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

/// The stop of the sandbox, taken while the widget still lives.
typedef SandboxStatusNotifierStop = Future<void> Function();

class _DaemonRestartControlState extends ConsumerState<DaemonRestartControl> {
  /// True from the click until the restart ended, however it ended.
  bool _busy = false;

  /// True after the daemon said a session holds it, until a fresh snapshot.
  bool _held = false;

  /// True while the bar asks whether the running session may end.
  bool _asking = false;

  /// Takes the focus when the question appears: the safe answer comes first.
  final FocusNode _cancelFocus = FocusNode(debugLabel: 'restart-cancel');

  /// Who had the focus before the question, and gets it back after a cancel.
  FocusNode? _focusBefore;

  /// The sandbox the question is about: an answer ends this one or none.
  SandboxId? _askedAbout;

  /// The line of the command that failed, until the next click.
  String? _failure;

  /// `Sandbox(Status)` asked afresh through [client]: true or false from the
  /// first snapshot, null when none came. It takes the client as an argument
  /// because it may run after this widget is gone.
  static Future<bool?> _askSession(DaemonClient client) async {
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

  @override
  void dispose() {
    _cancelFocus.dispose();
    super.dispose();
  }

  void _ask(SandboxId? sandbox) {
    _focusBefore = FocusManager.instance.primaryFocus;
    setState(() {
      _asking = true;
      _askedAbout = sandbox;
    });
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && _asking) {
        _cancelFocus.requestFocus();
      }
    });
  }

  void _cancel() {
    setState(() => _asking = false);
    _focusBefore?.requestFocus();
    _focusBefore = null;
  }

  Future<void> _restart() {
    final DaemonClient client = ref.read(daemonClientProvider);
    return _run(
      () => restartUserDaemon(
        ref.read(daemonRestartRunnerProvider),
        cli: ref.read(daemonCliProvider),
        daemonMinor: widget.daemonMinor,
        askSession: () => _askSession(client),
      ),
    );
  }

  Future<void> _stopAndRestart() {
    final DaemonClient client = ref.read(daemonClientProvider);
    final SandboxStatusNotifierStop stop = ref
        .read(sandboxStatusProvider.notifier)
        .stop;
    return _run(
      () => restartAfterStop(
        ref.read(daemonRestartRunnerProvider),
        stop: stop,
        cli: ref.read(daemonCliProvider),
        daemonMinor: widget.daemonMinor,
        askSession: () => _askSession(client),
      ),
    );
  }

  Future<void> _run(Future<DaemonRestart> Function() restart) async {
    setState(() {
      _asking = false;
      _busy = true;
      _failure = null;
    });
    try {
      final DaemonRestart outcome = await restart();
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
      // Another sandbox is another question: the answer never carries over.
      if (_asking &&
          (!agentSessionStoppable(next) ||
              next.value?.sandboxId != _askedAbout)) {
        setState(() => _asking = false);
      }
    });
    final AsyncValue<SandboxStatus> status = ref.watch(sandboxStatusProvider);
    // Without a command line the restart could not follow the stop.
    final bool stoppable =
        !_held &&
        agentSessionStoppable(status) &&
        ref.watch(daemonCliProvider) != null;
    final bool holds = !stoppable && (_held || agentSessionHolds(status));
    final bool asking =
        _asking && stoppable && status.value?.sandboxId == _askedAbout;
    final TextStyle style = tokens.typography.ui11.tinted(tokens.colors.fg1);
    final String? failure = _failure;
    // The bar is narrow, so the answer that names what is lost stands alone.
    if (asking) {
      return CallbackShortcuts(
        bindings: <ShortcutActivator, VoidCallback>{
          const SingleActivator(LogicalKeyboardKey.escape): _cancel,
        },
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            HButton(
              key: const Key('status-daemon-restart-cancel'),
              focusNode: _cancelFocus,
              variant: HButtonVariant.ghost,
              onPressed: _cancel,
              child: Text(l10n.sandboxStopCancel, style: style),
            ),
            SizedBox(width: tokens.spacing.x1),
            Flexible(
              child: HButton(
                key: const Key('status-daemon-restart-confirm'),
                variant: HButtonVariant.ghost,
                onPressed: _stopAndRestart,
                child: Text(
                  l10n.shellStatusRestartStopAndRestart,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.ui11.medium.tinted(
                    tokens.colors.warningText,
                  ),
                ),
              ),
            ),
          ],
        ),
      );
    }
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        HButton(
          key: const Key('status-daemon-restart'),
          variant: HButtonVariant.ghost,
          onPressed: holds || _busy
              ? null
              : stoppable
              ? () => _ask(status.value?.sandboxId)
              : _restart,
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
