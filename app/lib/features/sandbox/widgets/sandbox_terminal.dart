/// The terminal of the current session, for whichever host shows it
/// (HUM-298).
///
/// The Sandbox section shows it in its pane, the shell over the whole window
/// and in the floating window over the sections (HUM-300). Every host builds this
/// one widget, so all of them ask the same question -- which session runs? --
/// in the same words, and all of them reach the same emulator: the session
/// and its scrollback live in `terminalSessionProvider`, which outlives every
/// host. Only one host shows it at a time (`terminalPlacementProvider`).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/terminal_placement.dart';
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
    this.floating = false,
    super.key,
  });

  /// Whether the terminal takes the keyboard when it appears.
  final bool focusOnMount;

  /// Whether the session is asked for again when the terminal appears.
  final bool catchUp;

  /// Whether the host is the floating window (`TerminalPane.floating`).
  final bool floating;

  @override
  ConsumerState<SandboxTerminal> createState() => _SandboxTerminalState();
}

/// The session the floating window shows after its agent ended (HUM-300,
/// review rounds 1 and 2).
///
/// When the agent ends, the floating window stays a moment to show the end
/// before it closes; with the empty id of a session that no longer runs it
/// showed the empty state instead of the exit line. The id lives here and
/// not in the widget, because the window may be built anew in that moment:
/// floating, then the whole window, the agent ends, back to floating. A new
/// session replaces the id, putting the terminal back into its pane clears
/// it. The pane and the whole window keep the rule they had: no running
/// agent, no session.
final NotifierProvider<LastFloatingSession, String>
lastFloatingSessionProvider = NotifierProvider<LastFloatingSession, String>(
  LastFloatingSession.new,
);

/// The notifier behind [lastFloatingSessionProvider].
class LastFloatingSession extends Notifier<String> {
  @override
  String build() {
    ref
      ..listen<String>(sandboxStatusProvider.select(_runningId), (
        String? _,
        String id,
      ) {
        if (id.isNotEmpty) {
          state = id;
        }
      }, fireImmediately: true)
      // Names the status another sandbox, or none, the ended session is
      // gone: a sandbox B that starts during the linger of A must never get
      // A's emulator, nor an `attach` of A (review round 3).
      ..listen<String>(sandboxStatusProvider.select(_sandboxId), (
        String? _,
        String id,
      ) {
        if (state.isNotEmpty && id != state) {
          state = '';
        }
      })
      ..listen<TerminalPlacement>(terminalPlacementProvider, (
        TerminalPlacement? _,
        TerminalPlacement next,
      ) {
        if (next == TerminalPlacement.pane) {
          state = '';
        } else if (next == TerminalPlacement.floating) {
          final String id = _runningId(ref.read(sandboxStatusProvider));
          if (id.isNotEmpty) {
            state = id;
          }
        }
      });
    return '';
  }

  /// The id of the sandbox the status names, running or not, or empty.
  static String _sandboxId(AsyncValue<SandboxStatus> snapshot) =>
      snapshot.value?.sandboxId?.value ?? '';

  /// The id of the session whose agent runs, or empty.
  static String _runningId(AsyncValue<SandboxStatus> snapshot) {
    final SandboxStatus? status = snapshot.value;
    return status != null && status.agentRunning
        ? (status.sandboxId?.value ?? '')
        : '';
  }
}

class _SandboxTerminalState extends ConsumerState<SandboxTerminal> {
  @override
  void initState() {
    super.initState();
    // Every host starts the note, so it has seen the session before a
    // floating window needs it.
    ref.read(lastFloatingSessionProvider);
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
    final String running = status.agentRunning
        ? (status.sandboxId?.value ?? '')
        : '';
    // Only while the status still names the sandbox whose agent ended.
    final String saved = widget.floating
        ? ref.watch(lastFloatingSessionProvider)
        : '';
    final String ended = saved == (status.sandboxId?.value ?? '') ? saved : '';
    return TerminalPane(
      key: const Key('sandbox-terminal'),
      sandboxId: running.isEmpty ? ended : running,
      focusOnMount: widget.focusOnMount,
      floating: widget.floating,
    );
  }
}
