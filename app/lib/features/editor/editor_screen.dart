/// Der Pseudonymisierungs-Editor (HUM-047).
///
/// Er ersetzt die Anfrage-Karte im mittleren Pane, solange er offen ist. Links
/// steht, was der Agent geschickt hat, rechts, was hinausginge; darüber die
/// Funde, darunter die Zuordnung und die Entscheidung.
///
/// # Was dieser Bildschirm nicht tut
///
/// Er entscheidet nichts selbst. `Senden` ruft [onSend], und wer ihn einhängt,
/// bestimmt, welcher Weg zum Daemon genommen wird — der Bildschirm der
/// Warteschlange tut das, weil dort jede Entscheidung dieses Programms
/// zusammenläuft. Ein Editor, der selbst entschiede, wäre die zweite Stelle,
/// an der eine Anfrage hinausgeht (`docs/ARCHITECTURE.md` 5).
///
/// Er kennt auch keinen fremden Provider. Was er zum Bauen braucht, reicht der
/// Aufrufer als [DraftSource] herein; der Entwurf selbst lebt danach in
/// `draftProvider(flowId)` und überlebt jedes Schließen mit `Esc`.
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../core/domain/domain.dart';
import '../../core/domain/http_request.dart';
import '../../core/ui/diagnostic_severity.dart';
import '../../core/ui/fix_control.dart';
import '../../core/ui/h_diagnostic_card.dart';
import '../../core/ui/h_resizable_panes.dart';
import '../../core/ui/ui.dart';
import '../../l10n/l10n.dart';
import 'editor_intents.dart';
import 'model/draft.dart';
import 'model/draft_ops.dart';
import 'providers/draft_provider.dart';
import 'widgets/draft_editor.dart';
import 'widgets/findings_rail.dart';
import 'widgets/header_table.dart';
import 'widgets/mapping_strip.dart';
import 'widgets/original_view.dart';

/// Die Mindestbreite je Hälfte des geteilten Editors.
///
/// Unter 320 px passt keine Zeile Mono-12 mehr ohne Umbruch mitten im Wort,
/// und ein Vergleich, bei dem beide Seiten anders umbrechen, vergleicht nichts.
const double editorHalfMinWidth = 320;

/// Der Editor im mittleren Pane.
class EditorScreen extends ConsumerStatefulWidget {
  /// Baut den Editor für [flowId] aus [source].
  const EditorScreen({
    required this.flowId,
    required this.source,
    required this.onClose,
    required this.onSend,
    this.replaceAllOnOpen = false,
    this.canSend = true,
    this.sendDisabledReason = '',
    this.failure,
    super.key,
  });

  /// Der Flow, der bearbeitet wird.
  final FlowId flowId;

  /// Alles, woraus der Entwurf gebaut wird, falls noch keiner steht.
  final DraftSource source;

  /// Schließt den Editor; der Entwurf bleibt stehen.
  final VoidCallback onClose;

  /// Schickt die bearbeitete Fassung.
  final void Function(EditedRequest request, List<Replacement> replacements)
  onSend;

  /// Wahr, wenn der Editor mit jedem offenen Fund schon ersetzt aufgeht.
  ///
  /// So kommt er aus der Pause mit offenen Funden („Pseudonymisieren",
  /// HUM-049): Wer dort pseudonymisieren wollte, soll die Ersetzung sehen und
  /// nicht erst noch einmal „Alle ersetzen" drücken. Ersetzt wird einmal beim
  /// Aufgehen und nur, was noch offen ist; eine Ersetzung, die schon im
  /// Entwurf steht, bleibt, wie sie ist.
  final bool replaceAllOnOpen;

  /// Falsch, solange nicht gesendet werden darf (abgelaufen, schon entschieden).
  final bool canSend;

  /// Warum nicht gesendet werden darf; leer, solange es geht.
  final String sendDisabledReason;

  /// Warum das letzte Senden nichts bewirkt hat, oder null.
  ///
  /// Der Editor bleibt dann stehen: Eine Anfrage, die der Daemon abgelehnt
  /// hat, ist nicht hinausgegangen, und ein Editor, der sich trotzdem
  /// schlösse, behauptete das Gegenteil (`docs/UX.md` 4.4).
  final Diagnostic? failure;

  @override
  ConsumerState<EditorScreen> createState() => _EditorScreenState();
}

class _EditorScreenState extends ConsumerState<EditorScreen> {
  final GlobalKey<DraftEditorState> _editorKey = GlobalKey<DraftEditorState>();
  final FocusNode _focus = FocusNode(debugLabel: 'editor');
  final TextEditingController _label = TextEditingController();
  DraftTab _tab = DraftTab.body;
  bool _prompting = false;
  TextSelection? _selection;

  @override
  void initState() {
    super.initState();
    // Nach dem ersten Rahmen, nicht währenddessen: `load` schreibt in einen
    // Provider, und ein Provider, der im Aufbau geschrieben wird, baut den
    // Baum mitten im Aufbau neu.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      if (!mounted) {
        return;
      }
      final DraftNotifier drafts = ref.read(
        draftProvider(widget.flowId).notifier,
      )..load(widget.source);
      if (widget.replaceAllOnOpen) {
        drafts.replaceAllOpen(aliases: widget.source.aliases);
      }
      // Der Editor nimmt die Tastatur, sobald er aufgeht, und zwar
      // ausdruecklich: `autofocus` reicht nicht, weil es nur greift, solange
      // im umgebenden Bereich nichts anderes den Fokus hat -- und der
      // Bildschirm der Warteschlange hat ihn, wenn `E` den Editor geoeffnet
      // hat. Ohne diesen Griff liefen `Esc`, `Ctrl+Enter`, `F3` und `Ctrl+R`
      // gegen die Bindungen des Bildschirms darunter statt gegen die eigenen:
      // `Shortcuts` loest von der Fokusstelle nach oben auf, nie nach unten
      // (`docs/UX.md` 5.1, 5.2).
      _focus.requestFocus();
    });
  }

  @override
  void dispose() {
    _focus.dispose();
    _label.dispose();
    super.dispose();
  }

  void _send(Draft draft) {
    final ({String body, String? jsonError}) rendered = renderBody(draft);
    widget.onSend(
      buildEditedRequest(
        method: draft.method,
        url: draft.url,
        headers: <({String name, String value})>[
          for (final HeaderEntry entry in draft.headers)
            if (entry.name.isNotEmpty) (name: entry.name, value: entry.value),
        ],
        body: rendered.body,
      ),
      draft.replacements,
    );
  }

  /// Springt zum nächsten offenen Fund im Rumpf.
  ///
  /// „Nächster" heißt: der erste offene, der hinter dem Cursor steht, und
  /// sonst wieder der erste von oben. So kommt man mit wiederholtem `F3` durch
  /// alle und landet am Ende wieder am Anfang, statt am letzten hängen zu
  /// bleiben.
  void _nextFinding(Draft draft) {
    final List<FindingView> open = <FindingView>[
      for (final FindingView view in draft.findings)
        if (view.isOpen && view.location.kind == FindingLocation.body) view,
    ]..sort((FindingView a, FindingView b) => a.start.compareTo(b.start));
    if (open.isEmpty) {
      return;
    }
    final int cursor = _editorKey.currentState?.bodySelection?.end ?? -1;
    final FindingView next = open.firstWhere(
      (FindingView view) => view.start > cursor,
      orElse: () => open.first,
    );
    if (_tab != DraftTab.body) {
      setState(() => _tab = DraftTab.body);
    }
    WidgetsBinding.instance.addPostFrameCallback((Duration _) {
      _editorKey.currentState?.selectRange(next.start, next.end);
    });
  }

  void _pseudonymize() {
    final TextSelection? selection =
        _selection ?? _editorKey.currentState?.bodySelection;
    if (selection == null || selection.isCollapsed) {
      return;
    }
    setState(() => _prompting = true);
  }

  void _confirmPseudonym() {
    final TextSelection? selection =
        _selection ?? _editorKey.currentState?.bodySelection;
    setState(() => _prompting = false);
    if (selection == null || selection.isCollapsed) {
      return;
    }
    ref
        .read(draftProvider(widget.flowId).notifier)
        .replaceSelection(
          DraftLocation.body,
          selection.start,
          selection.end,
          _label.text,
          aliases: widget.source.aliases,
        );
    _label.clear();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Draft? draft = ref.watch(draftProvider(widget.flowId));
    if (draft == null) {
      return const SizedBox.shrink();
    }
    final DraftNotifier notifier = ref.read(
      draftProvider(widget.flowId).notifier,
    );
    final ({String body, String? jsonError}) rendered = renderBody(draft);
    final EditedRequestProblem? problem = checkEditedRequest(
      method: draft.method,
      pathAndQuery: draft.pathAndQuery,
    );
    // Eine Kopfzeile, die der Daemon still fallen ließe oder selbst setzt,
    // hält das Senden genauso an wie ein ungültiger Pfad: Sonst meldete der
    // Editor Erfolg für eine Zeile, die nie ankommt.
    final ({int row, HeaderProblem problem})? headerProblem = checkHeaders(
      draft.headers,
    );
    final bool sendable = problem == null && headerProblem == null;
    return Shortcuts(
      shortcuts: editorShortcuts(),
      child: Actions(
        actions: <Type, Action<Intent>>{
          CloseEditorIntent: CallbackAction<CloseEditorIntent>(
            onInvoke: (CloseEditorIntent intent) {
              if (_prompting) {
                setState(() => _prompting = false);
                return null;
              }
              widget.onClose();
              return null;
            },
          ),
          SendEditedIntent: CallbackAction<SendEditedIntent>(
            onInvoke: (SendEditedIntent intent) {
              if (widget.canSend && sendable) {
                _send(draft);
              }
              return null;
            },
          ),
          NextFindingIntent: CallbackAction<NextFindingIntent>(
            onInvoke: (NextFindingIntent intent) {
              _nextFinding(draft);
              return null;
            },
          ),
          PseudonymizeSelectionIntent:
              CallbackAction<PseudonymizeSelectionIntent>(
                onInvoke: (PseudonymizeSelectionIntent intent) {
                  _pseudonymize();
                  return null;
                },
              ),
        },
        child: Focus(
          focusNode: _focus,
          // Der Editor nimmt die Tastatur, sobald er aufgeht. Ohne das bliebe
          // sie beim Bildschirm darunter, und `Esc`, `Ctrl+Enter`, `F3` und
          // `Ctrl+R` liefen gegen dessen Bindungen statt gegen die eigenen:
          // `Shortcuts` loest von der Fokusstelle nach oben auf, nie nach
          // unten. Wer mit `E` oeffnet, hat gerade die Tastatur benutzt und
          // will sie hier haben (`docs/UX.md` 5.1).
          autofocus: true,
          child: ColoredBox(
            color: tokens.colors.bg0,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                _Header(
                  draft: draft,
                  onClose: widget.onClose,
                  enabled: widget.canSend,
                  onMethod: notifier.setMethod,
                  onPathAndQuery: notifier.setPathAndQuery,
                ),
                FindingsRail(
                  findings: draft.findings,
                  enabled: widget.canSend,
                  onReplaceAll: () =>
                      notifier.replaceAllOpen(aliases: widget.source.aliases),
                  onNext: () => _nextFinding(draft),
                  replaceAllLabel: l10n.editorReplaceAll,
                  nextLabel: l10n.editorNextFinding,
                  cleanLabel: l10n.editorNoFindings,
                  chipLabel: l10n.editorFindingChip,
                ),
                Expanded(
                  child: HResizablePanes(
                    ratios: const <double>[0.5, 0.5],
                    minWidths: const <double>[
                      editorHalfMinWidth,
                      editorHalfMinWidth,
                    ],
                    onRatiosChanged: (List<double> _) {},
                    splitterSemanticsLabel: l10n.editorSplitter,
                    children: <Widget>[
                      OriginalView(
                        text: widget.source.bodyText,
                        findings: draft.findings,
                        label: l10n.editorOriginal,
                      ),
                      DraftEditor(
                        key: _editorKey,
                        draft: draft,
                        tab: _tab,
                        enabled: widget.canSend,
                        onTab: (DraftTab tab) => setState(() => _tab = tab),
                        onBody: notifier.setBody,
                        onHeader: notifier.setHeader,
                        onAddHeader: notifier.addHeader,
                        onRemoveHeader: notifier.removeHeader,
                        onPathAndQuery: notifier.setPathAndQuery,
                        onSelection: (TextSelection selection) =>
                            _selection = selection,
                        tabLabels: <String>[
                          l10n.editorTabBody,
                          l10n.editorTabHeaders,
                          l10n.editorTabQuery,
                        ],
                        lockedLabel: l10n.editorHeaderLocked,
                        addHeaderLabel: l10n.editorHeaderAdd,
                        removeHeaderLabel: l10n.editorHeaderRemove,
                        nameLabel: l10n.editorHeaderName,
                        valueLabel: l10n.editorHeaderValue,
                        bodyLabel: l10n.editorDraft,
                        notEditableLabel: l10n.editorBodyNotEditable,
                      ),
                    ],
                  ),
                ),
                if (_prompting) _prompt(tokens, l10n),
                MappingStrip(
                  replacements: draft.replacements,
                  headingLabel: l10n.editorMapping,
                  pseudonymLabel: l10n.editorMappingPseudonym,
                  typeLabel: l10n.editorMappingType,
                  originalLabel: l10n.editorMappingOriginal,
                ),
                _ActionBar(
                  draft: draft,
                  failure: widget.failure,
                  canSend: widget.canSend && sendable,
                  problem: problem,
                  headerProblem: headerProblem,
                  jsonError: rendered.jsonError,
                  disabledReason: widget.sendDisabledReason,
                  onSend: () => _send(draft),
                  onDiscard: () {
                    notifier.reset(widget.source);
                    widget.onClose();
                  },
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  /// Die Eingabe für `Ctrl+R`: eine Zeile über der Leiste, kein Modal.
  ///
  /// Ein Modal gehört destruktiven Entscheidungen (BACKLOG.md 5); eine
  /// Auswahl zu pseudonymisieren ist das Gegenteil davon, und es ist mit einem
  /// zweiten `Ctrl+R` oder `Esc` wieder weg.
  Widget _prompt(HTokens tokens, AppLocalizations l10n) => DecoratedBox(
    decoration: BoxDecoration(
      color: tokens.colors.bg2,
      border: Border(top: BorderSide(color: tokens.colors.line)),
    ),
    child: Padding(
      padding: EdgeInsets.all(tokens.spacing.x2),
      child: Row(
        children: <Widget>[
          Text(
            l10n.editorPseudonymizeAs,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            child: HTextField(
              key: const Key('editor-pseudonymize-label'),
              controller: _label,
              semanticsLabel: l10n.editorPseudonymizeAs,
              hint: l10n.editorPseudonymizeHint,
              autofocus: true,
              onSubmitted: (String _) => _confirmPseudonym(),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('editor-pseudonymize-confirm'),
            onPressed: () => _confirmPseudonym(),
            child: Text(l10n.editorPseudonymizeConfirm),
          ),
        ],
      ),
    ),
  );
}

/// Wie breit das Methodenfeld ist: genug für `OPTIONS` in Mono-13.
const double methodFieldWidth = 104;

/// Methode, Ziel und Pfad.
///
/// Methode und Pfad sind Felder, das Ziel ist es nicht. Die Trennung ist die
/// Aussage dieses Issues: Über **dieses** Ziel hat ein Mensch entschieden, und
/// der Daemon lehnt jede Verschiebung mit `EDIT_001` ab; Methode und Pfad darf
/// er dagegen ändern, und ohne Felder dafür bliebe „bearbeiten" ein Wort ohne
/// Deckung. Der Pfad trägt die Query mit — beide sind eine Zeichenkette, und
/// zwei getrennte Ablagen liefen auseinander (Fallstrick von HUM-047).
class _Header extends StatefulWidget {
  const _Header({
    required this.draft,
    required this.onClose,
    required this.onMethod,
    required this.onPathAndQuery,
    required this.enabled,
  });

  final Draft draft;
  final VoidCallback onClose;
  final ValueChanged<String> onMethod;
  final ValueChanged<String> onPathAndQuery;
  final bool enabled;

  @override
  State<_Header> createState() => _HeaderState();
}

class _HeaderState extends State<_Header> {
  late final TextEditingController _method = TextEditingController(
    text: widget.draft.method,
  );
  late final TextEditingController _path = TextEditingController(
    text: widget.draft.pathAndQuery,
  );

  @override
  void didUpdateWidget(_Header oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Nur was sich wirklich unterscheidet: Eine Zuweisung setzt den Cursor ans
    // Ende, und wer beim Tippen jedes Mal ans Ende springt, kann nicht tippen
    // (`docs/UX.md` 2.8). Die Methode wird beim Schreiben groß geschrieben,
    // also weicht der Entwurf hier absichtlich von der Eingabe ab.
    if (_method.text.toUpperCase() != widget.draft.method) {
      _method.text = widget.draft.method;
    }
    if (_path.text != widget.draft.pathAndQuery) {
      _path.text = widget.draft.pathAndQuery;
    }
  }

  @override
  void dispose() {
    _method.dispose();
    _path.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Row(
        children: <Widget>[
          SizedBox(
            width: methodFieldWidth,
            child: HTextField(
              key: const Key('editor-method'),
              controller: _method,
              enabled: widget.enabled,
              semanticsLabel: l10n.editorMethod,
              onChanged: widget.onMethod,
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HGlyphIcon(
            HGlyph.lock,
            size: lockedGlyphSize,
            color: tokens.colors.fg2,
            semanticsLabel: l10n.editorAuthorityLocked,
          ),
          SizedBox(width: tokens.spacing.x1),
          Text(
            widget.draft.authority.display(widget.draft.scheme),
            key: const Key('editor-authority'),
            style: tokens.typography.mono13.tinted(tokens.colors.fg2),
          ),
          SizedBox(width: tokens.spacing.x2),
          Expanded(
            child: HTextField(
              key: const Key('editor-path'),
              controller: _path,
              enabled: widget.enabled,
              semanticsLabel: l10n.editorPath,
              onChanged: widget.onPathAndQuery,
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('editor-close'),
            variant: HButtonVariant.ghost,
            onPressed: widget.onClose,
            child: Text(l10n.editorBack),
          ),
        ],
      ),
    );
  }
}

/// Die Leiste des Editors: senden, verwerfen, offene Funde.
class _ActionBar extends StatelessWidget {
  const _ActionBar({
    required this.draft,
    required this.failure,
    required this.canSend,
    required this.problem,
    required this.headerProblem,
    required this.jsonError,
    required this.disabledReason,
    required this.onSend,
    required this.onDiscard,
  });

  final Draft draft;
  final Diagnostic? failure;
  final bool canSend;
  final EditedRequestProblem? problem;
  final ({int row, HeaderProblem problem})? headerProblem;
  final String? jsonError;
  final String disabledReason;
  final VoidCallback onSend;
  final VoidCallback onDiscard;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int open = openFindings(draft);
    final ({int row, HeaderProblem problem})? bad = headerProblem;
    final String badName = bad == null ? '' : draft.headers[bad.row].name;
    final String? note = switch (problem) {
      EditedRequestProblem.method => l10n.editorInvalidMethod(draft.method),
      EditedRequestProblem.path => l10n.editorInvalidPath,
      null => switch (bad?.problem) {
        HeaderProblem.name => l10n.editorHeaderInvalidName(badName),
        HeaderProblem.value => l10n.editorHeaderInvalidValue(badName),
        HeaderProblem.ownedByDaemon => l10n.editorHeaderOwned(badName),
        null => jsonError == null ? null : l10n.editorJsonInvalid,
      },
    };
    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(top: BorderSide(color: tokens.colors.line)),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Wrap(
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: tokens.spacing.x3,
              runSpacing: tokens.spacing.x2,
              children: <Widget>[
                HButton(
                  key: const Key('editor-send'),
                  variant: HButtonVariant.primary,
                  onPressed: canSend ? onSend : null,
                  leading: HGlyphIcon(
                    HGlyph.arrowUpRightPencil,
                    size: lockedGlyphSize,
                    color: tokens.colors.bg0,
                  ),
                  child: Text(l10n.editorSend),
                ),
                HButton(
                  key: const Key('editor-discard'),
                  variant: HButtonVariant.ghost,
                  onPressed: onDiscard,
                  child: Text(l10n.editorDiscard),
                ),
                Text(
                  l10n.editorOpenFindings(open),
                  key: const Key('editor-open-findings'),
                  style: tokens.typography.ui12.tinted(
                    open > 0 ? tokens.stateText.held : tokens.colors.fg1,
                  ),
                ),
              ],
            ),
            // Was der Daemon abgelehnt hat, steht am Ort der Entscheidung und
            // nicht in einem Modal (ADR-012, `docs/UX.md` 4.4). Der Editor
            // bleibt daneben stehen: Die Anfrage ist nicht hinausgegangen.
            if (failure != null) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              Align(
                alignment: Alignment.centerLeft,
                child: HDiagnosticCard(
                  key: const Key('editor-send-error'),
                  code: failure!.code,
                  severityLabel: severityLabel(l10n, failure!.severity),
                  color: severityColor(tokens, failure!.severity),
                  title: l10n.editorSendFailed,
                  why: failure!.why,
                  fix: FixControl(fix: failure!.fix),
                  docsUrl: failure!.docsUrl,
                ),
              ),
            ],
            if (note != null || disabledReason.isNotEmpty) ...<Widget>[
              SizedBox(height: tokens.spacing.x2),
              Text(
                note ?? disabledReason,
                key: const Key('editor-note'),
                style: tokens.typography.ui12.tinted(tokens.stateText.held),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
