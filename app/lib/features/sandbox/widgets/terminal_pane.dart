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
///   title of the human belong to the human (`docs/SECURITY.md` 3.3). The
///   session even refuses the clipboard requests of OSC 52 outright
///   (`terminal_provider.dart`, HUM-313), because the emulator would
///   otherwise answer them itself. Copy and paste are actions of the human:
///   a chord, the menu, the middle button.
library;

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:xterm2/core.dart';
import 'package:xterm2/ui.dart';

import '../../../core/domain/domain.dart';
import '../../../core/shortcuts/intents.dart';
import '../../../core/ui/h_notice_layer.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/shell_glyph.dart';
import '../../../core/ui/terminal_placement.dart';
import '../../../core/ui/terminal_zoom.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/generated/app_localizations.dart';
import '../providers/terminal_provider.dart';

/// The height of the two strips above the terminal.
const double terminalStripHeight = 24;

/// Whether the emulator of a terminal on the screen has the keyboard
/// (HUM-300).
///
/// The shell asks it before the terminal changes its host: a terminal that
/// had the keyboard in its pane gets it back in the floating window, one that
/// did not leaves it where it is, with the queue. A `debugLabel` would answer
/// the same in a test and nothing in a release build.
bool terminalHoldsKeyboard() =>
    _TerminalPaneState._live.any((FocusNode node) => node.hasPrimaryFocus);

/// The physical keys of chords a terminal took, each until its release
/// (HUM-301, HUM-313, HUM-300); why they are kept is told at the pane's
/// field `_heldChordKeys`.
///
/// One instance for every terminal, with the lifetime of the provider scope:
/// it registers its release handler on [HardwareKeyboard] once and removes it
/// when the scope ends (review round 3). A release anywhere drops the key
/// after a microtask, so the terminal that took the chord still finds its own
/// release here.
class HeldChordKeys {
  /// Creates the set and registers the release handler.
  HeldChordKeys() {
    HardwareKeyboard.instance.addHandler(_dropReleased);
  }

  final Set<PhysicalKeyboardKey> _keys = <PhysicalKeyboardKey>{};

  /// Whether [key] is held for a chord.
  bool contains(PhysicalKeyboardKey key) => _keys.contains(key);

  /// Holds [key] until its release.
  void add(PhysicalKeyboardKey key) => _keys.add(key);

  /// Lets [key] go.
  void remove(PhysicalKeyboardKey key) => _keys.remove(key);

  bool _dropReleased(KeyEvent event) {
    if (event is KeyUpEvent) {
      final PhysicalKeyboardKey key = event.physicalKey;
      scheduleMicrotask(() => _keys.remove(key));
    }
    return false;
  }

  /// Removes the release handler.
  void dispose() => HardwareKeyboard.instance.removeHandler(_dropReleased);
}

/// The one [HeldChordKeys] of the application.
final Provider<HeldChordKeys> heldChordKeysProvider = Provider<HeldChordKeys>((
  Ref ref,
) {
  final HeldChordKeys held = HeldChordKeys();
  ref.onDispose(held.dispose);
  return held;
});

/// The terminal of the running session.
class TerminalPane extends ConsumerStatefulWidget {
  /// Shows the terminal of [sandboxId], or the reason there is none.
  const TerminalPane({
    required this.sandboxId,
    this.shellId = '',
    this.focusOnMount = false,
    this.floating = false,
    this.headerInset = EdgeInsets.zero,
    this.tabs,
    this.notices,
    super.key,
  });

  /// The session this pane belongs to; empty while nothing runs.
  final String sandboxId;

  /// The further shell this pane shows (HUM-314), or empty for the agent's
  /// terminal. Everything this pane does holds for a shell unchanged; only
  /// the line at its end names the shell instead of the agent.
  final String shellId;

  /// The room the banner leaves on its left and right side: the floating
  /// window keeps its resize grip free there (HUM-352). Only the horizontal
  /// sides count.
  final EdgeInsets headerInset;

  /// The row of tabs under the banner (HUM-314), or none.
  final Widget? tabs;

  /// Notices that float over the emulator (HUM-302, HUM-314), or none. The
  /// host passes them for as long as it shows tabs, not only while one
  /// stands: wrapping the emulator per notice would rebuild it on every
  /// arrival.
  final Widget? notices;

  /// Whether the terminal takes the keyboard when it is mounted (HUM-298).
  ///
  /// A host that shows the terminal because somebody asked for it -- the
  /// whole window, or the pane coming back from there -- hands the keys to
  /// the agent at once. `autofocus` does not do that: it only acts while
  /// nothing in the scope has the focus, and the shell always holds it.
  final bool focusOnMount;

  /// Whether the host is the floating window (HUM-300). Its title bar carries
  /// the buttons that put the terminal back, so the banner leaves out its own
  /// float button; the button for the whole window stays.
  final bool floating;

  @override
  ConsumerState<TerminalPane> createState() => _TerminalPaneState();
}

class _TerminalPaneState extends ConsumerState<TerminalPane> {
  /// The focus nodes of every emulator that is mounted, for
  /// [terminalHoldsKeyboard].
  static final Set<FocusNode> _live = <FocusNode>{};

  final FocusNode _focus = FocusNode(debugLabel: 'terminal');
  final TerminalController _controller = TerminalController();

  /// Der Emulator, um zu fragen, welche Zeilen er gerade zeigt (HUM-353).
  final GlobalKey<TerminalViewState> _viewKey = GlobalKey<TerminalViewState>();
  final HContextMenuController _menu = HContextMenuController();

  /// The physical keys of zoom chords (HUM-301) and clipboard chords
  /// (HUM-313) whose press this pane took, each until its own release.
  ///
  /// The activators accept the press and, for larger and smaller, its
  /// repeats; the release and the repeats of the reset, of copy and of paste
  /// they do not accept.
  /// Those events would fall through to the emulator, and an agent that asks
  /// for releases and repeats (the kitty keyboard protocol) would receive the
  /// second half of a chord whose first half it never saw. So the pane
  /// remembers the key and keeps every further event of it until the release.
  ///
  /// **One entry per key, and another key does not end it** (review round 1
  /// of HUM-313). Hold `Ctrl+Shift+V`, tap another key, release the `V`: with
  /// a single slot cleared by every press, that release went to the agent.
  ///
  /// **A loss of focus does not end it either** (review round 2). Hold
  /// `Ctrl+Shift+V`, let the focus go elsewhere, bring it back by hovering
  /// over the terminal (HUM-303), then release the `V`: had the set been
  /// cleared on the way, that release reached the agent. A release that went
  /// to another node meanwhile leaves the key in the set until its next
  /// press, and a new press of a key still in it counts as a new press.
  ///
  /// **Every chord of the shell, and one set for every host** (HUM-300,
  /// review round 1). `Ctrl+Shift+P` lets the terminal float or puts it back,
  /// so the release of the `P` arrives in the emulator of the *other* host;
  /// with a set per pane that release reached the agent. The set is shared
  /// by all panes for that reason, and it takes the press of every chord of
  /// the shell the pane hands on, not only the zoom.
  ///
  /// **A release anywhere ends it** (HUM-300, review round 2). The release of
  /// a chord key can land outside every terminal -- the palette opened by
  /// `Ctrl+K` holds the keyboard when the `K` comes up. Left in the set, the
  /// key swallowed the repeats of a later press that began elsewhere and
  /// reached the terminal by hover. A handler on [HardwareKeyboard] drops the
  /// key on every release. Those handlers run *before* the focus system hands
  /// the event to the terminal, so the drop waits for a microtask: the
  /// terminal still finds its own release in the set and keeps it from the
  /// agent, and the next event of the platform finds the set clean.
  ///
  /// The set and that handler live in [heldChordKeysProvider], not in a pane
  /// (review round 3): with no pane mounted -- the floating window hidden for
  /// lack of room, the old pane gone -- a handler owned by panes was gone too.
  late final HeldChordKeys _heldChordKeys;

  @override
  void initState() {
    super.initState();
    _live.add(_focus);
    _heldChordKeys = ref.read(heldChordKeysProvider);
    _attach();
    if (widget.focusOnMount) {
      _takeFocus();
    }
    // `Ctrl+Backquote` and the palette (HUM-303). Every host listens; the
    // rule below lets only the one that is shown act.
    ref.listenManual<int>(
      terminalFocusRequestProvider,
      (int? previous, int next) => _takeKeyboard(byHover: false),
    );
  }

  /// Whether the emulator may take the keyboard now, asked by the pointer
  /// ([byHover]) or by `Ctrl+Backquote` (HUM-303).
  ///
  /// **The pointer decides where the keys go, with two exceptions, and both
  /// keep the focus where it is.**
  ///
  /// * **A layer above the work.** A dialog, a sheet, a menu or the command
  ///   palette each hold the keyboard in a focus scope of their own
  ///   (`HModal`, `HSheet`, the menu of `HContextMenu`). Their scope is never
  ///   one the terminal stands in, so the rule needs no list of them: the
  ///   terminal takes the focus only from a node whose scope is its own or one
  ///   of the scopes around it. The menu of the terminal itself is the case
  ///   that shows why: it opens under the pointer, over the terminal, and
  ///   lets the pointer through; without this rule the next movement would
  ///   take the keys from it, and `Esc` would cancel the agent instead of
  ///   closing the menu. This holds for the shortcut too.
  /// * **A text field somebody types in**, only for the pointer. Crossing the
  ///   terminal on the way to a button must not cut a note or a filter in
  ///   half. Whoever wants the terminal then clicks it or presses the
  ///   shortcut, which asks deliberately.
  ///
  /// **No delay.** The obvious alternative, focus only after the pointer
  /// rested 150 ms, was weighed and dropped: it leaves a moment in which the
  /// pointer is on the terminal and the keyboard is still in the queue, and
  /// an `Enter` typed for the agent in that moment would allow a held
  /// request. Taking the focus at once closes that gap; the text-field rule
  /// covers what the delay would have protected.
  bool _mayTakeKeyboard({required bool byHover}) {
    if (!mounted || widget.sandboxId.isEmpty || !Visibility.of(context)) {
      return false;
    }
    if (_focus.hasPrimaryFocus || HLayerScope.keyboardClaimed) {
      return false;
    }
    final FocusScopeNode? own = _focus.enclosingScope;
    if (own == null) {
      // Not attached: there is no emulator on the screen to give it to.
      return false;
    }
    final FocusNode? holder = FocusManager.instance.primaryFocus;
    if (holder != null) {
      final FocusScopeNode? held = holder.nearestScope;
      if (held != own && !own.ancestors.contains(held)) {
        return false;
      }
    }
    return !(byHover && isTextInputFocused());
  }

  /// Hands the keyboard to the emulator when [_mayTakeKeyboard] allows it.
  void _takeKeyboard({required bool byHover}) {
    if (_mayTakeKeyboard(byHover: byHover)) {
      _focus.requestFocus();
    }
  }

  void _hovered(PointerEvent _) => _takeKeyboard(byHover: true);

  /// Gives the keyboard to the emulator one frame from now, once it is built.
  ///
  /// Within the same rule as the pointer and the shortcut: a layer that holds
  /// the keyboard keeps it (HUM-303). Opening the settings from the terminal
  /// over the whole window puts the terminal back into its pane, and the
  /// pane hands it the keyboard as soon as the session is known again --
  /// which is after the sheet has taken it. Without the rule every key typed
  /// into the sheet went to the agent behind it.
  void _takeFocus() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && Visibility.of(context)) {
        _takeKeyboard(byHover: false);
      }
    });
  }

  @override
  void didUpdateWidget(TerminalPane oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.sandboxId != widget.sandboxId ||
        oldWidget.shellId != widget.shellId) {
      _attach();
      // A host that asked for the keyboard before any session ran -- the
      // whole window over the empty state -- gets it when the session
      // arrives; until then there was no emulator to give it to (HUM-298).
      if (widget.focusOnMount && oldWidget.sandboxId.isEmpty) {
        _takeFocus();
      }
    }
  }

  @override
  void dispose() {
    _live.remove(_focus);
    _focus.dispose();
    _controller.dispose();
    super.dispose();
  }

  /// The session of this pane: the agent's terminal or a further shell's.
  TerminalSessionProvider get _session =>
      terminalSessionProvider(widget.sandboxId, shellId: widget.shellId);

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
      unawaited(ref.read(_session.notifier).attach());
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
  /// noch mit der Maus heraus (`docs/UX.md` 5.1). Dasselbe gilt für
  /// `Ctrl+Shift+Enter`, das Terminal über das ganze Fenster und zurück
  /// (HUM-298): Es steht in [shellShortcuts] und nimmt deshalb denselben Weg.
  /// Ebenso `Ctrl+Backquote`, die Tastatur ans Terminal (HUM-303): Steht der
  /// Fokus schon hier, tut es nichts, und der Agent bekommt kein Byte.
  /// Und die Zoom-Kürzel (HUM-301): `Ctrl` mit `+`, `=` oder `-`, mit dem
  /// Plus, Minus oder der Null des Ziffernblocks, und `Ctrl+Shift+0`.
  /// Dazu Kopieren und Einfügen (HUM-313), `Ctrl+Shift+C` und `Ctrl+Shift+V`,
  /// die nicht in [shellShortcuts] stehen und nur hier gelten. Dazu
  /// `Ctrl+C`, solange etwas sichtbar ausgewählt ist (HUM-353): Es kopiert.
  /// Alles andere gehört ihm, `Ctrl+C` ohne Auswahl als `0x03`, `Ctrl+V` als
  /// `0x16`,
  /// `Ctrl+P` als `0x10` und
  /// `Esc` eingeschlossen, das OpenCode und pi zum Abbrechen brauchen.
  ///
  /// Hört an dieser Stelle des Baums niemand auf eine Absicht -- im Test etwa,
  /// der die Kachel ohne Schale zeigt --, bleibt die Taste beim Agenten.
  /// Die Kürzel, die der Emulator selbst behandelt: keine (HUM-313).
  ///
  /// Seine Vorgabe bindet drei, und keines davon bleibt. **Alles auswählen**
  /// (`Ctrl+A`) ist in readline und in jedem Programm auf bubbletea -- also
  /// auch in OpenCode -- der Sprung an den Zeilenanfang; das Kontextmenü
  /// wählt stattdessen alles aus. **Einfügen** liegt dort auf `Ctrl+V`, und
  /// das ist in readline das wörtliche Einfügen des nächsten Zeichens
  /// (`0x16`); in jedem Terminal unter Linux fügt erst `Ctrl+Shift+V` ein.
  /// Außerdem fragt seine Einfügen-Handlung nicht, ob die Ansicht nur liest.
  /// **Kopieren** (`Ctrl+Shift+C`) wäre richtig gebunden, aber Kopieren und
  /// Einfügen gehören zusammen an eine Stelle: [_onKey] fragt
  /// [TerminalClipboardActivator], auf Apple-Plattformen mit `Cmd`.
  ///
  /// Der Emulator prüft seine Kürzel vor jeder Übersetzung in Bytes
  /// (`TerminalView._handleKeyEvent`), also hilft nichts als die leere
  /// Tabelle; `null` hieße „nimm deine Vorgabe".
  static const Map<ShortcutActivator, Intent> _emulatorShortcuts =
      <ShortcutActivator, Intent>{};

  /// Ob [event] das schlichte `Ctrl+C` ist, das über einer Auswahl kopiert
  /// (HUM-353).
  ///
  /// Nicht auf Apple-Plattformen: Dort kopiert `Cmd+C`
  /// ([TerminalClipboardActivator]), und `Ctrl+C` ist wie in jedem Terminal
  /// von macOS immer die Unterbrechung.
  static bool _isSelectionCopyChord(KeyEvent event) {
    if (event is! KeyDownEvent || event.logicalKey != LogicalKeyboardKey.keyC) {
      return false;
    }
    if (defaultTargetPlatform == TargetPlatform.macOS ||
        defaultTargetPlatform == TargetPlatform.iOS) {
      return false;
    }
    final HardwareKeyboard keyboard = HardwareKeyboard.instance;
    return keyboard.isControlPressed &&
        !keyboard.isShiftPressed &&
        !keyboard.isAltPressed &&
        !keyboard.isMetaPressed;
  }

  /// Ob im Puffer, den das Terminal gerade zeigt, eine Auswahl **sichtbar**
  /// ist (HUM-353); der Puffer ist derselbe wie in [_copy].
  ///
  /// Eine Auswahl überlebt im Emulator Tippen und Ausgabe. Ist sie aus dem
  /// Bild gerollt, hat der Mensch sie vergessen, und ein `Ctrl+C` kopierte
  /// still unsichtbaren Text, statt den Agenten zu unterbrechen. Eine solche
  /// Auswahl hebt diese Funktion deshalb auf und antwortet `false`; das
  /// `0x03` geht dann durch. Sichtbar heißt: Mindestens eine ihrer Zeilen
  /// liegt im Fenster, auch nur angeschnitten.
  bool _hasVisibleSelection() {
    final BufferRange? selection = _controller.selectionFor(
      ref.read(_session).terminal.buffer,
    );
    if (selection == null) {
      return false;
    }
    // `RenderTerminal` exportiert `xterm2` nicht; der Typ bleibt erschlossen.
    final view = _viewKey.currentState?.renderTerminal;
    if (view == null) {
      return false;
    }
    final int top = view.getCellOffset(Offset.zero).y;
    final int bottom = view.getCellOffset(Offset(0, view.size.height - 1)).y;
    final BufferRange range = selection.normalized;
    if (range.begin.y <= bottom && range.end.y >= top) {
      return true;
    }
    _controller.clearSelection();
    return false;
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent event) {
    if (event is KeyDownEvent) {
      _heldChordKeys.remove(event.physicalKey);
    }
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
      if (event is KeyDownEvent) {
        _heldChordKeys.add(event.physicalKey);
      }
      Actions.maybeInvoke<Intent>(context, binding.value);
      return KeyEventResult.handled;
    }
    // Kopieren und Einfügen (HUM-313), nur hier und nicht in der Schale:
    // Außerhalb des Terminals bleiben die Griffe frei. Der Griff gehört der
    // Anwendung auch dann, wenn er gerade nichts tun kann -- ohne Auswahl,
    // oder während die Ansicht nur liest --, denn sonst meldete der Emulator
    // einem Agenten mit dem Tastaturprotokoll von kitty die Taste als eigene
    // Folge (`CSI 99;6 u`), mitten in einem Griff, der nicht ihm galt.
    for (final TerminalClipboard action in TerminalClipboard.values) {
      if (TerminalClipboardActivator(action)
          .accepts(event, HardwareKeyboard.instance)) {
        _heldChordKeys.add(event.physicalKey);
        switch (action) {
          case TerminalClipboard.copy:
            _copy();
          case TerminalClipboard.paste:
            _paste();
        }
        return KeyEventResult.handled;
      }
    }
    // `Ctrl+C` über einer sichtbaren Auswahl kopiert sie (HUM-353), wie
    // `Ctrl+Shift+C`, und hebt sie danach auf. Der Agent bekommt dann kein
    // `0x03`: Wer markiert und `Ctrl+C` drückt, will kopieren, und OpenCode
    // beendete sich in seiner Vorgabe auf genau diese Taste. Ohne sichtbare
    // Auswahl bleibt `Ctrl+C` das `0x03` des Agenten, ein zweiter Druck nach
    // dem Kopieren also wieder die Unterbrechung.
    if (_isSelectionCopyChord(event) && _hasVisibleSelection()) {
      _heldChordKeys.add(event.physicalKey);
      _copy();
      _controller.clearSelection();
      return KeyEventResult.handled;
    }
    // Der Rest eines Griffs, den diese Ansicht genommen hat: die
    // Wiederholungen und das Loslassen (HUM-301, HUM-313). Der Griff gilt
    // dabei nicht noch einmal.
    if (_heldChordKeys.contains(event.physicalKey)) {
      if (event is KeyUpEvent) {
        _heldChordKeys.remove(event.physicalKey);
      }
      return KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = AppLocalizations.of(context);
    if (widget.sandboxId.isEmpty) {
      // Auch ohne Sitzung steht die Kopfzeile mit dem Knopf da: Wer das
      // Terminal über das ganze Fenster geholt hat, kommt auch ohne laufende
      // Sitzung mit der Maus zurück (HUM-298).
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          _Banner(
            text: l10n.sandboxTerminalUntrustedBanner,
            floating: widget.floating,
            running: false,
            inset: widget.headerInset,
          ),
          Expanded(child: _Idle(text: l10n.sandboxTerminalIdle)),
        ],
      );
    }
    final TerminalSessionState session = ref.watch(_session);
    // Der Abriss steht als letzte Zeile im Terminal selbst, dort, wo der
    // Blick ist (HUM-058, Zeile 19): einmal je Abriss, in der gedämpften
    // Farbe der Palette (SGR 90), und der Rücklauf darüber bleibt. Er ist
    // eine Zeile dieser Anwendung und kein Byte des Agenten; der Agent sieht
    // sie nie.
    ref.listen<TerminalSessionState>(_session, (
      TerminalSessionState? previous,
      TerminalSessionState next,
    ) {
      if (next.reconnecting && !(previous?.reconnecting ?? false)) {
        next.terminal.write(
          '\r\n\x1b[90m${l10n.sandboxTerminalReconnecting}\x1b[0m\r\n',
        );
      }
    });
    final TerminalNotice? notice = ref.watch(heldNoticeProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _Banner(
          text: l10n.sandboxTerminalUntrustedBanner,
          floating: widget.floating,
          running: true,
          inset: widget.headerInset,
        ),
        if (notice != null)
          _HeldStrip(
            text: l10n.sandboxTerminalHeldNotice(
              notice.method,
              notice.host,
              notice.path,
            ),
          ),
        ?widget.tabs,
        if (_strip(l10n, session) case final Widget strip) strip,
        Expanded(
          child: switch (widget.notices) {
            final Widget notices => HNoticeLayer(
              // Never over the bottom rows, where the human types.
              maxHeightFor: (double region) => region / 2,
              notices: notices,
              child: _view(tokens, l10n, session),
            ),
            null => _view(tokens, l10n, session),
          },
        ),
        // Das Ende bekommt einen sichtbaren Platz, und zwar jedes Ende
        // (HUM-136). Der Exit-Code zuerst, weil er die genaueste Auskunft
        // ist; danach, was über das Ende des Stroms zu sagen ist.
        if (session.exitCode case final int code)
          _ExitStrip(
            text: widget.shellId.isEmpty
                ? l10n.sandboxTerminalExited(code)
                : l10n.sandboxTerminalShellExited(code),
          )
        else if (session.phase == TerminalPhase.detached)
          _ExitStrip(
            text: _endText(l10n, session),
            stripKey: const Key('sandbox-terminal-detached'),
          ),
      ],
    );
  }

  /// Was unter dem Terminal steht, wenn der Strom endete (HUM-136).
  ///
  /// **Zwei Sätze, weil zwei verschiedene Dinge passiert sind, und einer
  /// davon wäre im anderen Fall eine Behauptung über etwas, das diese
  /// Anwendung nicht wissen kann** (`backlog/CONVENTIONS.md` 4.13).
  /// `DAEMON_001` heißt: Es antwortet überhaupt kein Daemon mehr -- und die
  /// Sandbox lebt in genau diesem Daemon. „Die Sitzung läuft weiter" wäre
  /// dann eine Beruhigung ohne Deckung; was dort steht, ist deshalb, dass der
  /// Zustand der Sitzung unbekannt ist. Endete nur der Strom, während der
  /// Daemon weiter antwortet, bleibt der Satz für diesen Fall -- auch der
  /// verspricht nicht, dass die Sitzung lebt, sondern nennt die Bedingung.
  String _endText(AppLocalizations l10n, TerminalSessionState session) =>
      session.diagnostic?.code == DiagnosticCodes.daemonUnreachable
      ? l10n.sandboxTerminalConnectionLost
      : l10n.sandboxTerminalDetached;

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
        // The same question as [_copy]: a selection of the other buffer
        // (main or alternate screen) is no selection here.
        enabled: _controller.selectionFor(session.terminal.buffer) != null,
        onSelected: _copy,
        shortcut: _apple ? null : l10n.sandboxTerminalCopyKey,
      ),
      // Einfügen geht am Rohmodus vorbei: `Terminal.paste` ruft `onOutput`
      // auch dann, wenn die Ansicht nur liest, und die Bytes gingen hinauf,
      // wo der Daemon sie verwirft. Ein Menüpunkt, der nichts tut, ist genau
      // die Lüge, die der Kommentar am Cursor drei Zeilen tiefer ausschließt.
      HMenuItem(
        label: l10n.sandboxTerminalPaste,
        enabled: _mayPaste(session),
        onSelected: _paste,
        shortcut: _apple ? null : l10n.sandboxTerminalPasteKey,
      ),
      // Ersetzt `Ctrl+A` des Emulators, das dem Agenten gehört (HUM-313).
      HMenuItem(label: l10n.sandboxTerminalSelectAll, onSelected: _selectAll),
    ],
    // The pointer over the emulator hands it the keyboard, within the rule of
    // [_mayTakeKeyboard] (HUM-303). On entering, and on every movement
    // after: a text field that had the focus when the pointer came in may
    // have let go of it since.
    child: _WheelZoom(
      onZoom: _wheelZoomed,
      child: MouseRegion(
        onEnter: _hovered,
        onHover: _hovered,
        child: Listener(
          onPointerDown: _middleClicked,
          child: ColoredBox(
            color: tokens.terminal.background,
            child: TerminalView(
              session.terminal,
              key: _viewKey,
              controller: _controller,
              focusNode: _focus,
              autofocus: true,
              shortcuts: _emulatorShortcuts,
              theme: _theme(tokens.terminal),
              textStyle: TerminalStyle(
                // `mono13` until somebody zooms (HUM-301). A new size changes
                // the cells, the emulator counts columns and rows again and
                // reports them through `onResize`, and the session sends them
                // up on its throttled path (HUM-305): the agent redraws.
                fontSize: ref.watch(terminalFontSizeProvider),
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
              readOnly:
                  session.readOnly || session.phase != TerminalPhase.attached,
            ),
          ),
        ),
      ),
    ),
  );

  /// `Ctrl` + wheel over the emulator: up is larger, down smaller (HUM-301).
  void _wheelZoomed(bool larger) {
    final TerminalFontSize size = ref.read(terminalFontSizeProvider.notifier);
    larger ? size.zoomIn() : size.zoomOut();
  }

  /// Whether the menu and the chord name `Cmd` instead of `Ctrl+Shift`.
  bool get _apple => switch (defaultTargetPlatform) {
    TargetPlatform.iOS || TargetPlatform.macOS => true,
    _ => false,
  };

  /// Whether pasted text would reach the agent: only a writer of an attached
  /// session. Otherwise the daemon drops the bytes (see the menu).
  bool _mayPaste(TerminalSessionState session) =>
      !session.readOnly && session.phase == TerminalPhase.attached;

  /// Copies the selection into the clipboard of the human (HUM-042, HUM-313).
  ///
  /// The one way text leaves the terminal for the host: a chord, a menu entry,
  /// each an action of the human. A selection alone copies nothing -- Flutter
  /// on Linux has no primary selection to put it in, and the clipboard is
  /// not the place for every drag of the mouse. Trailing blanks of each line
  /// are dropped, as every terminal does; the emulator pads lines with them.
  void _copy() {
    final Buffer buffer = ref.read(_session).terminal.buffer;
    final BufferRange? selection = _controller.selectionFor(buffer);
    if (selection == null) {
      return;
    }
    unawaited(
      Clipboard.setData(ClipboardData(text: buffer.getText(selection, true))),
    );
  }

  /// Pastes the clipboard as input for the agent (HUM-042, HUM-313).
  ///
  /// [Terminal.paste] does the two things a paste needs and typing does not.
  /// While the program asked for bracketed paste (`DECSET 2004`), the text
  /// goes up between `ESC [200~` and `ESC [201~`, so a shell or an agent takes
  /// several lines as one input instead of running them line by line. And it
  /// drops every escape sequence of the pasted text, `ESC [201~` included,
  /// which would end the bracket early and turn the rest into typed commands,
  /// and replaces the control bytes that a terminal acts on (`0x03`, `0x1a`,
  /// ...) by a blank. Typing the text instead ([Terminal.textInput]) would
  /// skip both.
  void _paste() {
    // The session the human pasted into. The state of this pane outlives a
    // change of sandbox or shell (`didUpdateWidget`), so the answer of the
    // clipboard may arrive while another session is shown; it then goes
    // nowhere.
    final TerminalSessionProvider pasted = _session;
    if (!_mayPaste(ref.read(pasted))) {
      return;
    }
    unawaited(
      Clipboard.getData(Clipboard.kTextPlain).then((ClipboardData? data) {
        final String? text = data?.text;
        if (text == null || text.isEmpty || !mounted) {
          return;
        }
        if (pasted != _session) {
          return;
        }
        // Asked again: the clipboard answers later, and meanwhile the session
        // may have ended or handed the keyboard to somebody else.
        final TerminalSessionState session = ref.read(pasted);
        if (_mayPaste(session)) {
          session.terminal.paste(text);
        }
      }),
    );
  }

  /// Selects the whole buffer, scrollback included (HUM-313).
  ///
  /// The menu entry that replaces `Ctrl+A` of the emulator, which belongs to
  /// the agent. Selecting changes nothing outside the terminal; copying it is
  /// a step of its own.
  void _selectAll() {
    final Buffer buffer = ref.read(_session).terminal.buffer;
    _controller.setSelection(
      buffer.createAnchor(0, 0),
      buffer.createAnchor(buffer.viewWidth, buffer.height - 1),
      mode: SelectionMode.line,
    );
  }

  /// The middle button pastes, as on every Linux desktop (HUM-313).
  ///
  /// What it pastes is the clipboard: the primary selection, which X11 and
  /// Wayland paste there, is not reachable from Flutter. Only while the
  /// program in the terminal does not take the mouse itself: a full-screen
  /// TUI with mouse reporting gets the click as its own (the emulator sends
  /// it), unless `Shift` is held and the program did not claim that too --
  /// the same rule the emulator applies to its own clicks.
  void _middleClicked(PointerDownEvent event) {
    if (event.kind != PointerDeviceKind.mouse ||
        event.buttons != kMiddleMouseButton) {
      return;
    }
    final Terminal terminal = ref.read(_session).terminal;
    final bool programTakesTheMouse =
        terminal.mouseMode != MouseMode.none &&
        !(HardwareKeyboard.instance.isShiftPressed &&
            !terminal.mouseShiftCaptureMode);
    if (!programTakesTheMouse) {
      _paste();
    }
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

/// The sentence that never goes away, and the button that moves the terminal.
///
/// This strip is the header of the terminal wherever it stands: in its pane
/// and over the whole window, where it is the one row left above the
/// emulator (HUM-298). The buttons sit at its right end, so they are in the
/// same place in every layout and a second click undoes the first: the
/// floating window (HUM-300) and the whole window. In the floating window
/// its own title bar puts the terminal back, so only the whole window stays
/// here.
class _Banner extends StatelessWidget {
  const _Banner({
    required this.text,
    required this.floating,
    required this.running,
    required this.inset,
  });

  final String text;

  /// The room left on the sides (`TerminalPane.headerInset`).
  final EdgeInsets inset;

  /// Whether the host is the floating window.
  final bool floating;

  /// Whether a session runs in this terminal; without one it does not float.
  final bool running;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      constraints: const BoxConstraints(minHeight: terminalStripHeight),
      color: tokens.colors.bg2,
      padding: EdgeInsets.only(
        left: tokens.spacing.x2 + inset.left,
        right: inset.right,
      ),
      child: Row(
        children: <Widget>[
          Expanded(
            child: Text(
              text,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
          if (!floating) FloatButton(running: running),
          const FullWindowButton(),
        ],
      ),
    );
  }
}

/// Lets the terminal float over the sections, or puts it back into its pane
/// (HUM-300).
///
/// The same as `Ctrl+Shift+P`, and the tooltip says so. Disabled while no
/// agent runs: a floating window over the empty state would show nothing
/// worth watching and close itself again. It writes the placement and nothing
/// else; the shell lays the window out from it.
class FloatButton extends ConsumerWidget {
  /// Creates the button; [running] says whether a session runs.
  const FloatButton({required this.running, super.key});

  /// Whether an agent runs in the terminal. A floating terminal can always
  /// be put back, also after its agent ended.
  final bool running;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool floating =
        ref.watch(terminalPlacementProvider) == TerminalPlacement.floating;
    final String label = floating
        ? l10n.sandboxTerminalFloatLeave
        : l10n.sandboxTerminalFloatEnter;
    return HoverLabel(
      label: label,
      side: HoverLabelSide.below,
      child: HButton(
        key: const Key('sandbox-terminal-float'),
        variant: HButtonVariant.ghost,
        size: HButtonSize.sm,
        semanticsLabel: label,
        onPressed: floating || running
            ? ref.read(terminalPlacementProvider.notifier).toggleFloating
            : null,
        child: ShellGlyphIcon(
          ShellGlyph.pictureInPicture,
          size: 14,
          color: floating || running ? tokens.colors.fg1 : tokens.colors.fg2,
        ),
      ),
    );
  }
}

/// Shows the terminal over the whole window, or puts it back (HUM-298).
///
/// The same as `Ctrl+Shift+Enter`, and the tooltip says so, so the click also
/// teaches the chord. It writes the placement and nothing else; the shell
/// lays the window out from it.
class FullWindowButton extends ConsumerWidget {
  /// Creates the button.
  const FullWindowButton({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = AppLocalizations.of(context);
    final bool full =
        ref.watch(terminalPlacementProvider) == TerminalPlacement.fullWindow;
    final String label = full
        ? l10n.sandboxTerminalFullWindowLeave
        : l10n.sandboxTerminalFullWindowEnter;
    // Below and not to the right: the button stands at the right end of the
    // pane, and over the whole window at the edge of the window.
    return HoverLabel(
      label: label,
      side: HoverLabelSide.below,
      child: HButton(
        key: const Key('sandbox-terminal-full-window'),
        variant: HButtonVariant.ghost,
        size: HButtonSize.sm,
        semanticsLabel: label,
        onPressed: ref
            .read(terminalPlacementProvider.notifier)
            .toggleFullWindow,
        child: ShellGlyphIcon(
          full ? ShellGlyph.minimize : ShellGlyph.maximize,
          size: 14,
          color: tokens.colors.fg1,
        ),
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
  const _ExitStrip({
    required this.text,
    this.stripKey = const Key('sandbox-terminal-exit'),
  });

  final String text;

  /// Woran ein Test diesen Streifen erkennt. Zwei Enden, zwei Schlüssel.
  final Key stripKey;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Container(
      key: stripKey,
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

/// Turns `Ctrl` + wheel over its child into a zoom of the terminal text
/// (HUM-301), and keeps the wheel from scrolling the scrollback meanwhile.
///
/// **Why not a `Listener`.** Every `Scrollable` inside the emulator -- the
/// scrollback and, for a full-screen TUI, the one that turns the wheel into
/// mouse reports -- registers each wheel event with the
/// `PointerSignalResolver`, and the first to register wins it. The event is
/// handed along the hit-test path from the innermost box outwards, so a
/// `Listener` around the emulator registers last and loses: `Ctrl` + wheel
/// would scroll instead of zooming. This box enters itself into the path
/// before its children and thus registers first, and only for a wheel with
/// `Ctrl` held. Every other wheel event it leaves alone.
class _WheelZoom extends SingleChildRenderObjectWidget {
  const _WheelZoom({required this.onZoom, required super.child});

  /// Called with `true` for a step larger, `false` for a step smaller.
  final ValueChanged<bool> onZoom;

  @override
  RenderObject createRenderObject(BuildContext context) =>
      _RenderWheelZoom(onZoom);

  @override
  void updateRenderObject(BuildContext context, _RenderWheelZoom renderObject) {
    renderObject.onZoom = onZoom;
  }
}

class _RenderWheelZoom extends RenderProxyBox {
  _RenderWheelZoom(this.onZoom);

  ValueChanged<bool> onZoom;

  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (!size.contains(position)) {
      return false;
    }
    // Before the children, not after them as every proxy does: the order of
    // the path is the order in which the resolver hears the candidates.
    result.add(BoxHitTestEntry(this, position));
    hitTestChildren(result, position: position);
    return true;
  }

  @override
  void handleEvent(PointerEvent event, BoxHitTestEntry entry) {
    if (event is! PointerScrollEvent ||
        !HardwareKeyboard.instance.isControlPressed) {
      return;
    }
    final double delta = event.scrollDelta.dy;
    if (delta == 0) {
      return;
    }
    GestureBinding.instance.pointerSignalResolver.register(
      event,
      (PointerSignalEvent _) => onZoom(delta < 0),
    );
  }
}
