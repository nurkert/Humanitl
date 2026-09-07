/// The terminal of the agent, and the two strips above it (HUM-042).
///
/// This is where the work happens, and it is the one pane of this program
/// whose content belongs to somebody else. Three things follow from that, and
/// all three are visible:
///
/// * **The banner never goes away.** The output of the agent is untrusted, and
///   a warning that appears only when something looks wrong is a warning
///   nobody reads. It stands above the terminal for as long as the terminal
///   does (`docs/THREAT-MODEL.md` K-09).
/// * **The strip is outside the emulator.** When a request of the agent waits
///   for a person, it says so here rather than only in the byte stream: a
///   full-screen TUI redraws with absolute addressing and would paint over
///   that line with its next frame.
/// * **No filter runs here.** The bytes arrive filtered from the daemon, and
///   the emulator gets no OSC handler of its own: the clipboard and the window
///   title of the human belong to the human (`docs/SECURITY.md` 3.3).
library;

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:xterm2/core.dart';
import 'package:xterm2/ui.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/intents.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/generated/app_localizations.dart';
import '../providers/terminal_provider.dart';

/// The height of the two strips above the terminal.
const double terminalStripHeight = 24;

/// The terminal of the running session.
class TerminalPane extends ConsumerStatefulWidget {
  /// Shows the terminal of [sandboxId], or the reason there is none.
  const TerminalPane({required this.sandboxId, super.key});

  /// The session this pane belongs to; empty while nothing runs.
  final String sandboxId;

  @override
  ConsumerState<TerminalPane> createState() => _TerminalPaneState();
}

class _TerminalPaneState extends ConsumerState<TerminalPane> {
  final FocusNode _focus = FocusNode(debugLabel: 'terminal');
  final TerminalController _controller = TerminalController();
  final HContextMenuController _menu = HContextMenuController();

  @override
  void initState() {
    super.initState();
    _attach();
  }

  @override
  void didUpdateWidget(TerminalPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.sandboxId != widget.sandboxId) {
      _attach();
    }
  }

  @override
  void dispose() {
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  void _attach() {
    if (widget.sandboxId.isEmpty) {
      return;
    }
    // After the first frame: by then the view has told the emulator how many
    // columns it has, and the `Open` carries the geometry the human really
    // sees rather than the 80x24 every terminal starts with.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) {
        return;
      }
      unawaited(
        ref.read(terminalSessionProvider(widget.sandboxId).notifier).attach(),
      );
    });
  }

  /// Wer eine Taste bekommt: die Anwendung oder der Agent.
  ///
  /// **`ignored` heißt hier „mach du".** `TerminalView` ruft diese Funktion vor
  /// seiner eigenen Behandlung und nimmt jede Antwort außer `ignored` als
  /// Übersteuerung: Es hört dann sofort auf und übersetzt die Taste nicht mehr
  /// in Bytes. Bis zum 2026-09-07 stand hier `skipRemainingHandlers` für alles,
  /// und weil Buchstaben über die Texteingabe laufen und nicht über die
  /// Tastenbehandlung, fiel es niemandem auf: Tippen ging, aber Rücktaste,
  /// Entf, Pfeile, Enter und jedes `Ctrl`-Kürzel erreichten den Agenten nie.
  /// Gemeldet hat es ein Mensch vor dem Bildschirm, nicht ein Test.
  ///
  /// **Die Kürzel der Anwendung ruft diese Funktion selbst auf, und das muss
  /// sie.** Der naheliegende Weg -- `ignored` zurückgeben und die Taste nach
  /// oben durchreichen -- trägt nur so weit, wie der Emulator die Taste nicht
  /// kennt. `Ctrl+K` ist für ihn `0x0b` und `Ctrl+6` ist `0x1e`
  /// (`xterm2/src/core/input/handler.dart`, `CtrlInputHandler`); er meldet
  /// `handled`, und damit endet die Reise des Ereignisses vor jedem
  /// `Shortcuts` darüber. Die andere naheliegende Antwort,
  /// `skipRemainingHandlers`, hält zwar den Emulator auf, beendet die Reise
  /// aber genauso (`FocusManager` bricht die Schleife über die Vorfahren ab).
  /// Bleibt der ehrliche Weg: Diese Funktion sucht die Taste in
  /// [shellShortcuts] und löst die Absicht dort aus, wo sie hingehört -- über
  /// [Actions], also bei genau der Stelle, die sie auch sonst behandelt.
  ///
  /// Der Preis steht ausdrücklich hier: `Ctrl+1` bis `Ctrl+6` und `Ctrl+K`
  /// erreichen den Agenten nicht. Ohne sie käme man aus einem Vollbild-TUI nur
  /// noch mit der Maus heraus (`docs/UX.md` 5.1). Alles andere gehört ihm,
  /// `Ctrl+C` als `0x03` und `Ctrl+P` als `0x10` eingeschlossen.
  ///
  /// Hört an dieser Stelle des Baums niemand auf eine Absicht -- im Test etwa,
  /// der die Kachel ohne Schale zeigt --, bleibt die Taste beim Agenten.
  /// Die Kürzel, die der Emulator selbst behandelt.
  ///
  /// Seine Vorgabe bindet drei: Kopieren (`Ctrl+Shift+C`), Einfügen
  /// (`Ctrl+V`) und **alles auswählen** (`Ctrl+A`). Die ersten beiden spiegeln
  /// das Kontextmenü darüber und bleiben. Das dritte nicht: `Ctrl+A` ist in
  /// readline und in jedem Programm auf bubbletea -- also auch in OpenCode --
  /// der Sprung an den Zeilenanfang, und ein Terminal, in dem der nicht
  /// ankommt, ist an dieser Stelle kaputt. Der Emulator prüft seine Kürzel vor
  /// jeder Übersetzung in Bytes (`TerminalView._handleKeyEvent`), also hilft
  /// hier nichts als das Kürzel selbst wegzunehmen; ausgewählt wird mit der
  /// Maus und kopiert über das Menü.
  ///
  /// Die Vorgabe kommt weiterhin aus dem Emulator und wird nur beschnitten:
  /// Auf einem Mac trägt sie `Meta` statt `Ctrl`, und das soll sie behalten.
  Map<ShortcutActivator, Intent> _emulatorShortcuts() {
    final Map<ShortcutActivator, Intent> shortcuts =
        Map<ShortcutActivator, Intent>.of(defaultTerminalShortcuts);
    shortcuts.removeWhere(
      (ShortcutActivator _, Intent intent) => intent is SelectAllTextIntent,
    );
    return shortcuts;
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    for (final MapEntry<ShortcutActivator, Intent> binding
        in shellShortcuts().entries) {
      if (!binding.key.accepts(event, HardwareKeyboard.instance)) {
        continue;
      }
      // Mit der Absicht suchen, nicht nur mit ihrem Typ: `Actions.maybeFind`
      // schlägt über `intent.runtimeType` nach, `Actions.handler` dagegen
      // über den statischen Typ `Intent` -- und den registriert niemand.
      if (Actions.maybeFind<Intent>(context, intent: binding.value) == null) {
        break;
      }
      Actions.maybeInvoke<Intent>(context, binding.value);
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (widget.sandboxId.isEmpty) {
      return _Idle(text: l10n.sandboxTerminalIdle);
    }
    final TerminalSessionState session = ref.watch(
      terminalSessionProvider(widget.sandboxId),
    );
    final TerminalNotice? notice = ref.watch(heldNoticeProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _Banner(text: l10n.sandboxTerminalUntrustedBanner),
        if (notice != null)
          _HeldStrip(
            text: l10n.sandboxTerminalHeldNotice(
              notice.method,
              notice.host,
              notice.path,
            ),
          ),
        if (_strip(l10n, session) case final Widget strip) strip,
        Expanded(child: _view(tokens, l10n, session)),
        if (session.exitCode case final int code)
          _ExitStrip(text: l10n.sandboxTerminalExited(code)),
      ],
    );
  }

  /// Die Zeile über dem Terminal, oder keine.
  ///
  /// Drei Lagen, und der Unterschied zwischen ihnen ist der Grund für diese
  /// Funktion. **Zusehen** ist der Normalfall neben einer schreibenden
  /// Kommandozeile: ein Satz im ruhigen Chrome, keine Zustandsfarbe, denn
  /// nichts daran ist der Zustand einer Anfrage (`docs/UX.md` 5 und 6).
  /// **Ein Befund** ist die Absage, die dieses Fenster wirklich trifft, und
  /// trägt das Orange eines Findings. **Und dazwischen** liegt der Fall, den
  /// ein früherer Entwurf übersah: Endet der Agent, während dieses Fenster
  /// zusieht, bleibt `TERM_001` im Zustand stehen -- der Befund gehört dann
  /// einer Lage, die vorbei ist, und er käme neben der Exit-Zeile in der Farbe
  /// zurück, die „ging nicht" heißt. Deshalb hängt die Unterdrückung am Code
  /// und nicht an der Phase.
  Widget? _strip(AppLocalizations l10n, TerminalSessionState session) {
    final Diagnostic? diagnostic = session.diagnostic;
    if (diagnostic == null) {
      return null;
    }
    final bool gaveUpTheKeyboard =
        session.readOnly &&
        diagnostic.code == DiagnosticCodes.terminalSecondWriter &&
        session.phase != TerminalPhase.refused;
    if (gaveUpTheKeyboard) {
      return session.phase == TerminalPhase.attached
          ? _WatchingStrip(text: l10n.sandboxTerminalWatching)
          : null;
    }
    return _FindingStrip(
      text: l10n.sandboxTerminalFinding(diagnostic.code, diagnostic.why),
    );
  }

  Widget _view(
    HTokens tokens,
    AppLocalizations l10n,
    TerminalSessionState session,
  ) => HContextMenu(
    controller: _menu,
    semanticsLabel: l10n.sandboxTerminalMenu,
    itemsBuilder: () => <HMenuItem>[
      HMenuItem(
        label: l10n.sandboxTerminalCopy,
        enabled: _controller.selection != null,
        onSelected: _copy,
      ),
      // Einfügen geht am Rohmodus vorbei: `Terminal.paste` ruft `onOutput`
      // auch dann, wenn die Ansicht nur liest, und die Bytes gingen hinauf,
      // wo der Daemon sie verwirft. Ein Menüpunkt, der nichts tut, ist genau
      // die Lüge, die der Kommentar am Cursor drei Zeilen tiefer ausschließt.
      HMenuItem(
        label: l10n.sandboxTerminalPaste,
        enabled: !session.readOnly && session.phase == TerminalPhase.attached,
        onSelected: _paste,
      ),
    ],
    child: ColoredBox(
      color: tokens.terminal.background,
      child: TerminalView(
        session.terminal,
        controller: _controller,
        focusNode: _focus,
        autofocus: true,
        shortcuts: _emulatorShortcuts(),
        theme: _theme(tokens.terminal),
        textStyle: TerminalStyle(
          fontSize: tokens.typography.mono13.fontSize ?? 13,
          fontFamily: HType.monoFamily,
          fontFamilyFallback: HType.monoFallback,
        ),
        padding: EdgeInsets.all(tokens.spacing.x2),
        onSecondaryTapDown: (TapDownDetails details, CellOffset _) =>
            _menu.open(details.globalPosition),
        onKeyEvent: _onKey,
        // A reader may not type, and nobody may while the session is not
        // attached: the daemon drops those keys, and an emulator that still
        // takes them shows a cursor waiting for an agent that never hears it.
        readOnly: session.readOnly || session.phase != TerminalPhase.attached,
      ),
    ),
  );

  void _copy() {
    final BufferRange? selection = _controller.selection;
    if (selection == null) {
      return;
    }
    final String text = ref
        .read(terminalSessionProvider(widget.sandboxId))
        .terminal
        .buffer
        .getText(selection);
    unawaited(Clipboard.setData(ClipboardData(text: text)));
  }

  void _paste() {
    unawaited(
      Clipboard.getData(Clipboard.kTextPlain).then((ClipboardData? data) {
        final String? text = data?.text;
        if (text != null && text.isNotEmpty && mounted) {
          ref
              .read(terminalSessionProvider(widget.sandboxId))
              .terminal
              .paste(text);
        }
      }),
    );
  }

  /// The palette of this program, in the shape the emulator wants.
  TerminalTheme _theme(HTerminalPalette palette) => TerminalTheme(
    cursor: palette.cursor,
    selection: palette.selection,
    foreground: palette.foreground,
    background: palette.background,
    black: palette.normal[0],
    red: palette.normal[1],
    green: palette.normal[2],
    yellow: palette.normal[3],
    blue: palette.normal[4],
    magenta: palette.normal[5],
    cyan: palette.normal[6],
    white: palette.normal[7],
    brightBlack: palette.bright[0],
    brightRed: palette.bright[1],
    brightGreen: palette.bright[2],
    brightYellow: palette.bright[3],
    brightBlue: palette.bright[4],
    brightMagenta: palette.bright[5],
    brightCyan: palette.bright[6],
    brightWhite: palette.bright[7],
    searchHitBackground: palette.normal[3],
    searchHitBackgroundCurrent: palette.bright[3],
    searchHitForeground: palette.background,
  );
}

/// The sentence that never goes away.
class _Banner extends StatelessWidget {
  const _Banner({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: tokens.colors.bg2,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      alignment: Alignment.centerLeft,
      child: Text(
        text,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
    );
  }
}

/// What the agent is waiting for.
class _HeldStrip extends StatelessWidget {
  const _HeldStrip({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      key: const Key('sandbox-terminal-held'),
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: HColorDerivation.tint(tokens.state.held),
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      alignment: Alignment.centerLeft,
      child: Row(
        children: <Widget>[
          HStateGlyph(state: HFlowState.held, size: 14),
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            child: Text(
              text,
              style: tokens.typography.ui12.tinted(tokens.stateText.held),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ],
      ),
    );
  }
}

/// The line that says the agent ended.
class _ExitStrip extends StatelessWidget {
  const _ExitStrip({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      key: const Key('sandbox-terminal-exit'),
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: tokens.colors.bg2,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      alignment: Alignment.centerLeft,
      child: Text(
        text,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
      ),
    );
  }
}

/// A finding about this terminal, `TERM_001` above all.
///
/// `state.error` is the orange of a finding and not the red of a block: red
/// means a request did not go out, and nothing else (`docs/UX.md` rule 6).
class _FindingStrip extends StatelessWidget {
  const _FindingStrip({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      key: const Key('sandbox-terminal-finding'),
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: HColorDerivation.tint(tokens.state.error),
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      alignment: Alignment.centerLeft,
      child: Text(
        text,
        style: tokens.typography.ui12.tinted(tokens.stateText.error),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

/// Why this window has no keyboard: somebody else writes in this session.
///
/// Neutral chrome, like the line that says the agent has ended. Nothing here
/// is the state of a request -- `held` belongs to a flow that waits, and such
/// a flow has its own strip above this one -- and `docs/UX.md` 5 gives what
/// carries no state the quiet colours.
class _WatchingStrip extends StatelessWidget {
  const _WatchingStrip({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      key: const Key('sandbox-terminal-watching'),
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: tokens.colors.bg2,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
      alignment: Alignment.centerLeft,
      child: Text(
        text,
        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
    );
  }
}

/// What stands where the terminal would be while nothing runs.
class _Idle extends StatelessWidget {
  const _Idle({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      color: tokens.terminal.background,
      alignment: Alignment.center,
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Text(
        text,
        style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        textAlign: TextAlign.center,
      ),
    );
  }
}
