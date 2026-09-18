/// Die rechte Hälfte des Editors: der Entwurf, den ein Mensch ändert.
///
/// Drei Reiter auf demselben Entwurf — Rumpf, Kopfzeilen, Query. Sie sind
/// Sichten und keine Kopien: Der Query-Reiter schreibt in `pathAndQuery`
/// zurück, aus dem er kommt, und kein Wert steht zweimal (`backlog/sprint-4.md`,
/// HUM-047 Fallstricke).
library;

import 'package:flutter/material.dart'
    show AdaptiveTextSelectionToolbar, materialTextSelectionControls;
import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../model/draft.dart';
import 'header_table.dart';

/// Welcher Reiter des Entwurfs offen ist.
enum DraftTab {
  /// Der Rumpf.
  body,

  /// Die Kopfzeilen.
  headers,

  /// Die Query als Schlüssel-Wert-Tabelle.
  query,
}

/// Der bearbeitbare Entwurf.
class DraftEditor extends StatefulWidget {
  /// Baut den Editor.
  const DraftEditor({
    required this.draft,
    required this.tab,
    required this.onTab,
    required this.onBody,
    required this.onHeader,
    required this.onAddHeader,
    required this.onRemoveHeader,
    required this.onPathAndQuery,
    required this.tabLabels,
    required this.lockedLabel,
    required this.addHeaderLabel,
    required this.removeHeaderLabel,
    required this.nameLabel,
    required this.valueLabel,
    required this.bodyLabel,
    required this.notEditableLabel,
    this.onSelection,
    this.enabled = true,
    super.key,
  });

  /// Der Entwurf.
  final Draft draft;

  /// Der offene Reiter.
  final DraftTab tab;

  /// Wird mit dem gewählten Reiter gerufen.
  final ValueChanged<DraftTab> onTab;

  /// Wird mit dem neuen Rumpf gerufen.
  final ValueChanged<String> onBody;

  /// Wird mit Index, Name und Wert einer geänderten Kopfzeile gerufen.
  final void Function(int index, String name, String value) onHeader;

  /// Hängt eine leere Kopfzeile an.
  final VoidCallback onAddHeader;

  /// Entfernt die Kopfzeile an dieser Stelle.
  final ValueChanged<int> onRemoveHeader;

  /// Wird mit dem neuen `pathAndQuery` gerufen.
  final ValueChanged<String> onPathAndQuery;

  /// Wird mit der Auswahl im Rumpf gerufen, für `Ctrl+R`.
  final ValueChanged<TextSelection>? onSelection;

  /// Falsch, solange der Rumpf nicht bearbeitet werden darf.
  final bool enabled;

  /// Die drei Reiterbeschriftungen, in der Reihenfolge von [DraftTab].
  final List<String> tabLabels;

  /// Das Wort für eine gesperrte Kopfzeile.
  final String lockedLabel;

  /// Die Beschriftung des Hinzufügen-Knopfes.
  final String addHeaderLabel;

  /// Die Beschriftung des Entfernen-Knopfes.
  final String removeHeaderLabel;

  /// Die Spaltenüberschrift für den Namen.
  final String nameLabel;

  /// Die Spaltenüberschrift für den Wert.
  final String valueLabel;

  /// Die Überschrift der Rumpf-Hälfte.
  final String bodyLabel;

  /// Was dasteht, solange der Rumpf nicht bearbeitet werden kann.
  final String notEditableLabel;

  @override
  State<DraftEditor> createState() => DraftEditorState();
}

/// Der Zustand des Entwurfs-Editors; öffentlich, damit der Bildschirm die
/// Auswahl lesen kann, die `Ctrl+R` pseudonymisiert.
class DraftEditorState extends State<DraftEditor> {
  late final HDecoratedTextController _body = HDecoratedTextController(
    text: widget.draft.body,
  );
  final FocusNode _bodyFocus = FocusNode(debugLabel: 'editor-body');
  final GlobalKey<EditableTextState> _bodyKey = GlobalKey<EditableTextState>(
    debugLabel: 'editor-draft-body',
  );
  late final TextSelectionGestureDetectorBuilder _gestures =
      TextSelectionGestureDetectorBuilder(delegate: _BodyGestures(_bodyKey));

  /// Die Auswahl im Rumpf, oder null, wenn nichts ausgewählt ist.
  TextSelection? get bodySelection {
    final TextSelection selection = _body.selection;
    return selection.isValid && !selection.isCollapsed ? selection : null;
  }

  /// Legt die Auswahl auf `[start, end)` und holt den Fokus dorthin.
  ///
  /// Das ist, was „springen" heißt: Die Auswahl ist zugleich die Marke, die
  /// ein Mensch sieht, und die Stelle, die das Feld ins Bild scrollt. Ein
  /// Knopf „Nächstes", der nur einen Reiter wechselte, verspräche einen
  /// Sprung und täte keinen (`docs/UX.md` 5.3).
  void selectRange(int start, int end) {
    final int length = _body.text.length;
    if (start < 0 || end > length || start >= end) {
      return;
    }
    _bodyFocus.requestFocus();
    _body.selection = TextSelection(baseOffset: start, extentOffset: end);
  }

  @override
  void initState() {
    super.initState();
    _body.update(_decorations());
    _body.addListener(_reportSelection);
  }

  @override
  void didUpdateWidget(DraftEditor oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Der Text wird nur gesetzt, wenn er sich wirklich unterscheidet: Eine
    // Zuweisung setzt den Cursor ans Ende, und wer beim Tippen jedes Mal ans
    // Ende springt, kann nicht tippen (`docs/UX.md` 2.8).
    if (_body.text != widget.draft.body) {
      final TextSelection before = _body.selection;
      _body.text = widget.draft.body;
      if (before.isValid && before.end <= widget.draft.body.length) {
        _body.selection = before;
      }
    }
    _body.update(_decorations());
  }

  @override
  void dispose() {
    _body
      ..removeListener(_reportSelection)
      ..dispose();
    _bodyFocus.dispose();
    super.dispose();
  }

  void _reportSelection() => widget.onSelection?.call(_body.selection);

  /// Erst die Funde, dann die Ersetzungen: Eine erledigte Stelle leuchtet und
  /// ist nicht mehr unterstrichen, und in [HEditorDecorations.slice] gewinnt
  /// die spätere Markierung.
  List<HEditorDecoration> _decorations() => <HEditorDecoration>[
    for (final FindingView view in widget.draft.findings)
      if (view.location.kind == FindingLocation.body && view.isOpen)
        HEditorDecoration(
          start: view.start,
          end: view.end,
          kind: view.finding.tier == FindingTier.checksum
              ? HEditorDecorationKind.secret
              : HEditorDecorationKind.pii,
        ),
    for (final Replacement done in widget.draft.replacements)
      if (done.location.kind == FindingLocation.body)
        HEditorDecoration(
          start: done.start,
          end: done.end,
          kind: HEditorDecorationKind.replaced,
        ),
  ];

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.symmetric(
            horizontal: tokens.spacing.x3,
            vertical: tokens.spacing.x2,
          ),
          child: Align(
            alignment: Alignment.centerLeft,
            child: HSegmented<DraftTab>(
              key: const Key('editor-tabs'),
              options: <HSegmentOption<DraftTab>>[
                for (final DraftTab tab in DraftTab.values)
                  HSegmentOption<DraftTab>(
                    value: tab,
                    label: widget.tabLabels[tab.index],
                  ),
              ],
              selected: widget.tab,
              onSelect: widget.onTab,
            ),
          ),
        ),
        const HHairline(),
        Expanded(child: _pane(tokens)),
      ],
    );
  }

  Widget _pane(HTokens tokens) => switch (widget.tab) {
    DraftTab.body => _bodyPane(tokens),
    DraftTab.headers => HeaderTable(
      headers: widget.draft.headers,
      onChanged: widget.onHeader,
      onAdd: widget.onAddHeader,
      onRemove: widget.onRemoveHeader,
      lockedLabel: widget.lockedLabel,
      addLabel: widget.addHeaderLabel,
      removeLabel: widget.removeHeaderLabel,
      nameLabel: widget.nameLabel,
      valueLabel: widget.valueLabel,
    ),
    DraftTab.query => _QueryTable(
      pathAndQuery: widget.draft.pathAndQuery,
      onChanged: widget.onPathAndQuery,
      nameLabel: widget.nameLabel,
      valueLabel: widget.valueLabel,
    ),
  };

  Widget _bodyPane(HTokens tokens) {
    if (!widget.draft.bodyIsEditable) {
      return Center(
        child: Padding(
          padding: EdgeInsets.all(tokens.spacing.x6),
          child: Text(
            widget.notEditableLabel,
            key: const Key('editor-body-not-editable'),
            textAlign: TextAlign.center,
            style: tokens.typography.ui12.tinted(tokens.colors.fg1),
          ),
        ),
      );
    }
    // Ein nacktes `EditableText` kann tippen und sonst fast nichts: kein
    // Ziehen mit der Maus, kein Doppelklick auf ein Wort, kein Kontextmenü.
    // Flutter nennt es dafür selbst „inadequate for user-facing
    // applications", und `Ctrl+R` braucht eine Auswahl. Der
    // `TextSelectionGestureDetectorBuilder` ist der Weg, den Flutter dafür
    // vorsieht; er bedient das Feld über [_bodyKey], deshalb steht
    // `rendererIgnoresPointer` auf wahr.
    return Padding(
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: _gestures.buildGestureDetector(
        behavior: HitTestBehavior.translucent,
        // Der `GlobalKey` gehört dem Gestenbauer; der schlichte Schlüssel
        // daneben ist der, unter dem Tests und Goldens das Feld finden.
        child: KeyedSubtree(
          key: const Key('editor-draft-body'),
          child: EditableText(
            key: _bodyKey,
            controller: _body,
            focusNode: _bodyFocus,
            style: tokens.typography.mono12.tinted(tokens.colors.fg0),
            cursorColor: tokens.colors.accent,
            backgroundCursorColor: tokens.colors.fg2,
            selectionColor: tokens.colors.accent.withValues(alpha: 0.25),
            selectionControls: materialTextSelectionControls,
            maxLines: null,
            expands: true,
            readOnly: !widget.enabled,
            rendererIgnoresPointer: true,
            onChanged: widget.onBody,
            contextMenuBuilder:
                (BuildContext context, EditableTextState state) =>
                    AdaptiveTextSelectionToolbar.editableText(
                      editableTextState: state,
                    ),
          ),
        ),
      ),
    );
  }
}

/// Verbindet die Zeigergesten mit dem Rumpf-Feld.
///
/// Der Bauplan, den Flutter für ein eigenes Textfeld vorsieht: Er braucht den
/// Schlüssel des `EditableText` und zwei Antworten — Auswahl ja, Kraftdruck
/// nein (das ist eine iOS-Geste und dieses Programm läuft auf Linux).
class _BodyGestures extends TextSelectionGestureDetectorBuilderDelegate {
  _BodyGestures(this._key);

  final GlobalKey<EditableTextState> _key;

  @override
  GlobalKey<EditableTextState> get editableTextKey => _key;

  @override
  bool get forcePressEnabled => false;

  @override
  bool get selectionEnabled => true;
}

/// Die Query als Tabelle, mit `pathAndQuery` als einziger Ablage.
class _QueryTable extends StatelessWidget {
  const _QueryTable({
    required this.pathAndQuery,
    required this.onChanged,
    required this.nameLabel,
    required this.valueLabel,
  });

  final String pathAndQuery;
  final ValueChanged<String> onChanged;
  final String nameLabel;
  final String valueLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final List<({String name, String value})> pairs = queryPairs(pathAndQuery);
    return ListView.builder(
      key: const Key('editor-query'),
      padding: EdgeInsets.all(tokens.spacing.x3),
      itemCount: pairs.length + 1,
      itemBuilder: (BuildContext context, int i) {
        if (i == 0) {
          return Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  nameLabel,
                  style: tokens.typography.ui11.tinted(tokens.colors.fg2),
                ),
              ),
              Expanded(
                flex: 2,
                child: Text(
                  valueLabel,
                  style: tokens.typography.ui11.tinted(tokens.colors.fg2),
                ),
              ),
            ],
          );
        }
        final ({String name, String value}) pair = pairs[i - 1];
        return Padding(
          padding: EdgeInsets.only(top: tokens.spacing.x2),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Expanded(
                child: Text(
                  pair.name,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                ),
              ),
              Expanded(
                flex: 2,
                child: Text(
                  pair.value,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg0),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

/// Die Query als Paare, prozent-dekodiert für die Anzeige.
///
/// Ein Wert ohne `=` steht als Name mit leerem Wert da; ein leerer Name wird
/// verworfen. Der Rückweg (`queryOf`) kodiert wieder, damit nichts, was ein
/// Mensch eingibt, die Query zerteilen kann.
List<({String name, String value})> queryPairs(String pathAndQuery) {
  final int mark = pathAndQuery.indexOf('?');
  if (mark < 0) {
    return const <({String name, String value})>[];
  }
  final String query = pathAndQuery.substring(mark + 1);
  return <({String name, String value})>[
    for (final String part in query.split('&'))
      if (part.isNotEmpty)
        (
          name: Uri.decodeQueryComponent(part.split('=').first),
          value: part.contains('=')
              ? Uri.decodeQueryComponent(part.substring(part.indexOf('=') + 1))
              : '',
        ),
  ];
}

/// Aus Paaren wieder ein `pathAndQuery`, alle Werte URL-kodiert.
String pathAndQueryOf(String path, List<({String name, String value})> pairs) {
  if (pairs.isEmpty) {
    return path;
  }
  final String query = <String>[
    for (final ({String name, String value}) pair in pairs)
      '${Uri.encodeQueryComponent(pair.name)}=${Uri.encodeQueryComponent(pair.value)}',
  ].join('&');
  return '$path?$query';
}
