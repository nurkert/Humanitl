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

  /// Lets `Ctrl+1..5` leave the terminal.
  ///
  /// Everything else belongs to the agent -- including `Ctrl+C`, which reaches
  /// it as byte `0x03`. The five section shortcuts are the way out; without
  /// this the terminal would swallow them, and the only way to another screen
  /// would be the mouse (`docs/UX.md` 5.1).
  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    final bool control =
        HardwareKeyboard.instance.isControlPressed ||
        HardwareKeyboard.instance.isMetaPressed;
    if (control && navigationKeys.contains(event.logicalKey)) {
      return KeyEventResult.ignored;
    }
    return KeyEventResult.skipRemainingHandlers;
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
        if (session.diagnostic case final Diagnostic diagnostic)
          _FindingStrip(
            text: l10n.sandboxTerminalFinding(diagnostic.code, diagnostic.why),
          ),
        Expanded(child: _view(tokens, l10n, session)),
        if (session.exitCode case final int code)
          _ExitStrip(text: l10n.sandboxTerminalExited(code)),
      ],
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
      HMenuItem(label: l10n.sandboxTerminalPaste, onSelected: _paste),
    ],
    child: ColoredBox(
      color: tokens.terminal.background,
      child: TerminalView(
        session.terminal,
        controller: _controller,
        focusNode: _focus,
        autofocus: true,
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
