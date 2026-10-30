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
/// says whose turn it is. The floating terminal (HUM-300) is the third value
/// and the third host: a small window over the sections, so the agent can be
/// watched while requests are decided.
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

  /// In a small window floating over the sections, on whatever section is
  /// shown (HUM-300).
  floating,
}

/// Where the terminal stands now; [TerminalPlacement.pane] at the start.
final NotifierProvider<TerminalPlacementNotifier, TerminalPlacement>
terminalPlacementProvider =
    NotifierProvider<TerminalPlacementNotifier, TerminalPlacement>(
      TerminalPlacementNotifier.new,
    );

/// The notifier behind [terminalPlacementProvider].
class TerminalPlacementNotifier extends Notifier<TerminalPlacement> {
  /// Where the terminal stood before it went over the whole window: the
  /// whole window returns there (HUM-300). Only [TerminalPlacement.pane] or
  /// [TerminalPlacement.floating].
  TerminalPlacement _beforeFullWindow = TerminalPlacement.pane;

  @override
  TerminalPlacement build() => TerminalPlacement.pane;

  /// Shows the terminal over the whole window, or puts it back where it stood
  /// before -- its pane or the floating window -- when it already fills the
  /// window: the shortcut and the button do the same.
  void toggleFullWindow() {
    if (state == TerminalPlacement.fullWindow) {
      state = _beforeFullWindow;
    } else {
      _beforeFullWindow = state;
      state = TerminalPlacement.fullWindow;
    }
  }

  /// Puts the terminal back where it stood before the whole window. Nothing
  /// happens when it does not fill the window.
  void leaveFullWindow() {
    if (state == TerminalPlacement.fullWindow) {
      state = _beforeFullWindow;
    }
  }

  /// Lets the terminal float, or puts it back into its pane when it already
  /// floats: the shortcut and the button do the same (HUM-300). From the
  /// whole window it floats as well; the whole window then no longer returns
  /// to the pane.
  void toggleFloating() =>
      state == TerminalPlacement.floating ? dock() : float();

  /// Lets the terminal float. Nothing happens when it already does.
  void float() {
    _beforeFullWindow = TerminalPlacement.pane;
    state = TerminalPlacement.floating;
  }

  /// Puts a floating terminal back into its pane. Nothing happens when it
  /// does not float.
  void dock() {
    if (state == TerminalPlacement.floating) {
      state = TerminalPlacement.pane;
    }
  }
}

/// Whether the Sandbox section is the one the shell shows (HUM-315).
///
/// The shell knows its sections, the hosts of the terminal do not, and a
/// feature may not ask the shell (ARCHITECTURE 5); the shell writes this
/// note whenever the section changes, and [terminalHostProvider] reads it.
final NotifierProvider<SandboxPageShown, bool> sandboxPageShownProvider =
    NotifierProvider<SandboxPageShown, bool>(SandboxPageShown.new);

/// The notifier behind [sandboxPageShownProvider].
class SandboxPageShown extends Notifier<bool> {
  @override
  bool build() => false;

  /// Notes whether the Sandbox section is shown.
  void set({required bool shown}) {
    if (state != shown) {
      state = shown;
    }
  }
}

/// The host that shows the terminal for the placement the person [chosen]
/// (HUM-315).
///
/// **A floating terminal docks on the Sandbox page.** While that page shows,
/// the terminal stands in its pane there, where it belongs, and a window
/// over the same page would only cover the page's own copy of it. The choice
/// stays: [terminalPlacementProvider] keeps [TerminalPlacement.floating],
/// and leaving the page lets the terminal float again. Docking because of
/// the page is a state of the view, never a decision of the person.
///
/// This is the one rule that turns a choice into a host; every host and the
/// shell ask it through [terminalHostProvider] and nowhere else.
TerminalPlacement terminalHostFor(
  TerminalPlacement chosen, {
  required bool sandboxPageShown,
}) => chosen == TerminalPlacement.floating && sandboxPageShown
    ? TerminalPlacement.pane
    : chosen;

/// The host that shows the terminal now: [terminalHostFor] over the choice
/// in [terminalPlacementProvider] and the page in [sandboxPageShownProvider]
/// (HUM-315).
///
/// Who decides what is shown reads this one; who changes or names the
/// person's choice (the buttons, the palette, the shortcuts) reads
/// [terminalPlacementProvider].
final Provider<TerminalPlacement> terminalHostProvider =
    Provider<TerminalPlacement>(
      (Ref ref) => terminalHostFor(
        ref.watch(terminalPlacementProvider),
        sandboxPageShown: ref.watch(sandboxPageShownProvider),
      ),
    );

/// How often somebody asked for the keyboard to go to the terminal
/// (HUM-303).
///
/// `Ctrl+Backquote` and the palette ask for it from the shell, which does not
/// know the focus node of the emulator; the terminal does not know the
/// shortcut. The count is the note between them: every host of the terminal
/// listens, and the one that is shown takes the keyboard. A host that is not
/// shown ignores it. With no terminal on the screen and a running agent the
/// shell first lets the terminal float and asks one frame later (HUM-300);
/// without an agent the request does nothing. Only a change
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
