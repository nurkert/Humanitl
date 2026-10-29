/// The terminal of the current session, for whichever host shows it
/// (HUM-298).
///
/// The Sandbox section shows it in its pane, the shell over the whole window,
/// and a floating overlay later a third time (HUM-300). Every host builds this
/// one widget, so all of them ask the same question -- which session runs? --
/// in the same words, and all of them reach the same emulator: the session
/// and its scrollback live in `terminalSessionProvider`, which outlives every
/// host. Only one host shows it at a time (`terminalPlacementProvider`).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../providers/sandbox_status_provider.dart';
import 'terminal_pane.dart';

/// The terminal of the running session, or its empty state.
class SandboxTerminal extends ConsumerStatefulWidget {
  /// Creates the terminal for a host.
  ///
  /// [focusOnMount] hands the keyboard to the agent as soon as the host shows
  /// it. [catchUp] asks the daemon once which session runs: a host outside
  /// the Sandbox section cannot rely on the section having asked recently.
  const SandboxTerminal({
    this.focusOnMount = false,
    this.catchUp = false,
    super.key,
  });

  /// Whether the terminal takes the keyboard when it appears.
  final bool focusOnMount;

  /// Whether the session is asked for again when the terminal appears.
  final bool catchUp;

  @override
  ConsumerState<SandboxTerminal> createState() => _SandboxTerminalState();
}

class _SandboxTerminalState extends ConsumerState<SandboxTerminal> {
  @override
  void initState() {
    super.initState();
    if (widget.catchUp) {
      // A session may have started from the command line while another
      // section was showing; the same catch-up as the section does when it
      // becomes visible.
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (mounted) {
          unawaited(ref.read(sandboxStatusProvider.notifier).refresh());
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final SandboxStatus status =
        ref.watch(sandboxStatusProvider).value ?? const SandboxStatus();
    return TerminalPane(
      key: const Key('sandbox-terminal'),
      sandboxId: status.agentRunning ? (status.sandboxId?.value ?? '') : '',
      focusOnMount: widget.focusOnMount,
    );
  }
}
