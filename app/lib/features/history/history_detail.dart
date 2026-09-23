/// The detail half of the history screen: everything the daemon recorded
/// about one flow.
///
/// The head is built from the row that is already loaded, so selecting a row
/// answers "which request is this" in the same frame. Only the headers and
/// the bodies wait for the daemon, and they wait in their own place
/// (`docs/UX.md` 2.11).
library;

import 'dart:async';

import 'package:flutter/services.dart';
// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/body/body_view.dart';
import '../../core/domain/domain.dart';
import '../../core/ipc/daemon_client.dart';
import '../../core/ui/edited_badge.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'history_filter_bar.dart';
import 'history_metrics.dart';
import 'history_view.dart';
import 'providers/history_detail.dart';

/// Which side of one flow the detail shows.
enum HistoryDetailTab {
  /// The request as it arrived.
  request,

  /// The answer that came back.
  response,

  /// The request as the person changed it before sending.
  edited,
}

/// The detail of one flow.
class HistoryDetail extends ConsumerStatefulWidget {
  /// Creates the detail of [flow].
  const HistoryDetail({required this.flow, super.key});

  /// The row the detail belongs to.
  final Flow flow;

  @override
  ConsumerState<HistoryDetail> createState() => _HistoryDetailState();
}

class _HistoryDetailState extends ConsumerState<HistoryDetail> {
  HistoryDetailTab _tab = HistoryDetailTab.request;
  bool _copied = false;

  @override
  void didUpdateWidget(HistoryDetail oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.flow.id != widget.flow.id) {
      _tab = HistoryDetailTab.request;
      _copied = false;
      return;
    }
    // Das Detail wird je Auswahl einmal geholt, die Zeile darüber lebt. Wird
    // sie zum Datensatz, ist der Abzug von vorhin überholt: der Recorder hat
    // Antwort und bearbeitete Anfrage geschrieben, und ohne dieses Nachfassen
    // bliebe unter einem fertigen Kopf ein Skelett stehen, das nie durch das
    // ersetzt wird, was es beschrieben hat (`docs/UX.md` 2.11). Gefragt wird
    // `isTerminal` und nicht `responseIsFinal`: ein Flow, der oben scheitert,
    // ist schon `failed`, wenn er `recorded` wird, und der Schritt nach
    // `failed` allein brächte nur den laufenden Stand ohne Datensatz.
    //
    // Die bearbeitete Anfrage steht früher fest, aber nicht so früh, wie die
    // Zeile es meldet. Der Daemon veröffentlicht `Decided` zuerst; erst
    // danach übergibt der Proxy `Dir::RequestEdited` dem Schreiber
    // (`daemon/crates/proxy/src/handler.rs`), und der schreibt in Stapeln
    // alle 50 ms (`daemon/crates/recorder/src/writer.rs`, `BATCH_INTERVAL`).
    // Ein `GetFlow` gleich nach `Decided` kann deshalb noch ohne sie
    // zurückkommen. Nachgefasst wird darum bei jedem Schritt, den der Flow
    // danach noch macht — `forwarded`, `responded` —, solange die
    // bearbeitete Anfrage fehlt. Das verkleinert das Fenster, schließt es
    // aber nicht: fallen beide Schritte in dieselben 50 ms, wartet der Tab
    // bis `recorded`. Schließen kann es nur der Daemon, der sie für einen
    // laufenden Flow unabhängig vom Schreiber ausliefert (offen nach HUM-154
    // in `backlog/sprint-4.md`).
    //
    // Nachgefasst wird nur, was noch nicht da ist. Das Blatt kann den
    // Datensatz schon geholt haben, bevor es seine Zeile weiterreicht (siehe
    // `_refreshSheetFlow` im Bildschirm); ein zweites `GetFlow` für dasselbe
    // Ende wäre eine Frage, deren Antwort schon auf dem Tisch liegt.
    final FlowDetail? held = ref
        .read(historyDetailProvider(widget.flow.id))
        .value;
    final bool recordWritten =
        !oldWidget.flow.state.isTerminal &&
        widget.flow.state.isTerminal &&
        !(held?.summary.state.isTerminal ?? false);
    final bool editedArrived =
        oldWidget.flow.edited != widget.flow.edited &&
        held?.editedRequest == null;
    final bool editedStillMissing =
        widget.flow.edited &&
        held?.editedRequest == null &&
        oldWidget.flow.state != widget.flow.state &&
        (widget.flow.state == FlowState.forwarded ||
            widget.flow.state == FlowState.responded);
    if (recordWritten || editedArrived || editedStillMissing) {
      // Nach dem Bild, nicht mittendrin: `didUpdateWidget` läuft innerhalb
      // eines Baus, und ein Provider, der dort ungültig gemacht wird, ruft
      // `markNeedsBuild` während desselben Baus.
      final FlowId id = widget.flow.id;
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        if (mounted) {
          ref.invalidate(historyDetailProvider(id));
        }
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow flow = widget.flow;
    final AsyncValue<FlowDetail> detail = ref.watch(
      historyDetailProvider(flow.id),
    );
    final bool hasEdited = flow.edited;
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        // The head is capped and scrolls rather than pushing the tabs out of
        // the pane: the split can be dragged small, and at
        // `TextScaler.linear(2.0)` the six facts alone are taller than a
        // short pane (`docs/UX.md` 6, no fixed height around text).
        final Widget head = ConstrainedBox(
          constraints: BoxConstraints(maxHeight: constraints.maxHeight * 0.5),
          child: SingleChildScrollView(child: _Head(flow: flow)),
        );
        final Widget tabs = Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x2,
          ),
          child: Row(
            children: <Widget>[
              for (final HistoryDetailTab tab in HistoryDetailTab.values)
                if (tab != HistoryDetailTab.edited || hasEdited)
                  Padding(
                    padding: EdgeInsets.only(right: tokens.spacing.x2),
                    child: HButton(
                      key: Key('history-tab-${tab.name}'),
                      variant: _tab == tab
                          ? HButtonVariant.secondary
                          : HButtonVariant.ghost,
                      onPressed: () => setState(() => _tab = tab),
                      child: Text(_tabLabel(l10n, tab)),
                    ),
                  ),
            ],
          ),
        );
        // Ist das Detail breit genug, steht der Rumpf rechts neben Kopf,
        // Tabs und Kopfzeilen und bekommt die ganze Höhe des Detailbereichs
        // (HUM-153). Untereinander blieben ihm bei 1400 × 900 knapp hundert
        // Pixel: genug für Titel und Umschalter, nicht für den Inhalt. Die
        // linke Spalte ist so breit wie das Textmaß der Fakten und
        // Kopfzeilen (`docs/UX.md` 3.2, 90 Zeichen `mono12`); was darüber
        // hinausginge, wäre dort Rinne und ist hier Platz für den Rumpf.
        // Nebeneinander stehen beide nur, wenn der Rumpf mindestens so breit
        // wird wie die linke Spalte, also nicht im schmalen Blatt und nicht
        // bei großer Schrift. Wechselt die Anordnung, weil das Fenster oder
        // die Schrift sich ändert, baut sich der Rumpf neu auf und beginnt
        // wieder oben; die Ansicht (Baum, Text, Hex) bleibt, sie steht im
        // Provider.
        final double headWidth = HSize.measureWidth(
          MediaQuery.textScalerOf(context)
              .scale(tokens.typography.mono12.fontSize!),
        );
        if (constraints.maxWidth < headWidth * 2) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              head,
              const HHairline(),
              tabs,
              Flexible(child: _pane(tokens, detail, _TabPart.both)),
            ],
          );
        }
        // Die Tab-Taste geht erst durch die linke Spalte, dann durch den
        // Rumpf, wie im schmalen Detail. Nach Lesereihenfolge käme der Rumpf,
        // der oben beginnt, vor den Tabs, die tiefer stehen.
        return FocusTraversalGroup(
          policy: OrderedTraversalPolicy(),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              FocusTraversalOrder(
                order: const NumericFocusOrder(1),
                child: FocusTraversalGroup(
                  child: SizedBox(
                    key: const Key('history-detail-head-column'),
                    width: headWidth,
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        head,
                        const HHairline(),
                        tabs,
                        Expanded(
                          child: _pane(tokens, detail, _TabPart.headers),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
              const HHairline(vertical: true),
              Expanded(
                key: const Key('history-detail-body-column'),
                child: FocusTraversalOrder(
                  order: const NumericFocusOrder(2),
                  child: FocusTraversalGroup(
                    child: _pane(tokens, detail, _TabPart.body),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  /// Was unter den Tabs steht, ganz oder zur Hälfte ([part]), für den Stand
  /// [detail] des Abzugs.
  ///
  /// Warten und Fehlschlag stehen dort, wo der Inhalt stünde: das Skelett in
  /// beiden Spalten in deren Dichte, die Fehlerkarte am Rumpf, wo der Blick
  /// ohnehin hingeht (`docs/UX.md` 2.11, 4.4).
  Widget _pane(HTokens tokens, AsyncValue<FlowDetail> detail, _TabPart part) =>
      switch (detail) {
        AsyncData<FlowDetail>(:final FlowDetail value) => _TabBody(
          tab: _tab,
          part: part,
          // Die lebende Zeile, nicht die Zusammenfassung im Abzug: nur
          // sie weiß, ob gerade noch etwas ankommt (HUM-154).
          flow: widget.flow,
          detail: value,
          copied: _copied,
          onCopy: (String text) {
            unawaited(Clipboard.setData(ClipboardData(text: text)));
            setState(() => _copied = true);
          },
        ),
        AsyncError<FlowDetail>(:final Object error) =>
          part == _TabPart.headers
              ? const SizedBox.shrink()
              : Padding(
                  padding: EdgeInsets.all(tokens.spacing.x3),
                  child: _Failure(error: error),
                ),
        _ => HistoryWaitGate(
          child: _BodySkeleton(lines: part == _TabPart.headers ? 4 : 10),
        ),
      };

  String _tabLabel(AppLocalizations l10n, HistoryDetailTab tab) =>
      switch (tab) {
        HistoryDetailTab.request => l10n.historyDetailTabRequest,
        HistoryDetailTab.response => l10n.historyDetailTabResponse,
        HistoryDetailTab.edited => l10n.historyDetailTabEdited,
      };
}

/// The head: the URL, and the six facts that belong to it.
///
/// The URL is the largest type on this screen (`docs/UX.md` 3.1) and is
/// selectable, because comparing it against a rule is what a person does
/// here. `SelectableRegion` is the widgets-layer equivalent of
/// `SelectableText`; this application has no Material.
class _Head extends StatelessWidget {
  const _Head({required this.flow});

  final Flow flow;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final HFlowState state = historyVisualState(flow);
    final String unknown = l10n.historyUnknownValue;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            children: <Widget>[
              // The one badge that carries the method hue: in the head there
              // is no rail beside it to be confused with (`docs/UX.md` 3.3,
              // rule 4).
              HMethodBadge(method: flow.methodLabel),
              SizedBox(width: tokens.spacing.x2),
              HStateGlyph(
                state: state,
                semanticsLabel: l10n.flowStateLabel(state),
              ),
              SizedBox(width: tokens.spacing.x1),
              Text(
                l10n.flowStateLabel(state),
                // The text-capable reading, never `stateColor`: that palette
                // is clamped to 3:1 as an area (`docs/UX.md` 6).
                style: tokens.typography.ui12.tinted(
                  tokens.stateTextColor(state),
                ),
              ),
              // Derselbe Chip wie in der Zeile darüber und in der
              // Warteschlange (HUM-047). Er hängt an `edited` und nicht am
              // Zustand: Eine bearbeitete Anfrage, deren Ziel danach
              // scheiterte, steht als Fehler da und ging trotzdem bearbeitet
              // hinaus.
              //
              // Für den Screenreader ist er stumm, in jedem Zustand: Das Wort
              // steht schon im Zustand daneben („Allowed, edited“) und, auch
              // nach einem Fehler des Ziels, in der Tatsache „Decision“
              // darunter, die `historyDecisionLabelState` aus der Entscheidung
              // und nicht aus dem Zustand liest. Ein drittes „Edited“ wäre nur
              // Wiederholung.
              if (flow.edited) ...<Widget>[
                SizedBox(width: tokens.spacing.x2),
                const ExcludeSemantics(
                  child: EditedBadge(key: Key('history-detail-edited')),
                ),
              ],
            ],
          ),
          SizedBox(height: tokens.spacing.x1),
          SelectableRegion(
            selectionControls: emptyTextSelectionControls,
            child: Text(
              flow.url,
              style: tokens.typography.mono14.tinted(tokens.colors.fg0),
              maxLines: 2,
            ),
          ),
          SizedBox(height: tokens.spacing.x2),
          Wrap(
            spacing: tokens.spacing.x4,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[
              _Fact(
                label: l10n.historyDetailReceived,
                value: formatHistoryTimestamp(flow.receivedAt),
              ),
              _Fact(
                label: l10n.historyDetailDecision,
                // What was decided, not how the row looks: an allow whose
                // upstream failed was still an allow, and the state is on
                // the glyph right above (`backlog/CONVENTIONS.md` 4.13).
                // A meta request has no decision and never will: the proxy
                // answered it itself, and a dash would read as "unknown"
                // (`backlog/CONVENTIONS.md` 4.13, HUM-103).
                value: switch (flow.decision) {
                  null when flow.meta => l10n.historyDecisionNone,
                  null => unknown,
                  final DecisionKind decision => l10n.flowStateLabel(
                    historyDecisionLabelState(decision),
                  ),
                },
              ),
              _Fact(label: l10n.historyDetailRule, value: _decider(l10n, flow)),
              _Fact(
                label: l10n.historyDetailDuration,
                value: formatHistoryDuration(flow, unknown: unknown),
              ),
              _Fact(
                label: l10n.historyDetailSize,
                value: historyResponseStreaming(flow)
                    // The answer is still running in, so the number is a
                    // running total and says so (`backlog/sprint-2.md`,
                    // HUM-032: bei Response `streaming` die Live-Größe).
                    ? '${formatHistorySizePair(flow, unknown: unknown)} · '
                          '${l10n.historyDetailStreaming}'
                    : formatHistorySizePair(flow, unknown: unknown),
              ),
              _Fact(
                label: l10n.historyDetailFindings,
                value: '${flow.findingCount}',
                findings: flow.findingCount > 0,
              ),
              // How many findings left unresolved with the allow (HUM-160):
              // 0 after "Send anyway", the open ones after holding the valve.
              // Only where the daemon counted; no count, no fact, never a
              // guessed zero.
              if (flow.unresolvedFindings case final int unresolved)
                _Fact(
                  label: l10n.historyDetailUnresolvedFindings,
                  value: '$unresolved',
                  findings: unresolved > 0,
                ),
            ],
          ),
          // The note stands under the decision it explains, on its own line:
          // it is free text of up to 500 characters and would break the row of
          // short facts above (HUM-117). No note, no line — an empty one would
          // read as "there was nothing to say".
          if (flow.decisionNote.isNotEmpty) ...<Widget>[
            SizedBox(height: tokens.spacing.x2),
            SelectableRegion(
              selectionControls: emptyTextSelectionControls,
              child: Text(
                l10n.historyDetailNote(flow.decisionNote),
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ),
          ],
        ],
      ),
    );
  }

  String _decider(AppLocalizations l10n, Flow flow) =>
      switch (historyDecider(flow)) {
        HistoryDecider.rule => l10n.historyDeciderRule(
          flow.ruleId == null ? '' : historyRuleShort(flow.ruleId!),
        ),
        HistoryDecider.manual => l10n.historyDeciderManual,
        HistoryDecider.timeout => l10n.historyDeciderTimeout,
        HistoryDecider.passthrough => l10n.historyDeciderPassthrough,
        HistoryDecider.pending => l10n.historyDeciderPending,
        HistoryDecider.meta => l10n.historyDeciderMeta,
      };
}

/// One label and its value in the head.
class _Fact extends StatelessWidget {
  const _Fact({
    required this.label,
    required this.value,
    this.findings = false,
  });

  final String label;
  final String value;

  /// Puts the value in the findings colour: `stateTextColor`, the
  /// text-capable reading of the state palette, never the area colour
  /// (`docs/UX.md` 6).
  final bool findings;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Text(label, style: tokens.typography.ui12.tinted(tokens.colors.fg1)),
        SizedBox(width: tokens.spacing.x1),
        Text(
          value,
          style: tokens.typography.mono12.tinted(
            findings
                ? tokens.stateTextColor(HFlowState.error)
                : tokens.colors.fg0,
          ),
        ),
      ],
    );
  }
}

/// Welcher Teil eines Tabs an einer Stelle steht.
enum _TabPart {
  /// Kopfzeilen und Rumpf untereinander, im schmalen Detail.
  both,

  /// Nur die Kopfzeilen, in der linken Spalte des breiten Details.
  headers,

  /// Nur der Rumpf, in der rechten Spalte des breiten Details.
  body,
}

/// Headers and body of one tab, or one of the two ([part]).
class _TabBody extends ConsumerWidget {
  const _TabBody({
    required this.tab,
    required this.part,
    required this.flow,
    required this.detail,
    required this.copied,
    required this.onCopy,
  });

  final HistoryDetailTab tab;

  /// Ob Kopfzeilen, Rumpf oder beides.
  final _TabPart part;

  /// Die Zeile, wie sie jetzt steht; sie folgt den Ereignissen des Daemons.
  final Flow flow;

  final FlowDetail detail;
  final bool copied;
  final ValueChanged<String> onCopy;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final List<Header> headers = switch (tab) {
      HistoryDetailTab.request => detail.request?.headers ?? const <Header>[],
      HistoryDetailTab.edited =>
        detail.editedRequest?.headers ?? const <Header>[],
      HistoryDetailTab.response => detail.response?.headers ?? const <Header>[],
    };
    final BodyRef? body = switch (tab) {
      HistoryDetailTab.request => detail.request?.body,
      HistoryDetailTab.edited => detail.editedRequest?.body,
      HistoryDetailTab.response => detail.responseBody,
    };
    // Die Funde des Daemons liegen im Rumpf der Anfrage, so wie sie ankam.
    // Eine Antwort durchsucht er nicht, und im bearbeiteten Rumpf stehen die
    // Stellen nicht mehr dort, wo er sie gefunden hat.
    final List<Finding> findings = switch (tab) {
      HistoryDetailTab.request => detail.findings,
      HistoryDetailTab.response || HistoryDetailTab.edited => const <Finding>[],
    };
    final bool missing =
        tab == HistoryDetailTab.response && detail.response == null;
    // Solange am Flow noch etwas geschrieben werden kann, hat eine fehlende
    // Seite nichts zu bedeuten. Das ist Warten, keine Aussage
    // (`docs/UX.md` 2.11) — aber die beiden Seiten warten auf Verschiedenes,
    // und deshalb stehen hier zwei Fragen:
    //
    // * Die Antwort wartet, solange noch Bytes kommen können.
    //   `responseIsFinal` ist genau die Frage, die die Größe im Kopf stellt,
    //   und sie wird an derselben Stelle gestellt, damit Kopf und Abschnitt
    //   nicht auseinanderlaufen.
    // * Die bearbeitete Anfrage wartet, solange sie fehlt und der Datensatz
    //   nicht geschrieben ist: ein `GetFlow` auf einen laufenden Flow trägt
    //   sie erst, wenn der Schreiber sie übernommen hat, bis dahin kann dort
    //   `None` stehen; sicher da ist sie erst bei `recorded`. Gefragt wird
    //   also `isTerminal`, und das ist allein `recorded`. `failed` ist kein Ende
    //   (`daemon/crates/core-types/src/flow.rs`: `Failed` + `Record` =
    //   `Recorded`, und `fail_closed` bringt jeden Zustand dorthin), also
    //   behauptet ein oben gescheiterter Flow hier nichts über eine Anfrage,
    //   die jemand von Hand bearbeitet hat.
    //
    // Die Anfrage selbst ist nie „noch unterwegs": ohne sie gäbe es den Flow
    // nicht.
    final bool pending = switch (tab) {
      HistoryDetailTab.request => false,
      HistoryDetailTab.response => !responseIsFinal(flow),
      HistoryDetailTab.edited =>
        detail.editedRequest == null && !flow.state.isTerminal,
    };
    final Widget headerTable = _Headers(
      headers: headers,
      copied: copied,
      onCopy: onCopy,
      emptyLabel: missing
          ? l10n.historyDetailNoResponse
          : l10n.historyDetailNoHeaders,
      pending: pending,
    );
    final Widget bodyView = _Body(
      flowId: detail.summary.id,
      reference: body,
      headers: headers,
      findings: findings,
      pending: pending,
    );
    return switch (part) {
      _TabPart.headers => headerTable,
      _TabPart.body => bodyView,
      _TabPart.both => LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ConstrainedBox(
                constraints: BoxConstraints(
                  maxHeight: constraints.maxHeight * 0.45,
                ),
                child: headerTable,
              ),
              const HHairline(),
              Expanded(child: bodyView),
            ],
          );
        },
      ),
    };
  }
}

/// The header table of one tab: name, value, and a copy control.
class _Headers extends StatefulWidget {
  const _Headers({
    required this.headers,
    required this.copied,
    required this.onCopy,
    required this.emptyLabel,
    required this.pending,
  });

  final List<Header> headers;
  final bool copied;
  final ValueChanged<String> onCopy;

  /// Was an der Stelle der Kopfzeilen steht, wenn keine da sind und keine
  /// mehr kommen.
  final String emptyLabel;

  /// True, solange diese Seite noch einläuft; siehe [BodyView.pending].
  ///
  /// Dann steht statt [emptyLabel] ein Skelett: auf Warten antwortet dieser
  /// Bildschirm mit der Skizze der Zeilen, die gleich kommen, nicht mit einem
  /// Satz über etwas, das noch niemand weiß (`docs/UX.md` 2.11).
  final bool pending;

  @override
  State<_Headers> createState() => _HeadersState();
}

class _HeadersState extends State<_Headers> {
  bool _ascending = true;

  /// Width of the name column. Wide enough for `content-security-policy`,
  /// which is the longest header a person meets often.
  static const double _nameWidth = 180;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final List<Header> sorted = List<Header>.of(widget.headers)
      ..sort(
        (Header a, Header b) =>
            _ascending ? a.name.compareTo(b.name) : b.name.compareTo(a.name),
      );
    // One list, title row included: a column with a fixed head and a
    // scrolling tail overflows as soon as the head grows -- and at
    // `TextScaler.linear(2.0)` it does (`docs/UX.md` 6).
    return ListView.builder(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      itemCount: 1 + (sorted.isEmpty ? 1 : sorted.length),
      itemBuilder: (BuildContext context, int index) {
        if (index == 0) {
          return _title(tokens, l10n, sorted);
        }
        if (sorted.isEmpty) {
          if (widget.pending) {
            // Dasselbe Warten wie unter den Tabs, mit derselben Schwelle: ein
            // Skelett, das kürzer stünde als eine Reaktionszeit, wäre ein
            // Flackern (`docs/UX.md` 2.11). Die Höhe steht fest, weil das
            // Skelett selbst eine Liste ist und in einer Liste keine
            // unbegrenzte Höhe bekommen darf.
            return const SizedBox(
              height: historyBodyRowHeight * 4,
              child: HistoryWaitGate(
                child: _BodySkeleton(
                  key: Key('history-detail-headers-pending'),
                  lines: 4,
                ),
              ),
            );
          }
          return Padding(
            padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
            child: Text(
              widget.emptyLabel,
              style: tokens.typography.ui13.tinted(tokens.colors.fg1),
            ),
          );
        }
        final Header header = sorted[index - 1];
        return Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x1 / 2,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              SizedBox(
                width: _nameWidth,
                child: Text(
                  header.name,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(
                child: Text(
                  header.text,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _title(HTokens tokens, AppLocalizations l10n, List<Header> sorted) =>
      Padding(
        padding: EdgeInsets.fromLTRB(
          tokens.spacing.x3,
          tokens.spacing.x2,
          tokens.spacing.x3,
          tokens.spacing.x1,
        ),
        child: Row(
          children: <Widget>[
            HButton(
              variant: HButtonVariant.ghost,
              onPressed: () => setState(() => _ascending = !_ascending),
              semanticsLabel: _ascending
                  ? l10n.historySortedAscending(l10n.historyDetailHeaders)
                  : l10n.historySortedDescending(l10n.historyDetailHeaders),
              child: Text(l10n.historyDetailHeaders),
            ),
            const Spacer(),
            if (sorted.isNotEmpty)
              HButton(
                variant: HButtonVariant.ghost,
                onPressed: () => widget.onCopy(
                  <String>[
                    for (final Header header in sorted)
                      '${header.name}: ${header.text}',
                  ].join('\n'),
                ),
                child: Text(
                  widget.copied
                      ? l10n.historyDetailCopied
                      : l10n.historyDetailCopy,
                ),
              ),
          ],
        ),
      );
}

/// The recorded body of one tab, in the view the queue uses.
///
/// Baum, Formular, Roh und Hex, ausgepackt nach dem `Content-Encoding` der
/// eigenen Seite, mit den Funden an ihrer Stelle (`backlog/sprint-2.md`,
/// HUM-116). Die Bytes kommen über den einen Rumpf-Provider in `core`, mit
/// seinem Zwischenspeicher und seinen Grenzen; dieser Bildschirm fügt keine
/// eigenen hinzu, auch kein zweites `Isolate.run`.
///
/// Auch der Satz über einen Rumpf, der nichts enthält, kommt von dort: das
/// Detail beschriftet ihn seit HUM-154 nicht mehr selbst. Was es beisteuert,
/// ist [pending] — nur es weiß, ob eine Seite noch einläuft.
class _Body extends StatelessWidget {
  const _Body({
    required this.flowId,
    required this.reference,
    required this.headers,
    required this.findings,
    required this.pending,
  });

  final FlowId flowId;
  final BodyRef? reference;
  final List<Header> headers;
  final List<Finding> findings;

  /// True, solange diese Seite noch ankommt; siehe [BodyView.pending].
  final bool pending;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return SingleChildScrollView(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: BodyView(
        flowId: flowId,
        body: reference,
        headers: headers,
        findings: findings,
        pending: pending,
      ),
    );
  }
}

/// Holds a waiting display back until waiting is worth showing.
///
/// Nothing for the first [HMotion.waitVisible] (150 ms), because a display
/// shorter than a reaction time reads as a flicker (`docs/UX.md` 2.11). What
/// this gate does not do is hold the display for [HMotion.waitMinVisible]
/// once it is up; that half of the rule lives where the waiting ends, in the
/// table's own gate, because a `FutureProvider` swaps its child in one frame
/// and a wrapper cannot delay a sibling.
class HistoryWaitGate extends StatefulWidget {
  /// Wraps the waiting display [child].
  const HistoryWaitGate({required this.child, super.key});

  /// What is shown once waiting is admitted.
  final Widget child;

  @override
  State<HistoryWaitGate> createState() => _HistoryWaitGateState();
}

class _HistoryWaitGateState extends State<HistoryWaitGate> {
  bool _visible = false;
  Timer? _appear;

  @override
  void initState() {
    super.initState();
    _appear = Timer(HMotion.waitVisible, () {
      if (mounted) {
        setState(() => _visible = true);
      }
    });
  }

  @override
  void dispose() {
    _appear?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      _visible ? widget.child : const SizedBox.expand();
}

/// Hairlines in the body density while the body is on its way.
class _BodySkeleton extends StatelessWidget {
  const _BodySkeleton({required this.lines, super.key});

  final int lines;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    // A list, not a column: the skeleton stands for lines that scroll, and a
    // column of them would overflow a short pane instead of being cut off
    // where the real lines are. The four lengths are fractions of the pane,
    // not pixel counts, so the sketch stays a sketch at any width.
    const List<double> shares = <double>[0.7, 0.4, 0.85, 0.55];
    return ExcludeSemantics(
      child: LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) =>
            ListView.builder(
              padding: EdgeInsets.all(tokens.spacing.x3),
              physics: const NeverScrollableScrollPhysics(),
              itemExtent: historyBodyRowHeight,
              itemCount: lines,
              itemBuilder: (BuildContext context, int index) => Align(
                alignment: Alignment.centerLeft,
                child: HHairline(
                  color: HColorDerivation.fade(tokens.colors.fg2, 0.4),
                  length:
                      (constraints.maxWidth - tokens.spacing.x6) *
                      shares[index % shares.length],
                ),
              ),
            ),
      ),
    );
  }
}

/// A failed load, anchored where the content would have stood.
class _Failure extends StatelessWidget {
  const _Failure({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Object error = this.error;
    final Diagnostic? diagnostic = error is DaemonException
        ? error.diagnostic
        : null;
    if (diagnostic == null) {
      return Text(
        l10n.historyDetailFailedTitle,
        style: tokens.typography.ui13.tinted(
          tokens.stateTextColor(HFlowState.error),
        ),
      );
    }
    return HDiagnosticCard(
      code: diagnostic.code,
      severityLabel: historySeverityLabel(l10n, diagnostic.severity),
      color: historySeverityColor(tokens, diagnostic.severity),
      title: l10n.historyDetailFailedTitle,
      why: diagnostic.why,
      docsUrl: diagnostic.docsUrl,
      width: double.infinity,
    );
  }
}
