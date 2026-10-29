/// Where the terminal of the sandbox stands (HUM-298).
///
/// The terminal belongs to the Sandbox section, but it can be shown over the
/// whole window from any section. A feature may not reach into the shell to
/// ask for that, and the shell may not reach into the inner workings of the
/// terminal (ARCHITECTURE 5); this provider is the note between them. The
/// shell reads it and decides the layout, the Sandbox section reads it and
/// leaves its own slot empty while the terminal stands elsewhere, and the
/// button in the terminal's header writes it.
///
/// **Exactly one place shows the terminal at a time.** The emulator and its
/// scrollback live in the session provider of the sandbox feature, which
/// outlives every widget; a host that mounts the terminal attaches to the
/// same session and shows the same lines. Two views of one session would
/// both resize it and both take keys, so the hosts take turns, and this value
/// says whose turn it is. A floating overlay (HUM-300) becomes a third value
/// here and a third host in the shell.
///
/// The same note carries the request for the keyboard (HUM-303): the shell
/// asks for it, the terminal that is shown takes it.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';

/// The places the terminal can stand.
enum TerminalPlacement {
  /// In its pane, in the upper part of the Sandbox section.
  pane,

  /// Over the whole content of the window, without rail and other panes.
  fullWindow,
}

/// Where the terminal stands now; [TerminalPlacement.pane] at the start.
final NotifierProvider<TerminalPlacementNotifier, TerminalPlacement>
terminalPlacementProvider =
    NotifierProvider<TerminalPlacementNotifier, TerminalPlacement>(
      TerminalPlacementNotifier.new,
    );

/// The notifier behind [terminalPlacementProvider].
class TerminalPlacementNotifier extends Notifier<TerminalPlacement> {
  @override
  TerminalPlacement build() => TerminalPlacement.pane;

  /// Shows the terminal over the whole window, or puts it back into its pane
  /// when it already stands there: the shortcut and the button do the same.
  void toggleFullWindow() => state = state == TerminalPlacement.fullWindow
      ? TerminalPlacement.pane
      : TerminalPlacement.fullWindow;

  /// Puts the terminal back into its pane. Nothing happens when it is there.
  void leaveFullWindow() {
    if (state == TerminalPlacement.fullWindow) {
      state = TerminalPlacement.pane;
    }
  }
}

/// How often somebody asked for the keyboard to go to the terminal
/// (HUM-303).
///
/// `Ctrl+Backquote` and the palette ask for it from the shell, which does not
/// know the focus node of the emulator; the terminal does not know the
/// shortcut. The count is the note between them: every host of the terminal
/// listens, and the one that is shown takes the keyboard. A host that is not
/// shown ignores it, so with no terminal on the screen the request does
/// nothing (HUM-300 will open the floating terminal instead). Only a change
/// counts; the number itself means nothing.
final NotifierProvider<TerminalFocusRequests, int>
terminalFocusRequestProvider = NotifierProvider<TerminalFocusRequests, int>(
  TerminalFocusRequests.new,
);

/// The notifier behind [terminalFocusRequestProvider].
class TerminalFocusRequests extends Notifier<int> {
  @override
  int build() => 0;

  /// Asks the terminal that is shown to take the keyboard.
  void request() => state++;
}
