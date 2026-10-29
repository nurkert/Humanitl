/// The shell: header, rail, sections, status bar, palette and the shortcuts
/// that bind them (HUM-019).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/config_providers.dart';
import '../../core/ipc/flow_handoff.dart';
import '../../core/ipc/flow_reveal.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/settings_opener.dart';
import '../../core/ui/terminal_placement.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import '../about/about_dialog.dart';
import '../audit/audit_screen.dart';
import '../editor/editor_host.dart';
import '../history/history_screen.dart';
import '../intercept/intercept_screen.dart';
import '../intercept/providers/decision.dart';
import '../intercept/providers/flows.dart';
import '../rules/rules_screen.dart';
import '../sandbox/providers/sandbox_status_provider.dart';
import '../sandbox/sandbox_screen.dart';
import '../sandbox/widgets/sandbox_terminal.dart';
import '../settings/settings_sheet.dart';
import '../setup/providers/setup_provider.dart';
import 'providers/connection.dart';
import 'providers/language.dart';
import 'providers/navigation.dart';
import 'providers/rail.dart';
import 'providers/setup_state.dart';
import 'providers/theme.dart';
import 'section.dart';
import 'widgets/command_palette.dart';
import 'widgets/frozen_banner.dart';
import 'widgets/frozen_sections.dart';
import 'widgets/header_bar.dart';
import 'widgets/icon_rail.dart';
import 'widgets/setup_host.dart';
import 'widgets/shell_notices.dart';
import 'widgets/status_bar.dart';

/// The shell, shown once the daemon answered.
class ShellScreen extends ConsumerStatefulWidget {
  /// Creates the shell for the connected [info].
  const ShellScreen({required this.info, super.key});

  /// What `GetInfo` said; the status bar shows it.
  final DaemonInfo info;

  @override
  ConsumerState<ShellScreen> createState() => _ShellScreenState();
}

class _ShellScreenState extends ConsumerState<ShellScreen> {
  final FocusNode _focus = FocusNode(debugLabel: 'shell');
  bool _paletteOpen = false;

  /// Whether the settings sheet is open (HUM-069). A sheet and not a section:
  /// the rail stays the five places of the work plus setup, and the settings
  /// lie over whichever of them is showing (`BACKLOG.md` 5, "Sheet").
  bool _settingsOpen = false;

  /// The key whose field the open sheet scrolls to and focuses, and how often
  /// that was asked for (HUM-275).
  String? _settingsFocusKey;
  int _settingsFocusSerial = 0;

  /// Whether the setup screen has already had its one chance to open itself.
  ///
  /// It gets exactly one: at startup, as soon as the four checks have settled,
  /// and only when they are not all green. After that the section belongs to
  /// the person -- a screen that pulled somebody out of the queue because a
  /// check went yellow ten minutes later would take the work away to show a
  /// checklist (`docs/UX.md` 2.8).
  bool _setupOffered = false;

  /// Ein Schlüssel je Daten-Abschnitt, damit ein Bruch der Verbindung ihre
  /// Elemente umhängt statt sie zu zerstören. Felder und keine Ausdrücke in
  /// `build`: Ein neuer Schlüssel je Frame hielte gar nichts.
  final List<GlobalKey> _keys = <GlobalKey>[
    for (int i = 0; i < 5; i++) GlobalKey(debugLabel: 'section-$i'),
  ];

  /// The focus scope around the terminal over the whole window (HUM-298).
  final FocusScopeNode _terminalScope = FocusScopeNode(
    debugLabel: 'terminal-full-window',
  );

  @override
  void initState() {
    super.initState();
    // Die Einstellungen schreiben den Projektordner über `configProvider`,
    // der Sandbox-Schnappschuss plant mit ihm. Kein Feature importiert ein
    // anderes; die Shell hält beide und reicht jeden lesbaren Ordner weiter,
    // damit Kopf und Setup den neuen zeigen, ohne dass der Bildschirm
    // wechselt (HUM-295). Sofort einmal, damit der Ordner beim Öffnen der
    // Shell als Ausgangspunkt gilt und nicht als Änderung.
    ref.listenManual<String?>(
      configProvider.select(
        (AsyncValue<ConfigValues> config) => config.value?.sandboxWorkDir,
      ),
      (String? _, String? workDir) {
        if (workDir != null) {
          ref.read(sandboxStatusProvider.notifier).storedWorkDirRead(workDir);
        }
      },
      fireImmediately: true,
    );
  }

  @override
  void dispose() {
    _focus.dispose();
    _terminalScope.dispose();
    super.dispose();
  }

  void _openPalette() => setState(() => _paletteOpen = true);

  void _closePalette() {
    setState(() => _paletteOpen = false);
    // The palette's field held the focus; without this the next Ctrl+2 would
    // reach nobody. Over the whole window the keys go back to the terminal
    // instead (HUM-298): the scope around it remembers which of its nodes had
    // the focus, and the shell's shortcuts stay above it either way.
    if (ref.read(terminalPlacementProvider) == TerminalPlacement.fullWindow) {
      _terminalScope.requestFocus();
    } else {
      _focus.requestFocus();
    }
  }

  void _togglePalette() => _paletteOpen ? _closePalette() : _openPalette();

  void _openSettings() {
    ref.read(terminalPlacementProvider.notifier).leaveFullWindow();
    setState(() {
      _settingsOpen = true;
      _settingsFocusKey = null;
    });
  }

  /// Opens the sheet at the field of [key], or plain without one: the way a
  /// diagnostic's `ChangeSetting` fix reaches the sheet (HUM-275).
  ///
  /// The sheet hangs in the layout with the rail, so a terminal over the
  /// whole window steps back first (HUM-298).
  void _openSettingsAt(String? key) {
    ref.read(terminalPlacementProvider.notifier).leaveFullWindow();
    setState(() {
      _settingsOpen = true;
      _settingsFocusKey = key;
      _settingsFocusSerial++;
    });
  }

  void _closeSettings() {
    setState(() {
      _settingsOpen = false;
      _settingsFocusKey = null;
    });
    // The sheet held the focus; without this `Ctrl+K` would reach nobody.
    _focus.requestFocus();
  }

  /// Die Befehle der Palette, für eine Verbindung, die [live] ist oder nicht.
  ///
  /// Die Palette liegt als Geschwister der eingefrorenen Fläche im [Stack],
  /// also erreicht `Ctrl+K` sie auch während eines Bruchs -- und das soll so
  /// sein, denn `reconnect` steht darin. Was die Warteschlange anfasst, steht
  /// dann aber nicht darin: `queue-allow-all` setzte
  /// [batchConfirmProvider], und dessen Modal wird innerhalb der
  /// eingefrorenen Fläche gezeichnet. Sein Bestätigen wäre still, sein Schirm
  /// wäre still und `Escape` wäre still; es stünde, bis die Verbindung
  /// zurückkommt. Ein Befehl, der nichts tut, gehört nicht in die Liste
  /// (`docs/UX.md` 5.3).
  List<PaletteCommand> _commands(BuildContext context, {required bool live}) {
    final AppLocalizations l10n = context.l10n;
    // Die andere der beiden Sprachen, gemessen an der, in der das Fenster
    // gerade spricht, und nicht an der Wahl: Ohne Wahl spricht es die des
    // Desktops (HUM-052).
    final AppLanguage otherLanguage =
        AppLanguage.fromLocale(Localizations.localeOf(context)) ==
            AppLanguage.de
        ? AppLanguage.en
        : AppLanguage.de;
    return <PaletteCommand>[
      for (final Section section in Section.values)
        PaletteCommand(
          id: 'go-${section.name}',
          label: l10n.shellPaletteGoTo(section.label(l10n)),
          run: () => _go(section),
        ),
      PaletteCommand(
        id: 'toggle-theme',
        label: l10n.shellPaletteToggleTheme,
        run: () => ref
            .read(themeModeProvider.notifier)
            .toggle(MediaQuery.platformBrightnessOf(context)),
      ),
      PaletteCommand(
        id: 'switch-language',
        label: l10n.shellPaletteSwitchLanguage(otherLanguage.endonym(l10n)),
        run: () => ref.read(languageProvider.notifier).set(otherLanguage),
      ),
      PaletteCommand(
        id: 'toggle-rail',
        label: ref.read(railExpandedProvider)
            ? l10n.shellPaletteRailCollapse
            : l10n.shellPaletteRailExpand,
        run: ref.read(railExpandedProvider.notifier).toggle,
      ),
      PaletteCommand(
        id: 'open-settings',
        label: l10n.settingsPaletteOpen,
        run: _openSettings,
      ),
      PaletteCommand(
        id: 'reconnect',
        label: l10n.shellPaletteReconnect,
        run: ref.read(connectionStateProvider.notifier).retry,
      ),
      // The one "allow all" of the program lives here and nowhere else: no
      // control on the queue carries that label, and this command opens the
      // modal with the hosts instead of sending anything (HUM-029).
      if (live)
        PaletteCommand(
          id: 'queue-allow-all',
          label: l10n.paletteQueueAllowAll,
          run: () {
            _go(Section.intercept);
            ref.read(interceptDecisionProvider.notifier).askAllowAll();
          },
        ),
      // Braucht keinen Daemon und steht deshalb auch während eines Bruchs
      // darin: Name, Version, Lizenz und die Namensnennung der Rangliste
      // (HUM-031).
      PaletteCommand(
        id: 'about',
        label: l10n.shellPaletteAbout,
        run: () => showHumanitlAbout(context),
      ),
      // Dasselbe wie `Ctrl+Shift+Enter` und der Knopf über dem Terminal
      // (HUM-298). Am Ende, damit die Zeilen davor ihren Platz behalten.
      PaletteCommand(
        id: 'toggle-terminal-full-window',
        label:
            ref.read(terminalPlacementProvider) == TerminalPlacement.fullWindow
            ? l10n.shellPaletteTerminalFullWindowLeave
            : l10n.shellPaletteTerminalFullWindowEnter,
        run: ref.read(terminalPlacementProvider.notifier).toggleFullWindow,
      ),
    ];
  }

  /// Räumt weg, was ein Bruch der Verbindung unbedienbar zurückließe.
  ///
  /// Zwei Dinge, und beide würden sonst mit dem Bruch stehen bleiben:
  ///
  /// - **Der Fokus.** Er sitzt beim Arbeiten in der Warteschlange, und die
  ///   liegt ab jetzt unter dem `ExcludeFocus` des Schnappschusses. Ohne
  ///   diesen Griff fiele der Fokus auf den Wurzelbereich zurück, und damit
  ///   wäre die ganze Tastatur der Shell tot: `Ctrl+1` bis `Ctrl+6` und
  ///   `Ctrl+K` hängen an [Shortcuts] über diesem Baum, und `Shortcuts` feuert
  ///   nur entlang der Fokuskette. Wer mit der Tastatur arbeitet, käme dann
  ///   weder in den Setup-Bildschirm noch an die Palette und damit an keine
  ///   der beiden Stellen, die den Daemon zurückholen (`docs/UX.md` 5.1). Der
  ///   Griff wartet auf das Ende des Frames, in dem `ExcludeFocus` in den Baum
  ///   kommt; davor nähme dieses ihn gleich wieder weg.
  /// - **Das Modal der Sammelentscheidung.** Es steht innerhalb der
  ///   eingefrorenen Fläche. Stand es offen, als die Verbindung wegging, sind
  ///   sein Bestätigen, sein Schirm und `Escape` still, und es bliebe stehen,
  ///   bis die Verbindung zurück ist. Die Frage, die es stellt, ist ohne
  ///   Daemon ohnehin nicht zu beantworten, also wird sie zurückgenommen; die
  ///   Warteschlange darunter bleibt unverändert stehen.
  void _linkChanged(bool? previous, bool live) {
    if (live) {
      return;
    }
    ref.read(batchConfirmProvider.notifier).cancel();
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted) {
        _focus.requestFocus();
      }
    });
  }

  /// Carries out a handover another section asked for.
  ///
  /// The history hands a held request to the queue; a feature may not reach
  /// into another feature to do it, and it does not have to — the shell is
  /// what composes the sections (ARCHITECTURE 5). The note is cleared here,
  /// so it is carried out once.
  void _takeHandoff(FlowId? previous, FlowId? next) {
    if (next == null) {
      return;
    }
    ref.read(selectedFlowIdProvider.notifier).select(next);
    _go(Section.intercept);
    ref.read(flowHandoffProvider.notifier).clear();
  }

  /// Shows the history when another section asked it to open a flow.
  ///
  /// The mirror of [_takeHandoff] for a finished flow: a finding over the
  /// queue can name a passthrough that no queue holds (HUM-039). The shell
  /// only switches the section; the history fetches and opens the flow and
  /// clears the note, because only it knows when the flow is open.
  void _takeReveal(FlowId? previous, FlowId? next) {
    if (next != null) {
      _go(Section.history);
    }
  }

  /// Opens the setup section once, when the first settled answer says that
  /// nothing can start.
  ///
  /// **The condition is [SetupState.needsSetup] and not
  /// [SetupState.canStart], and that is a correction of the specification.**
  /// HUM-044 asks for the setup screen "when one of the four checks is not
  /// green", written before HUM-075 decided that the language model is
  /// contacted only on request. Its line therefore carries `DOCTOR_013` on
  /// every start that nobody asked to probe -- which is most of them -- so
  /// "not green" would mean "always", and the queue would be behind a
  /// checklist for ever. What opens the screen is a row that failed or one
  /// still being asked. The start button asks the stricter question and waits
  /// for all four rows to be green ([SetupState.canStart]); the two are
  /// deliberately not the same predicate.
  void _offerSetup(SetupState? previous, SetupState next) {
    if (_setupOffered || next.worst == SetupCheckState.checking) {
      return;
    }
    _setupOffered = true;
    if (next.needsSetup) {
      ref.read(navigationProvider.notifier).go(Section.setup);
    }
  }

  // --- The terminal over the whole window (HUM-298) -------------------------

  /// A section that is asked for ends the whole window: whoever presses
  /// `Ctrl+2` or picks "Go to History" in the palette wants to see History,
  /// and the queue that a handover opens has to be seen too.
  void _sectionAsked(Section? previous, Section next) =>
      ref.read(terminalPlacementProvider.notifier).leaveFullWindow();

  /// Shows [section] and ends the whole window, also when [section] is
  /// already the one underneath: then the navigation does not change, and
  /// [_sectionAsked] never hears of it.
  void _go(Section section) {
    ref.read(terminalPlacementProvider.notifier).leaveFullWindow();
    ref.read(navigationProvider.notifier).go(section);
  }

  /// Takes the keyboard when the terminal changes place.
  ///
  /// The control that held the focus may just have been hidden, and a focus
  /// that falls back to the root would leave every shortcut of the shell
  /// mute, `Ctrl+Shift+Enter` included. The terminal takes the keys from
  /// here one frame later, when its new host mounts it
  /// (`TerminalPane.focusOnMount`).
  void _terminalMoved(TerminalPlacement? previous, TerminalPlacement next) =>
      _focus.requestFocus();

  /// The terminal of the current session in place of rail, sections and
  /// status bar. The header bar stays: it is the title bar of the window,
  /// with its buttons and the count of held requests.
  ///
  /// It stands in a focus scope of its own, so that the keys can go back to
  /// the terminal after the palette, which lies outside it, had them.
  Widget _terminalFullWindow() => FocusScope(
    node: _terminalScope,
    child: const SandboxTerminal(
      key: Key('shell-terminal-full-window'),
      focusOnMount: true,
      catchUp: true,
    ),
  );

  /// [layout] -- rail, sections and sheet --, or the terminal in its place
  /// while [fullWindow] holds.
  ///
  /// **The layout stays built underneath.** It carries the state of every
  /// section (the frozen order of the queue, scroll positions, an open
  /// editor), and leaving the whole window has to find all of it where it
  /// was. An [IndexedStack] keeps it, takes away its focus and tells every
  /// section through `Visibility.of` that it is not shown, so the decision
  /// keys of the queue are out of service while the terminal fills the
  /// window. Its tickers stop with the [TickerMode]. A floating terminal
  /// (HUM-300) becomes a sibling over [layout] in the same place.
  Widget _withTerminalHost({
    required bool fullWindow,
    required Widget layout,
  }) => IndexedStack(
    index: fullWindow ? 1 : 0,
    sizing: StackFit.expand,
    children: <Widget>[
      TickerMode(enabled: !fullWindow, child: layout),
      if (fullWindow) _terminalFullWindow() else const SizedBox.shrink(),
    ],
  );

  /// Die sechs Abschnitte, und bei einem Bruch der Verbindung fünf davon als
  /// Schnappschuss.
  ///
  /// Der Setup-Bildschirm steht **im** Stapel und nicht darüber: Wer ihn
  /// ansieht, behält die Kopfzeile mit den gehaltenen Anfragen und wechselt
  /// mit `Ctrl+1` zu ihnen (HUM-044).
  ///
  /// **Und er friert nicht mit ein.** Eingefroren wird, was Daten des Daemons
  /// zeigt und damit veraltet, sobald der Daemon schweigt. Der
  /// Setup-Bildschirm zeigt keine: Seine vier Zeilen messen selbst nach, und
  /// er trägt `Retry`, `Check again` und die Aktion von `DAEMON_001`, die
  /// `humanitld` installiert und startet. Läge er unter demselben
  /// [AbsorbPointer] wie die anderen fünf, wäre der eine Bildschirm, dessen
  /// Aufgabe es ist, den Daemon zurückzuholen, genau dann tot, wenn der
  /// Daemon tot ist -- und die einzigen lebenden Bedienelemente wären
  /// „Erneut verbinden" im Banner und `reconnect` in der Palette, von denen
  /// keines etwas installiert.
  /// **Jeder der fünf trägt einen `GlobalKey`, und der ist kein Schmuck.**
  /// Ein Bruch hängt [FrozenSections] über den Abschnitt und der
  /// Wiederanschluss nimmt es wieder weg; an dieser Stelle des Stapels
  /// wechselt damit der Widget-Typ, `Widget.canUpdate` sagt nein, und ohne
  /// diesen Schlüssel würde das Element darunter samt jedem `State` darin
  /// weggeworfen. Mit ihm wird es umgehängt. Was in diesen `State`s liegt,
  /// legt `docs/UX.md` 7 ausdrücklich dorthin -- die eingefrorene Reihenfolge
  /// der Warteschlange, ihre zugelassenen Zeilen, die Zeigeranwesenheit, die
  /// Scrollposition --, während der Zähler der ausstehenden Ankünfte ein
  /// Provider ist und den Bruch überlebt. Ohne den Schlüssel stünde nach
  /// jedem Bruch eine Pille „+6 neu" über sechs Zeilen, die das Pane gerade
  /// selbst zugelassen hat, und ein Klick darauf täte nichts
  /// (`docs/UX.md` 2.8 und 5.3).
  ///
  /// **Jeder Abschnitt steht in einem eigenen `TickerMode`, an nur für den
  /// sichtbaren.** `IndexedStack` malt die verdeckten Abschnitte nicht, hält
  /// ihre Ticker aber nicht an (es setzt nur `Visibility.of` und
  /// `ExcludeFocus`). Ohne das plante etwa das Absperrband über der
  /// Warteschlange (HUM-260) nach einem Wechsel in einen anderen Abschnitt
  /// weiter jeden Frame für Pixel, die niemand sieht, und
  /// `TickerMode.valuesOf` in der History gäbe nicht die Antwort, auf die sie
  /// sich verlässt. Der `TickerMode` steht immer da, auch eingeschaltet,
  /// damit die Form des Baums beim Wechsel gleich bleibt.
  Widget _sections(Section section, {required bool frozen}) => IndexedStack(
    index: section.index,
    children: _tickingOnlyAt(section.index, <Widget>[
      // Der Editor von HUM-047 wird hier eingehaengt und nicht in der
      // Warteschlange: Kein Feature importiert ein anderes, und die Shell ist
      // die eine Stelle, die beide kennen darf (`docs/ARCHITECTURE.md` 5).
      _snapshot(
        frozen,
        _keys[0],
        const InterceptScreen(editorBuilder: buildEditorPane),
      ),
      _snapshot(frozen, _keys[1], const HistoryScreen()),
      _snapshot(frozen, _keys[2], const RulesScreen()),
      _snapshot(frozen, _keys[3], const SandboxScreen()),
      _snapshot(frozen, _keys[4], const AuditScreen()),
      const SetupHost(),
    ]),
  );

  /// [sections], jeder in einem `TickerMode`, der nur bei [visible] an ist.
  static List<Widget> _tickingOnlyAt(int visible, List<Widget> sections) =>
      <Widget>[
        for (int i = 0; i < sections.length; i++)
          TickerMode(enabled: i == visible, child: sections[i]),
      ];

  /// [section] unter [key], als Schnappschuss, solange [frozen] gilt.
  static Widget _snapshot(bool frozen, GlobalKey key, Widget section) {
    final Widget kept = KeyedSubtree(key: key, child: section);
    return frozen ? FrozenSections(child: kept) : kept;
  }

  @override
  Widget build(BuildContext context) {
    ref.listen<FlowId?>(flowHandoffProvider, _takeHandoff);
    ref.listen<FlowId?>(flowRevealProvider, _takeReveal);
    // Ohne `fireImmediately`: Der erste Zustand ist `checking`, und darauf
    // schaltet `_offerSetup` ohnehin nicht. Der erste gesetzte Zustand kommt
    // als Änderung an, und genau auf ihn wartet die Weiche.
    ref.listen<SetupState>(setupStateProvider, _offerSetup);
    ref.listen<bool>(linkLiveProvider, _linkChanged);
    ref.listen<Section>(navigationProvider, _sectionAsked);
    ref.listen<TerminalPlacement>(terminalPlacementProvider, _terminalMoved);
    // Die Shell steht auch dann, wenn die Verbindung wegbricht. Sie sagt es
    // dann im Banner und friert die Abschnitte darunter ein, die Daten des
    // Daemons zeigen (`docs/UX.md` 4.2, Fall 4). Die eine Frage danach steht
    // in [linkLiveProvider] und wird hier nicht ein zweites Mal gestellt; der
    // Zustand selbst wird nur noch für den Grund im Banner gelesen.
    final bool live = ref.watch(linkLiveProvider);
    final ConnectionStatus link = ref.watch(connectionStateProvider);
    final Section section = ref.watch(navigationProvider);
    final Navigation navigation = ref.read(navigationProvider.notifier);
    final bool railExpanded = ref.watch(railExpandedProvider);
    final RailExpanded rail = ref.read(railExpandedProvider.notifier);
    final bool fullWindow =
        ref.watch(terminalPlacementProvider) == TerminalPlacement.fullWindow;
    final TerminalPlacementNotifier terminal = ref.read(
      terminalPlacementProvider.notifier,
    );
    final HTokens tokens = HTheme.of(context);
    return Shortcuts(
      shortcuts: shellShortcuts(),
      child: Actions(
        actions: <Type, Action<Intent>>{
          NavIntent: CallbackAction<NavIntent>(
            onInvoke: (NavIntent intent) {
              terminal.leaveFullWindow();
              navigation.goIndex(intent.index);
              return null;
            },
          ),
          ToggleTerminalFullWindowIntent:
              CallbackAction<ToggleTerminalFullWindowIntent>(
                onInvoke: (ToggleTerminalFullWindowIntent intent) {
                  terminal.toggleFullWindow();
                  return null;
                },
              ),
          PaletteIntent: CallbackAction<PaletteIntent>(
            onInvoke: (PaletteIntent intent) {
              _togglePalette();
              return null;
            },
          ),
          ToggleRailIntent: CallbackAction<ToggleRailIntent>(
            onInvoke: (ToggleRailIntent intent) {
              rail.toggle();
              return null;
            },
          ),
          SettingsIntent: CallbackAction<SettingsIntent>(
            onInvoke: (SettingsIntent intent) {
              _settingsOpen ? _closeSettings() : _openSettings();
              return null;
            },
          ),
        },
        child: Focus(
          focusNode: _focus,
          autofocus: true,
          // Der Weg eines `ChangeSetting` in das Blatt (HUM-275): Jede
          // Diagnostik-Karte darunter, auch die im Blatt selbst, öffnet es
          // über diesen Griff am Feld ihres Schlüssels.
          child: SettingsOpener(
            open: _openSettingsAt,
            child: Stack(
              fit: StackFit.expand,
              children: <Widget>[
                ColoredBox(
                  color: tokens.colors.bg0,
                  child: Column(
                    children: <Widget>[
                      HeaderBar(
                        section: section,
                        onPalette: _togglePalette,
                        onSettings: _openSettings,
                      ),
                      if (link case ConnectionFrozen(
                        :final Diagnostic diagnostic,
                      ))
                        FrozenBanner(
                          diagnostic: diagnostic,
                          onReconnect: ref
                              .read(connectionStateProvider.notifier)
                              .retry,
                        ),
                      const ShellNotices(),
                      Expanded(
                        child: _withTerminalHost(
                          fullWindow: fullWindow,
                          layout: Stack(
                            fit: StackFit.expand,
                            children: <Widget>[
                              Row(
                                children: <Widget>[
                                  IconRail(
                                    active: section,
                                    onSelect: navigation.go,
                                    expanded: railExpanded,
                                    onToggle: rail.toggle,
                                  ),
                                  Expanded(
                                    child: _sections(section, frozen: !live),
                                  ),
                                ],
                              ),
                              // Das Blatt hängt zwischen Kopfleiste und
                              // Statusleiste, wie jeder Pane: seine Kopfzeile
                              // steht in derselben Reihe wie die der Queue, und
                              // die Linie darunter läuft durch (HUM-286).
                              if (_settingsOpen)
                                Align(
                                  alignment: Alignment.centerRight,
                                  child: SettingsSheet(
                                    onClose: _closeSettings,
                                    focusKey: _settingsFocusKey,
                                    focusSerial: _settingsFocusSerial,
                                  ),
                                ),
                            ],
                          ),
                        ),
                      ),
                      // Hidden, not removed, so its state survives the whole
                      // window (HUM-298).
                      Visibility(
                        visible: !fullWindow,
                        maintainState: true,
                        child: StatusBar(info: widget.info),
                      ),
                    ],
                  ),
                ),
                if (_paletteOpen)
                  CommandPalette(
                    commands: _commands(context, live: live),
                    onClose: _closePalette,
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
