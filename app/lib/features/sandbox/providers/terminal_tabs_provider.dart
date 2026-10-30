/// The tabs of the sandbox terminal: the agent's, and further shells beside
/// it (HUM-314).
///
/// The daemon opens a further shell with `Exec` and names it; the shell's
/// terminal is then a [TerminalSession] like the agent's, keyed by the same
/// sandbox and the shell's id (HUM-299). What lives here is only the list of
/// tabs this window opened, which of them is shown, and the last refusal of
/// `Exec`. Nothing here filters, counts or decides what the daemon decides:
/// the limit of four is the daemon's, and its `TERM_002` is what the window
/// shows when it says no.
///
/// **The shells end with the session.** When the agent of the session ends,
/// or another sandbox takes its place, PID 1 of the sandbox ends every
/// further shell (HUM-299); the tabs go with it, and so do their emulators.
/// The daemon offers no call that ends one shell, so closing a tab ends only
/// this window's view of it; a shell that still runs ends with its process
/// or with the session.
library;

import 'dart:async';

import 'package:riverpod_annotation/riverpod_annotation.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui_state/ui_state_file.dart';
import 'sandbox_status_provider.dart';
import 'terminal_provider.dart';

part 'terminal_tabs_provider.g.dart';

/// How many further shells live at a time, as in the shim and the daemon
/// (`MAX_SHELLS`, HUM-299). The button stops here; the daemon's `TERM_002`
/// stays the word that counts, because a shell this window no longer shows
/// may still run.
const int maxFurtherShells = 4;

/// One further shell this window opened.
class ShellTab {
  /// A tab for [shellId], shown as shell [number].
  const ShellTab({required this.shellId, required this.number});

  /// The id `Exec` answered, for `TerminalInput.Open.shell_id`.
  final String shellId;

  /// The number in the label, counted per session from 1.
  final int number;
}

/// The tabs of the terminal of one session.
class TerminalTabsState {
  /// The tabs of [sandboxId].
  const TerminalTabsState({
    this.sandboxId = '',
    this.shells = const <ShellTab>[],
    this.active = '',
    this.opening = false,
    this.refusal,
    this.opened = 0,
    this.byUser = false,
  });

  /// The session whose agent runs, or empty.
  final String sandboxId;

  /// The further shells, in the order they were opened.
  final List<ShellTab> shells;

  /// The shell shown, or empty for the agent's terminal.
  final String active;

  /// True while an `Exec` is on its way.
  final bool opening;

  /// Why the daemon refused the last `Exec`, until it is dismissed or the
  /// next one goes out.
  final Diagnostic? refusal;

  /// How many shells this window opened in this session; the next one gets
  /// the number after it.
  final int opened;

  /// Whether the last change of [active] came straight from the human (a
  /// click on a tab, a chord, a close). Only then may the new tab take the
  /// keyboard from a text field; a shell that arrives late or the end of
  /// the session must not.
  final bool byUser;

  /// Whether the button stops: four further shells are open.
  bool get full => shells.length >= maxFurtherShells;

  /// The ids of all tabs in order, the agent's first as the empty id.
  List<String> get order => <String>[
    '',
    for (final ShellTab tab in shells) tab.shellId,
  ];

  /// The tab of [shellId], or null.
  ShellTab? tabOf(String shellId) {
    for (final ShellTab tab in shells) {
      if (tab.shellId == shellId) {
        return tab;
      }
    }
    return null;
  }

  /// A copy with other values. [refusal] is set as given, null included,
  /// when [setRefusal] is true.
  TerminalTabsState copyWith({
    List<ShellTab>? shells,
    String? active,
    bool? opening,
    bool setRefusal = false,
    Diagnostic? refusal,
    int? opened,
    bool? byUser,
  }) => TerminalTabsState(
    sandboxId: sandboxId,
    shells: shells ?? this.shells,
    active: active ?? this.active,
    opening: opening ?? this.opening,
    refusal: setRefusal ? refusal : this.refusal,
    opened: opened ?? this.opened,
    byUser: byUser ?? this.byUser,
  );
}

/// Whether the daemon opens further shells (`Info.capabilities` names
/// `sandbox.exec`). False while nothing is known.
@riverpod
bool shellsOffered(Ref ref) =>
    ref.watch(daemonInfoProvider).value?.offersShells ?? false;

/// The tabs of the terminal, for every host that shows it (pane, whole
/// window, floating window).
@Riverpod(keepAlive: true)
class TerminalTabs extends _$TerminalTabs {
  @override
  TerminalTabsState build() {
    // Nur eine Antwort des Daemons beendet die Shells (Review Runde 1): Ein
    // Status, der gerade lädt oder dessen Abfrage scheiterte, sagt nichts
    // darüber, ob der Agent noch läuft. Ohne diese Wache warf ein einziger
    // Fehlschlag der Abfrage alle Reiter einer laufenden Sitzung weg.
    ref.listen<AsyncValue<SandboxStatus>>(sandboxStatusProvider, (
      AsyncValue<SandboxStatus>? _,
      AsyncValue<SandboxStatus> next,
    ) {
      if (next is! AsyncData<SandboxStatus>) {
        return;
      }
      final String id = _runningId(next);
      if (id != state.sandboxId) {
        _endAll(id);
      }
    });
    return TerminalTabsState(
      sandboxId: _runningId(ref.read(sandboxStatusProvider)),
    );
  }

  /// The id of the session whose agent runs, or empty.
  static String _runningId(AsyncValue<SandboxStatus> snapshot) {
    final SandboxStatus? status = snapshot.value;
    return status != null && status.agentRunning
        ? (status.sandboxId?.value ?? '')
        : '';
  }

  /// Asks the daemon for a further shell of [cols] by [rows] and shows it.
  ///
  /// Nothing while no session runs, while an `Exec` is on its way or while
  /// four tabs are open. A refusal of the daemon stands in [state] as
  /// [TerminalTabsState.refusal] until it is dismissed.
  Future<void> openShell({required int cols, required int rows}) async {
    final String sandboxId = state.sandboxId;
    if (sandboxId.isEmpty || state.opening || state.full) {
      return;
    }
    state = state.copyWith(opening: true, setRefusal: true);
    final DaemonClient client = ref.read(daemonClientProvider);
    try {
      final ShellStarted started = await client.exec(
        sandboxId: sandboxId,
        cols: cols,
        rows: rows,
      );
      if (!ref.mounted) {
        return;
      }
      // The session may have ended while the daemon answered; its shells
      // ended with it.
      if (state.sandboxId != sandboxId) {
        return;
      }
      final int number = state.opened + 1;
      state = state.copyWith(
        shells: <ShellTab>[
          ...state.shells,
          ShellTab(shellId: started.shellId, number: number),
        ],
        active: started.shellId,
        opening: false,
        opened: number,
        // The answer may come after the human moved on to a text field.
        byUser: false,
      );
    } on DaemonException catch (error) {
      if (!ref.mounted || state.sandboxId != sandboxId) {
        return;
      }
      state = state.copyWith(
        opening: false,
        setRefusal: true,
        refusal: error.diagnostic,
      );
    }
  }

  /// Shows the tab of [shellId], or the agent's for the empty id.
  void select(String shellId) {
    if (shellId == state.active ||
        (shellId.isNotEmpty && state.tabOf(shellId) == null)) {
      return;
    }
    state = state.copyWith(active: shellId, byUser: true);
  }

  /// Shows the tab [delta] places to the right, around the end.
  void step(int delta) {
    final List<String> order = state.order;
    if (order.length < 2) {
      return;
    }
    final int at = order.indexOf(state.active);
    final int next = (at + delta) % order.length;
    select(order[next < 0 ? next + order.length : next]);
  }

  /// Closes the tab of [shellId]: this window detaches from the shell's
  /// terminal and forgets its emulator. The shell itself ends with its
  /// process or with the session; the daemon has no call to end it.
  void close(String shellId) {
    final List<String> order = state.order;
    final int at = order.indexOf(shellId);
    if (shellId.isEmpty || at < 0) {
      return;
    }
    final String active = state.active == shellId
        ? order[at - 1]
        : state.active;
    state = state.copyWith(
      shells: <ShellTab>[
        for (final ShellTab tab in state.shells)
          if (tab.shellId != shellId) tab,
      ],
      active: active,
      byUser: true,
    );
    _detach(state.sandboxId, shellId);
  }

  /// Hides the refusal of the last `Exec`.
  void dismissRefusal() {
    if (state.refusal != null) {
      state = state.copyWith(setRefusal: true);
    }
  }

  /// The session [sandboxId] replaced the last one, or none runs: every
  /// further shell of the old one has ended with it.
  void _endAll(String sandboxId) {
    final TerminalTabsState old = state;
    state = TerminalTabsState(sandboxId: sandboxId);
    _forget(old);
  }

  void _forget(TerminalTabsState tabs) {
    for (final ShellTab tab in tabs.shells) {
      _detach(tabs.sandboxId, tab.shellId);
    }
  }

  /// Detaches from the terminal of [shellId] and drops its emulator.
  void _detach(String sandboxId, String shellId) {
    final TerminalSessionProvider session = terminalSessionProvider(
      sandboxId,
      shellId: shellId,
    );
    if (ref.exists(session)) {
      unawaited(ref.read(session.notifier).detach());
      ref.invalidate(session);
    }
  }
}

/// The key under which the one-time hint about further shells is marked as
/// read in the state file of the interface.
const String shellHintSeenKey = 'terminalShellHintSeen';

/// Whether the hint "the agent can read this shell" has been closed before
/// (HUM-314).
///
/// Shown over the terminal when the first further shell opens, until it is
/// closed; then never again, also after a restart. It lives in the same state
/// file as the coach mark of the queue, and a write that does not go through
/// keeps it closed for this run only, as there.
@Riverpod(keepAlive: true)
class ShellHintSeen extends _$ShellHintSeen {
  @override
  bool build() =>
      ref.read(uiStateFileProvider).read()[shellHintSeenKey] == true;

  /// Closes the hint for good.
  void dismiss() {
    if (state) {
      return;
    }
    state = true;
    ref.read(uiStateFileProvider).setFlag(shellHintSeenKey);
  }
}
