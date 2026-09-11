/// The shell: header, rail, sections, status bar, palette and the shortcuts
/// that bind them (HUM-019).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/flow_handoff.dart';
import '../../core/ipc/flow_reveal.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import '../audit/audit_screen.dart';
import '../history/history_screen.dart';
import '../intercept/intercept_screen.dart';
import '../intercept/providers/decision.dart';
import '../intercept/providers/flows.dart';
import '../rules/rules_screen.dart';
import '../sandbox/sandbox_screen.dart';
import '../setup/providers/setup_provider.dart';
import 'providers/connection.dart';
import 'providers/navigation.dart';
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

  /// Whether the setup screen has already had its one chance to open itself.
  ///
  /// It gets exactly one: at startup, as soon as the four checks have settled,
  /// and only when they are not all green. After that the section belongs to
  /// the person -- a screen that pulled somebody out of the queue because a
  /// check went amber ten minutes later would take the work away to show a
  /// checklist (`docs/UX.md` 2.8).
  bool _setupOffered = false;

  /// Ein Schlüssel je Daten-Abschnitt, damit ein Bruch der Verbindung ihre
  /// Elemente umhängt statt sie zu zerstören. Felder und keine Ausdrücke in
  /// `build`: Ein neuer Schlüssel je Frame hielte gar nichts.
  final List<GlobalKey> _keys = <GlobalKey>[
    for (int i = 0; i < 5; i++) GlobalKey(debugLabel: 'section-$i'),
  ];

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  void _openPalette() => setState(() => _paletteOpen = true);

  void _closePalette() {
    setState(() => _paletteOpen = false);
    // The palette's field held the focus; without this the next Ctrl+2 would
    // reach nobody.
    _focus.requestFocus();
  }

  void _togglePalette() => _paletteOpen ? _closePalette() : _openPalette();

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
    final Navigation navigation = ref.read(navigationProvider.notifier);
    return <PaletteCommand>[
      for (final Section section in Section.values)
        PaletteCommand(
          id: 'go-${section.name}',
          label: l10n.shellPaletteGoTo(section.label(l10n)),
          run: () => navigation.go(section),
        ),
      PaletteCommand(
        id: 'toggle-theme',
        label: l10n.shellPaletteToggleTheme,
        run: () => ref
            .read(themeModeProvider.notifier)
            .toggle(MediaQuery.platformBrightnessOf(context)),
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
            navigation.go(Section.intercept);
            ref.read(interceptDecisionProvider.notifier).askAllowAll();
          },
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
    ref.read(navigationProvider.notifier).go(Section.intercept);
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
      ref.read(navigationProvider.notifier).go(Section.history);
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
  Widget _sections(Section section, {required bool frozen}) => IndexedStack(
    index: section.index,
    children: <Widget>[
      _snapshot(frozen, _keys[0], const InterceptScreen()),
      _snapshot(frozen, _keys[1], const HistoryScreen()),
      _snapshot(frozen, _keys[2], const RulesScreen()),
      _snapshot(frozen, _keys[3], const SandboxScreen()),
      _snapshot(frozen, _keys[4], const AuditScreen()),
      const SetupHost(),
    ],
  );

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
    // Die Shell steht auch dann, wenn die Verbindung wegbricht. Sie sagt es
    // dann im Banner und friert die Abschnitte darunter ein, die Daten des
    // Daemons zeigen (`docs/UX.md` 4.2, Fall 4). Die eine Frage danach steht
    // in [linkLiveProvider] und wird hier nicht ein zweites Mal gestellt; der
    // Zustand selbst wird nur noch für den Grund im Banner gelesen.
    final bool live = ref.watch(linkLiveProvider);
    final ConnectionStatus link = ref.watch(connectionStateProvider);
    final Section section = ref.watch(navigationProvider);
    final Navigation navigation = ref.read(navigationProvider.notifier);
    final HTokens tokens = HTheme.of(context);
    return Shortcuts(
      shortcuts: shellShortcuts(),
      child: Actions(
        actions: <Type, Action<Intent>>{
          NavIntent: CallbackAction<NavIntent>(
            onInvoke: (NavIntent intent) {
              navigation.goIndex(intent.index);
              return null;
            },
          ),
          PaletteIntent: CallbackAction<PaletteIntent>(
            onInvoke: (PaletteIntent intent) {
              _togglePalette();
              return null;
            },
          ),
        },
        child: Focus(
          focusNode: _focus,
          autofocus: true,
          child: Stack(
            fit: StackFit.expand,
            children: <Widget>[
              ColoredBox(
                color: tokens.colors.bg0,
                child: Column(
                  children: <Widget>[
                    HeaderBar(section: section, onPalette: _togglePalette),
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
                      child: Row(
                        children: <Widget>[
                          IconRail(active: section, onSelect: navigation.go),
                          Expanded(child: _sections(section, frozen: !live)),
                        ],
                      ),
                    ),
                    StatusBar(info: widget.info),
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
    );
  }
}
