/// The History section: filter, table, detail.
///
/// The screen owns three things and no more: the keyboard, the split between
/// table and detail, and what a double click does with a row. Everything else
/// is in the four widgets below it and in the two providers.
///
/// There is no shared transition into this list. `docs/UX.md` 2.9b allows the
/// `Hero` from a decided card into the history row only while both screens
/// stand on the screen at once; the shell shows one section at a time in an
/// `IndexedStack`, so the condition is not met and the transition is left out
/// rather than faked.
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/ipc/client_providers.dart';
import '../../core/ipc/connection.dart';
import '../../core/ipc/flow_events.dart';
import '../../core/ipc/flow_handoff.dart';
import '../../core/ipc/flow_reveal.dart';
import '../../core/shortcuts/intents.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_detail.dart';
import 'history_export_menu.dart';
import 'history_filter_bar.dart';
import 'history_table.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';
import 'providers/history_page.dart';

/// How often the screen has asked for the keyboard.
///
/// Claiming it in every build would pull it back out of the shell's rail one
/// frame after somebody tabbed there; the counter is what a test can hold
/// against that, because the defect is invisible from inside this screen —
/// its own focus node counts a focused child as focused.
@visibleForTesting
int debugHistoryFocusClaims = 0;

/// The bindings of the History section.
///
/// Every activator here has an action in [HistoryScreen]; a widget test
/// compares the two sets, because a bound key that does nothing is worse than
/// an unbound one (`docs/UX.md` 5.3).
Map<ShortcutActivator, Intent>
historyShortcuts() => <ShortcutActivator, Intent>{
  const SingleActivator(LogicalKeyboardKey.enter): const OpenFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.numpadEnter): const OpenFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.slash): const FilterIntent(),
  const SingleActivator(LogicalKeyboardKey.keyJ): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowDown): const NextFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.keyK): const PrevFlowIntent(),
  const SingleActivator(LogicalKeyboardKey.arrowUp): const PrevFlowIntent(),
};

/// Opens the selected row: a held request in the queue, anything else in the
/// sheet. The keyboard equivalent of the double click (`docs/UX.md` 5.1).
///
/// Local to this screen rather than in `core/shortcuts`: it means nothing
/// anywhere else, and a shared intent without a second user is a guess about
/// the future.
class OpenFlowIntent extends Intent {
  /// Creates the intent.
  const OpenFlowIntent();
}

/// How much of the height the detail takes by default.
const double historyDetailShare = 0.4;

/// The smallest share either half of the split can be squeezed to.
const double historySplitMin = 0.2;

/// Width of the detail sheet a double click opens.
const double historySheetWidth = 520;

/// The History section.
class HistoryScreen extends ConsumerStatefulWidget {
  /// Creates the section.
  const HistoryScreen({this.exportOpen = false, super.key});

  /// Opens the export modal at once. Only a golden passes it: a modal that a
  /// test has to click open cannot be photographed in one frame.
  final bool exportOpen;

  @override
  ConsumerState<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends ConsumerState<HistoryScreen> {
  final FocusNode _filterFocus = FocusNode(debugLabel: 'history-filter');
  final FocusNode _tableFocus = FocusNode(debugLabel: 'history-table');
  final GlobalKey<HistoryTableState> _tableKey = GlobalKey<HistoryTableState>(
    debugLabel: 'history-table',
  );

  Flow? _sheetFlow;

  /// Die Id, die gerade für das Blatt geholt wird, oder null.
  FlowId? _revealing;

  /// True, wenn dieser Flow fertig wurde, während er geholt wurde.
  bool _revealEnded = false;

  /// Die Nummer des jüngsten Abrufs für das Blatt; ein Doppelklick zählt mit.
  ///
  /// Nur der Abruf mit dieser Nummer darf das Blatt noch setzen und hinter
  /// sich aufräumen.
  int _revealGeneration = 0;

  late bool _exportOpen = widget.exportOpen;

  /// Why the last flow another screen asked for could not be opened, or null.
  Diagnostic? _revealFailure;

  late final Map<Type, Action<Intent>> _actions = <Type, Action<Intent>>{
    OpenFlowIntent: _SingleKeyAction<OpenFlowIntent>(_openSelected),
    FilterIntent: _SingleKeyAction<FilterIntent>(_focusFilter),
    NextFlowIntent: _SingleKeyAction<NextFlowIntent>(() => _move(1)),
    PrevFlowIntent: _SingleKeyAction<PrevFlowIntent>(() => _move(-1)),
  };

  @override
  void dispose() {
    _filterFocus.dispose();
    _tableFocus.dispose();
    super.dispose();
  }

  /// True while this section is the visible branch of the shell's stack.
  bool _visible = false;

  /// Takes the keyboard the first time the section becomes visible again.
  void _claimFocusOnceVisible(bool visible) {
    if (visible == _visible) {
      return;
    }
    _visible = visible;
    if (!visible) {
      return;
    }
    debugHistoryFocusClaims++;
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (mounted && _visible && !_tableFocus.hasFocus) {
        _tableFocus.requestFocus();
      }
    });
  }

  void _focusFilter() => _filterFocus.requestFocus();

  void _move(int delta) => _tableKey.currentState?.moveSelection(delta);

  /// What a double click does with [flow].
  ///
  /// A held request belongs on the screen where it can be decided, so the
  /// history asks for it to be shown there and the shell carries the request
  /// out: a feature may not reach into another feature, and the shell is what
  /// composes the sections (ARCHITECTURE 5). Everything else is read in a
  /// sheet, at full height — finished or not: a row whose answer is still
  /// running in opens here too, and the sheet follows it while it arrives.
  void _open(Flow flow) {
    if (flow.isHeld) {
      ref.read(flowHandoffProvider.notifier).request(flow.id);
      return;
    }
    ref.read(historySelectionProvider.notifier).select(flow.id);
    // Ein Doppelklick schlägt einen Abruf, der noch unterwegs ist: dessen
    // Antwort gehört zu einem Blatt, das es nicht mehr gibt, und überschriebe
    // sonst dieses mit dem Stand eines anderen Flows. Die Notiz wird
    // zurückgenommen, damit `_takeReveal` die Antwort als veraltet erkennt,
    // und das Abruffenster geschlossen (HUM-154).
    _revealGeneration++;
    _revealing = null;
    _revealEnded = false;
    if (ref.read(flowRevealProvider) != null) {
      ref.read(flowRevealProvider.notifier).clear();
    }
    // The sheet keeps the focus with itself and closes on `Escape`, but only
    // once the focus is inside it; the table is holding it right now.
    _tableFocus.unfocus();
    setState(() => _sheetFlow = flow);
  }

  /// Opens the selected row, the way a double click would.
  void _openSelected() {
    final FlowId? id = ref.read(historySelectionProvider);
    if (id == null) {
      return;
    }
    final Flow? flow = ref
        .read(historyPageProvider)
        .rows
        .where((Flow row) => row.id == id)
        .firstOrNull;
    if (flow != null) {
      _open(flow);
    }
  }

  /// Keeps the sheet current for a flow that stands in no loaded row.
  ///
  /// Der übliche Weg ist die Zeile: das Blatt liest sie aus der Seite und ist
  /// damit so frisch wie die Tabelle. Ein Flow über `flowRevealProvider`
  /// steht aber in keiner Zeile (siehe [_takeReveal]), und für ihn gäbe es
  /// sonst keine Quelle als den Abzug vom Öffnen — der Rumpf-Abschnitt
  /// wartete dann für immer auf eine Antwort, die längst aufgezeichnet ist
  /// (HUM-154).
  ///
  /// Gehorcht wird genau zwei Ereignissen. `Recorded` schickt der Daemon für
  /// jeden fertigen Flow, auch für den gescheiterten und den abgelaufenen —
  /// auf `Failed` zusätzlich zu horchen hieße, zweimal hintereinander
  /// dasselbe zu fragen, und die zweite Abfrage liefe in die erste hinein.
  /// `Lagged` trägt keine Id: was in der Lücke geschah, weiß niemand, also
  /// fragt das Blatt nach seinem eigenen Flow, so wie die Seite dort neu
  /// lädt. Ein `ResponseChunk` je Paket würde den Daemon einmal je Paket
  /// fragen und am Zustand nichts ändern.
  void _followSheetFlow(
    AsyncValue<FlowEvent>? previous,
    AsyncValue<FlowEvent> next,
  ) {
    // Auch der Flow, der gerade geholt wird: zwischen `_sheetFlow = null` und
    // der Antwort von `GetFlow` liegt ein Fenster, und ein Ende, das darin
    // fällt, käme nie wieder — die Antwort trüge dann die Zusammenfassung von
    // vor dem Ende, und das Blatt wartete für immer.
    final FlowId? watched = _sheetFlow?.id ?? _revealing;
    if (watched == null) {
      return;
    }
    final FlowId? id = switch (next.value) {
      FlowEventRecorded(:final FlowId flowId) => flowId,
      FlowEventLagged() => watched,
      _ => null,
    };
    if (id != watched) {
      return;
    }
    final Flow? sheet = _sheetFlow;
    if (sheet == null) {
      // Der Abruf läuft noch. Gemerkt, und sobald er zurück ist, nachgeholt;
      // jetzt nachzufassen hieße, gegen die eigene Antwort zu laufen.
      _revealEnded = true;
      return;
    }
    if (sheet.state.isTerminal) {
      // Fertig ist fertig; eine Lücke ändert daran nichts mehr.
      return;
    }
    if (ref.read(historyPageProvider).rows.any((Flow row) => row.id == id)) {
      // Die Zeile trägt den Zustand schon; ein zweites Nachfassen wäre eine
      // zweite Abfrage für dasselbe.
      return;
    }
    _refreshSheetFlow(watched);
  }

  /// Holt die Zusammenfassung zu [id] neu und übernimmt sie ins Blatt.
  ///
  /// Über denselben Provider, den das Detail im Blatt ohnehin liest: eine
  /// Abfrage, ein Ergebnis, und ein Fehler steht als Diagnose im Blatt selbst
  /// (`_Failure`). Schlägt sie fehl, bleibt der Abzug stehen und das Blatt
  /// wartet weiter, statt etwas zu behaupten.
  void _refreshSheetFlow(FlowId id) {
    ref.invalidate(historyDetailProvider(id));
    unawaited(
      ref.read(historyDetailProvider(id).future).then((FlowDetail detail) {
        if (mounted && _sheetFlow?.id == detail.summary.id) {
          setState(() => _sheetFlow = detail.summary);
        }
      }, onError: (Object _) {}),
    );
  }

  /// [live] mit allem, was [kept] schon wusste — für zwei Zusammenfassungen
  /// desselben fertigen Flows.
  ///
  /// Fertig ist fertig, also können beide nur verschieden viel wissen, nicht
  /// Verschiedenes: eine fehlende Dauer und ein fehlender Status werden aus
  /// [kept] genommen, und die Größen wachsen nur, weil ein Flow nach dem
  /// Ende keine Bytes verliert (HUM-154).
  static Flow _fullerOf(Flow kept, Flow live) => live.copyWith(
    duration: live.duration ?? kept.duration,
    status: live.status != 0 ? live.status : kept.status,
    requestSize: math.max(live.requestSize, kept.requestSize),
    responseSize: math.max(live.responseSize, kept.responseSize),
  );

  /// Opens a flow another screen asked for (HUM-039).
  ///
  /// The flow may be in no row of this table: a passthrough behind `LLM_005`
  /// is recorded but never listed with the held requests, and the page may
  /// not have reached it. The sheet therefore takes the summary from
  /// `GetFlow`, not from the rows. The note is cleared once the fetch has
  /// ended, so it is carried out once.
  ///
  /// Only the note that is still current acts: a newer one that arrived
  /// while this fetch ran owns the sheet. A fetch that fails is said over the
  /// list with the daemon's own sentence and never swallowed; a click that
  /// seems to do nothing is the one thing `docs/UX.md` 4.4 rules out.
  void _takeReveal(FlowId? previous, FlowId? next) {
    if (next == null) {
      return;
    }
    ref.read(historySelectionProvider.notifier).select(next);
    // A new request replaces what the last one left: its failure card and
    // its sheet. An old sheet next to a new failure would show the wrong
    // request under the right complaint.
    setState(() {
      _revealFailure = null;
      _sheetFlow = null;
    });
    // Solange geholt wird, gibt es keinen Abzug, an dem [_followSheetFlow]
    // ein Ende erkennen könnte. Es merkt sich deshalb diese Id, und was
    // während des Abrufs endet, wird danach nachgeholt.
    _revealing = next;
    _revealEnded = false;
    // Die Notiz allein unterscheidet zwei Abrufe desselben Flows nicht: wird
    // er nach einem Doppelklick ein zweites Mal aufgedeckt, sähe der erste
    // Abruf wieder aktuell aus und überschriebe das Blatt mit seinem älteren
    // Stand. Jeder Abruf trägt deshalb seine Nummer (HUM-154).
    final int generation = ++_revealGeneration;
    bool current() =>
        mounted &&
        _revealGeneration == generation &&
        ref.read(flowRevealProvider) == next;
    // The client itself and not `historyDetailProvider`: that one disposes
    // itself as soon as nobody watches it, and a one-shot read of its future
    // then ends in "the provider was disposed" instead of the daemon's
    // answer -- measured in the test of the failure path.
    unawaited(
      ref
          .read(daemonClientProvider)
          .getFlow(next)
          .then(
            (FlowDetail detail) {
              if (!current()) {
                return;
              }
              _tableFocus.unfocus();
              setState(() => _sheetFlow = detail.summary);
              if (_revealEnded && !detail.summary.state.isTerminal) {
                // Der Flow ist fertig geworden, während diese Antwort
                // unterwegs war: sie trägt den Stand von vorher, und ohne
                // dieses Nachholen bliebe das Blatt darauf sitzen.
                _refreshSheetFlow(next);
              }
            },
            onError: (Object error) {
              if (!current()) {
                return;
              }
              setState(
                () => _revealFailure = DaemonConnection.diagnosticOf(error),
              );
            },
          )
          .whenComplete(() {
            // Nur der eigene Durchlauf räumt auf: Kam während des Abrufs ein
            // neuer, gehören ihm Fenster und Notiz, auch wenn er denselben
            // Flow meint.
            if (_revealGeneration != generation) {
              return;
            }
            _revealing = null;
            _revealEnded = false;
            if (mounted && ref.read(flowRevealProvider) == next) {
              ref.read(flowRevealProvider.notifier).clear();
            }
          }),
    );
  }

  /// What the failure card offers: the daemon's own proposal, if it made one,
  /// and always the way to hide the card. A diagnostic that carries a
  /// `FixAction` without a visible action is a defect (`docs/UX.md` 4.4).
  Widget _revealActions(
    HTokens tokens,
    AppLocalizations l10n,
    Diagnostic failure,
  ) {
    final Widget dismiss = HButton(
      variant: HButtonVariant.secondary,
      onPressed: () => setState(() => _revealFailure = null),
      child: Text(l10n.historyRevealFailedDismiss),
    );
    final FixAction? fix = failure.fix;
    if (fix == null) {
      return dismiss;
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        FixControl(fix: fix, copyKey: const Key('history-reveal-failure-fix')),
        SizedBox(height: tokens.spacing.x2),
        dismiss,
      ],
    );
  }

  void _closeSheet() {
    setState(() => _sheetFlow = null);
    _tableFocus.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? failure = ref.watch(
      historyPageProvider.select((HistoryPageState page) => page.failure),
    );
    // A refused filter is answered under the filter field, where it was
    // caused; everything else is a data error and gets the banner over the
    // list (`docs/UX.md` 4.4).
    final bool dataError =
        failure != null && failure.code != historyFilterInvalidCode;
    // The section that is on screen owns the keyboard. The shell builds all
    // five at once in an `IndexedStack`, so an `autofocus` here would take
    // the focus away from the queue at start-up. Which section that is comes
    // from the shell, and a feature cannot ask the shell; `TickerMode` is on
    // exactly for the visible branch of an `IndexedStack`, so it answers the
    // same question without the import.
    //
    // Claimed once per becoming visible, never on every build: a focus that
    // is taken back in the next frame is a focus nobody can move away, and
    // the rail of the shell is one Tab away.
    _claimFocusOnceVisible(TickerMode.valuesOf(context).enabled);
    ref.listen<FlowId?>(flowRevealProvider, _takeReveal);
    ref.listen<AsyncValue<FlowEvent>>(flowEventsProvider, _followSheetFlow);
    final FlowId? selected = ref.watch(historySelectionProvider);
    final Flow? selectedFlow = selected == null
        ? null
        : ref.watch(
            historyPageProvider.select(
              (HistoryPageState page) =>
                  page.rows.where((Flow row) => row.id == selected).firstOrNull,
            ),
          );
    // Das Blatt hält einen Abzug der Zeile, mit der es geöffnet wurde, und
    // Ereignisse des Daemons erreichen ihn nie. Gesucht wird deshalb dieselbe
    // Id in den geladenen Zeilen, nicht in der Auswahl: die Auswahl kann
    // weiterziehen, während das Blatt offen bleibt, und dann wartete sein
    // Rumpf-Abschnitt für immer auf eine Antwort, die längst da ist
    // (HUM-154).
    //
    // Was die Seite hergibt, wird zugleich zum neuen Abzug. Die Zeile kann
    // aus der Seite verschwinden — ein Neuladen leert sie, ein Filter
    // schließt sie aus, die Fensterkante schiebt sie hinaus —, und dann fiele
    // das Blatt sonst auf den Stand vom Öffnen zurück, also von „fertig"
    // wieder auf „kommt noch". Kein `setState`: gebaut wird ohnehin gerade,
    // und gelesen wird in diesem Bau der frischere Wert.
    final Flow? snapshot = _sheetFlow;
    final Flow? live = snapshot == null
        ? null
        : ref.watch(
            historyPageProvider.select(
              (HistoryPageState page) => page.rows
                  .where((Flow row) => row.id == snapshot.id)
                  .firstOrNull,
            ),
          );
    //
    // Einmal fertig, immer fertig. Eine Seite, die nicht aufgefrischt ist,
    // ein Fenster, ein Neuladen nach einem Abriss: sie alle können für
    // dieselbe Id eine ältere Zeile führen, und das Blatt fiele von
    // `recorded` auf `forwarded` zurück — mitsamt Skelett, das nie wieder
    // verschwindet, weil das Endereignis schon verbraucht ist.
    //
    // Eingefroren wird der Zustand, nicht die ganze Zeile: eine Zeile, die
    // selbst fertig ist, darf den Abzug ersetzen. `Recorded` setzt in der
    // Seite nur den Zustand; Dauer und Größe bringt erst ein späteres
    // Neuladen, und ohne diese Ausnahme stünde im Kopf des Blatts weiter „—",
    // während die Tabelle daneben die Dauer zeigt.
    //
    // Und zwischen zwei fertigen Zeilen gilt: was einmal bekannt war, bleibt
    // bekannt. Hat das Blatt die volle Zusammenfassung schon geholt, während
    // die Seite noch eine dünne führt, darf die dünne nichts wegnehmen,
    // nur ergänzen.
    final bool frozen = snapshot != null && snapshot.state.isTerminal;
    final bool liveIsFinal = live?.state.isTerminal ?? false;
    final Flow? candidate = live != null && frozen && liveIsFinal
        ? _fullerOf(snapshot, live)
        : live;
    if (candidate != null &&
        candidate != snapshot &&
        (liveIsFinal || !frozen)) {
      _sheetFlow = candidate;
    }
    final Flow? sheetFlow = frozen && !liveIsFinal
        ? snapshot
        : (candidate ?? snapshot);
    return Shortcuts(
      shortcuts: historyShortcuts(),
      child: Actions(
        actions: _actions,
        child: Focus(
          focusNode: _tableFocus,
          child: Stack(
            children: <Widget>[
              ColoredBox(
                color: tokens.colors.bg0,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    HistoryFilterBar(
                      focusNode: _filterFocus,
                      trailing: HistoryExportButton(
                        onOpen: () => setState(() => _exportOpen = true),
                      ),
                    ),
                    const HHairline(),
                    if (_revealFailure case final Diagnostic revealFailure)
                      Padding(
                        padding: EdgeInsets.all(tokens.spacing.x3),
                        child: HDiagnosticCard(
                          key: const Key('history-reveal-failure'),
                          code: revealFailure.code,
                          severityLabel: historySeverityLabel(
                            l10n,
                            revealFailure.severity,
                          ),
                          color: historySeverityColor(
                            tokens,
                            revealFailure.severity,
                          ),
                          title: l10n.historyRevealFailedTitle,
                          why: revealFailure.why,
                          docsUrl: revealFailure.docsUrl,
                          width: double.infinity,
                          fix: _revealActions(tokens, l10n, revealFailure),
                        ),
                      ),
                    if (dataError)
                      Padding(
                        padding: EdgeInsets.all(tokens.spacing.x3),
                        child: HDiagnosticCard(
                          code: failure.code,
                          severityLabel: historySeverityLabel(
                            l10n,
                            failure.severity,
                          ),
                          color: historySeverityColor(tokens, failure.severity),
                          title: l10n.historyLoadFailedTitle,
                          why: failure.why,
                          docsUrl: failure.docsUrl,
                          width: double.infinity,
                          fix: HButton(
                            variant: HButtonVariant.secondary,
                            onPressed: () => unawaited(
                              ref.read(historyPageProvider.notifier).reload(),
                            ),
                            child: Text(l10n.historyReload),
                          ),
                        ),
                      ),
                    Expanded(
                      child: _VerticalSplit(
                        // A click into the table gives it the keyboard, the
                        // way a desktop list does; `J` and `K` then work
                        // without a detour over the rail.
                        top: Listener(
                          onPointerDown: (PointerDownEvent _) =>
                              _tableFocus.requestFocus(),
                          child: HistoryTable(key: _tableKey, onOpen: _open),
                        ),
                        bottom: selectedFlow == null
                            ? const _DetailPlaceholder()
                            : HistoryDetail(
                                key: ValueKey<String>(selectedFlow.id.value),
                                flow: selectedFlow,
                              ),
                      ),
                    ),
                  ],
                ),
              ),
              if (_exportOpen)
                HistoryExportModal(
                  onClose: () {
                    setState(() => _exportOpen = false);
                    _tableFocus.requestFocus();
                  },
                ),
              if (sheetFlow != null)
                Positioned(
                  top: 0,
                  right: 0,
                  bottom: 0,
                  child: _SlideInSheet(
                    // `HSheet` keeps the focus with itself and closes on
                    // `Escape` once `onClose` is set; a second focus scope
                    // and a second Escape binding over it would only be two.
                    child: HSheet(
                      title: Text(
                        l10n.historySheetTitle(
                          sheetFlow.methodLabel,
                          sheetFlow.host,
                        ),
                      ),
                      closeSemanticsLabel: l10n.historySheetClose,
                      onClose: _closeSheet,
                      width: historySheetWidth,
                      child: HistoryDetail(flow: sheetFlow),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

/// An action that steps aside while somebody types.
///
/// `Shortcuts` maps a single letter before any action lookup runs, so a bound
/// letter would swallow the keystroke inside the filter field. A disabled
/// action makes `ShortcutManager.handleKeypress` return `ignored`, and the
/// key reaches the field (`docs/UX.md` 5.2).
class _SingleKeyAction<T extends Intent> extends Action<T> {
  _SingleKeyAction(this.run);

  final VoidCallback run;

  /// False while somebody types, and false while the focus sits on a control
  /// that would handle the key itself. A disabled action lets
  /// `ShortcutManager.handleKeypress` answer `ignored`, the key falls through
  /// to the default bindings of `WidgetsApp`, and the focused control wins
  /// (`docs/UX.md` 5.2).
  @override
  bool get isActionEnabled => !isTextInputFocused() && !_focusTakesActivate();

  /// Der Typparameter ist `Intent` und nicht `ActivateIntent`: `Clickable` aus
  /// `shadcn_flutter` legt seine Aktion als `CallbackAction<Intent>` ab, und
  /// `maybeFind<ActivateIntent>` bricht darauf im Entwicklungsbau ab und gibt
  /// im Auslieferungsbau still `null` zurück — die Bildschirmtaste feuerte
  /// dann, obwohl ein Control den Fokus hält (flutter/flutter#180871).
  static bool _focusTakesActivate() {
    final BuildContext? context = FocusManager.instance.primaryFocus?.context;
    if (context == null) {
      return false;
    }
    return Actions.maybeFind<Intent>(context, intent: const ActivateIntent()) !=
        null;
  }

  @override
  Object? invoke(T intent) {
    run();
    return null;
  }
}

/// The detail area before a row is selected.
class _DetailPlaceholder extends StatelessWidget {
  const _DetailPlaceholder();

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Align(
        alignment: Alignment.topLeft,
        child: Text(
          l10n.historyDetailEmptyTitle,
          style: tokens.typography.ui13.tinted(tokens.colors.fg1),
        ),
      ),
    );
  }
}

/// Table over detail, with a splitter between them.
///
/// The share lives in a [ValueNotifier] that only this widget listens to: a
/// state write per pointer move would rebuild the whole screen at the frame
/// rate of the mouse (`docs/UX.md` 7). The drag follows the pointer one to
/// one; only the freed gap animates, and here nothing does (2.9).
class _VerticalSplit extends StatefulWidget {
  const _VerticalSplit({required this.top, required this.bottom});

  final Widget top;
  final Widget bottom;

  @override
  State<_VerticalSplit> createState() => _VerticalSplitState();
}

class _VerticalSplitState extends State<_VerticalSplit> {
  final ValueNotifier<double> _share = ValueNotifier<double>(
    historyDetailShare,
  );
  bool _dragging = false;

  @override
  void dispose() {
    _share.dispose();
    super.dispose();
  }

  /// Moves the split by [pixels] of the [height] available.
  void _move(double pixels, double height) {
    final double next = _share.value - pixels / math.max(height, 1);
    _share.value = next.clamp(historySplitMin, 1 - historySplitMin);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        final double height = constraints.maxHeight;
        return ValueListenableBuilder<double>(
          valueListenable: _share,
          builder: (BuildContext context, double share, Widget? _) {
            final double detail = height * share;
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Expanded(child: widget.top),
                _Splitter(
                  key: const Key('history-splitter'),
                  label: l10n.historySplitterLabel,
                  active: _dragging,
                  color: tokens.colors.line,
                  onStart: () => setState(() => _dragging = true),
                  onUpdate: (double pixels) => _move(pixels, height),
                  onEnd: () => setState(() => _dragging = false),
                ),
                SizedBox(height: detail, child: widget.bottom),
              ],
            );
          },
        );
      },
    );
  }
}

/// Nudges the split by one step.
class _NudgeIntent extends Intent {
  const _NudgeIntent(this.direction);

  /// -1 up, 1 down.
  final double direction;
}

/// The handle between table and detail.
///
/// The vertical counterpart of the splitter in `core/ui/h_resizable_panes`,
/// down to the focus ring and the arrow keys: every pointer gesture has a key
/// (`docs/UX.md` 5.1), and the height is [HSize.splitter], not a spacing
/// token that happens to be the same number.
class _Splitter extends StatefulWidget {
  const _Splitter({
    required this.label,
    required this.active,
    required this.color,
    required this.onStart,
    required this.onUpdate,
    required this.onEnd,
    super.key,
  });

  final String label;
  final bool active;
  final Color color;
  final VoidCallback onStart;
  final ValueChanged<double> onUpdate;
  final VoidCallback onEnd;

  @override
  State<_Splitter> createState() => _SplitterState();
}

class _SplitterState extends State<_Splitter> {
  bool _focused = false;

  void _nudge(double direction) {
    widget
      ..onStart()
      ..onUpdate(direction * HSize.splitterStep)
      ..onEnd();
  }

  @override
  Widget build(BuildContext context) {
    final bool marked = widget.active || _focused;
    return Semantics(
      label: widget.label,
      slider: true,
      child: FocusableActionDetector(
        mouseCursor: SystemMouseCursors.resizeRow,
        onFocusChange: (bool value) => setState(() => _focused = value),
        shortcuts: <ShortcutActivator, Intent>{
          const SingleActivator(LogicalKeyboardKey.arrowUp): const _NudgeIntent(
            -1,
          ),
          const SingleActivator(LogicalKeyboardKey.arrowDown):
              const _NudgeIntent(1),
        },
        actions: <Type, Action<Intent>>{
          _NudgeIntent: CallbackAction<_NudgeIntent>(
            onInvoke: (_NudgeIntent intent) {
              _nudge(intent.direction);
              return null;
            },
          ),
        },
        child: HFocusRing.inline(
          visible: _focused,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            dragStartBehavior: DragStartBehavior.down,
            onVerticalDragStart: (DragStartDetails _) => widget.onStart(),
            onVerticalDragUpdate: (DragUpdateDetails d) =>
                widget.onUpdate(d.delta.dy),
            onVerticalDragEnd: (DragEndDetails _) => widget.onEnd(),
            onVerticalDragCancel: widget.onEnd,
            child: SizedBox(
              height: HSize.splitter,
              child: Center(
                child: SizedBox(
                  // No literal: the dragged line is twice the resting
                  // hairline (`docs/UX.md` 2.1).
                  height: marked ? HSize.splitterActive : HSize.hairline,
                  child: ColoredBox(color: widget.color),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// The sheet, arriving from the edge it hangs on.
///
/// 180 ms, `enter`, eight pixels from the right plus a fade; under reduced
/// motion the path is gone and the fade stays (`docs/UX.md` 2.2 and 2.10).
class _SlideInSheet extends StatefulWidget {
  const _SlideInSheet({required this.child});

  final Widget child;

  @override
  State<_SlideInSheet> createState() => _SlideInSheetState();
}

class _SlideInSheetState extends State<_SlideInSheet>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..forward();

  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // The offset is a fraction of the sheet's own width, which is what
    // `SlideTransition` takes; eight logical pixels of a 520 px sheet.
    final double offset =
        HReducedMotion.distance(context, HMotion.arriveOffset) /
        historySheetWidth;
    return FadeTransition(
      opacity: _curve,
      child: SlideTransition(
        position: Tween<Offset>(
          begin: Offset(offset, 0),
          end: Offset.zero,
        ).animate(_curve),
        child: widget.child,
      ),
    );
  }
}
