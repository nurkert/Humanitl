/// Die Kopfzeilen des Entwurfs, als Tabelle.
///
/// Eine gesperrte Zeile ist grau, trägt ein Schloss und ein Wort dazu. Das
/// Wort ist der Punkt: Ein graues Feld ohne Begründung ist ein totes Control,
/// und ein totes Control ohne Grund ist schlechter als gar keines
/// (`backlog/CONVENTIONS.md` 4.13). `host` und `content-length` stehen deshalb
/// sichtbar in der Liste, statt weggelassen zu werden — der Mensch soll sehen,
/// was mitgeht, auch wenn er es nicht ändern darf.
library;

import 'package:flutter/widgets.dart';

import '../../../core/ui/ui.dart';
import '../model/draft.dart';

/// Die Kopfzeilen-Tabelle.
class HeaderTable extends StatelessWidget {
  /// Baut die Tabelle.
  const HeaderTable({
    required this.headers,
    required this.onChanged,
    required this.onAdd,
    required this.onRemove,
    required this.lockedLabel,
    required this.addLabel,
    required this.removeLabel,
    required this.nameLabel,
    required this.valueLabel,
    super.key,
  });

  /// Die Einträge des Entwurfs, in ihrer Reihenfolge.
  final List<HeaderEntry> headers;

  /// Wird mit Index, Name und Wert gerufen.
  final void Function(int index, String name, String value) onChanged;

  /// Hängt eine leere Zeile an.
  final VoidCallback onAdd;

  /// Entfernt die Zeile an dieser Stelle.
  final ValueChanged<int> onRemove;

  /// Das Wort für eine gesperrte Zeile.
  final String lockedLabel;

  /// Die Beschriftung des Hinzufügen-Knopfes.
  final String addLabel;

  /// Die Beschriftung des Entfernen-Knopfes.
  final String removeLabel;

  /// Die Spaltenüberschrift für den Namen.
  final String nameLabel;

  /// Die Spaltenüberschrift für den Wert.
  final String valueLabel;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return ListView.builder(
      key: const Key('editor-headers'),
      padding: EdgeInsets.all(tokens.spacing.x3),
      itemCount: headers.length + 2,
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
        if (i == headers.length + 1) {
          return Padding(
            padding: EdgeInsets.only(top: tokens.spacing.x3),
            child: Align(
              alignment: Alignment.centerLeft,
              child: HButton(
                key: const Key('editor-header-add'),
                variant: HButtonVariant.ghost,
                onPressed: onAdd,
                child: Text(addLabel),
              ),
            ),
          );
        }
        return _HeaderRow(
          // Der Schluessel haengt an der Stelle, nicht am Namen: Ein Name, den
          // gerade jemand tippt, wechselt mit jedem Zeichen, und ein Widget,
          // dessen Schluessel wechselt, verliert seinen `State` samt Cursor.
          key: ValueKey<int>(i - 1),
          index: i - 1,
          entry: headers[i - 1],
          onChanged: onChanged,
          onRemove: onRemove,
          lockedLabel: lockedLabel,
          removeLabel: removeLabel,
          nameLabel: nameLabel,
          valueLabel: valueLabel,
        );
      },
    );
  }
}

/// Eine Zeile: gesperrt als Text, sonst als zwei Eingabefelder.
///
/// Die freien Zeilen sind Felder und keine Beschriftungen. Ein Editor, der
/// eine Kopfzeile nur anzeigt und einen Knopf „Hinzufügen" daneben stellt, der
/// eine Zeile anlegt, in die niemand tippen kann, verspricht eine Bearbeitung
/// und liefert eine Ansicht (`backlog/CONVENTIONS.md` 4.13).
class _HeaderRow extends StatefulWidget {
  const _HeaderRow({
    required this.index,
    required this.entry,
    required this.onChanged,
    required this.onRemove,
    required this.lockedLabel,
    required this.removeLabel,
    required this.nameLabel,
    required this.valueLabel,
    super.key,
  });

  final int index;
  final HeaderEntry entry;
  final void Function(int index, String name, String value) onChanged;
  final ValueChanged<int> onRemove;
  final String lockedLabel;
  final String removeLabel;
  final String nameLabel;
  final String valueLabel;

  @override
  State<_HeaderRow> createState() => _HeaderRowState();
}

class _HeaderRowState extends State<_HeaderRow> {
  late final TextEditingController _name = TextEditingController(
    text: widget.entry.name,
  );
  late final TextEditingController _value = TextEditingController(
    text: widget.entry.value,
  );

  @override
  void didUpdateWidget(_HeaderRow oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Nur zuweisen, was sich wirklich unterscheidet: Eine Zuweisung setzt den
    // Cursor ans Ende, und wer beim Tippen jedes Mal ans Ende springt, kann
    // nicht tippen (`docs/UX.md` 2.8). Der Fall tritt ein, wenn eine
    // Ersetzung den Wert ändert, nicht beim Tippen selbst.
    if (_name.text != widget.entry.name) {
      _name.text = widget.entry.name;
    }
    if (_value.text != widget.entry.value) {
      _value.text = widget.entry.value;
    }
  }

  @override
  void dispose() {
    _name.dispose();
    _value.dispose();
    super.dispose();
  }

  void _report() => widget.onChanged(widget.index, _name.text, _value.text);

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final HeaderEntry entry = widget.entry;
    return Padding(
      padding: EdgeInsets.only(top: tokens.spacing.x2),
      child: entry.locked ? _locked(tokens, entry) : _editable(tokens),
    );
  }

  /// Eine gesperrte Zeile: Schloss, Name und Wert, alles nur zu lesen.
  ///
  /// Sie steht sichtbar in der Liste, statt weggelassen zu werden: Der Mensch
  /// soll sehen, was mitgeht, auch wenn er es nicht ändern darf. Die Sperre
  /// steht in der Semantik und nicht nur in der Farbe -- wer den Bildschirm
  /// hört statt ihn zu sehen, bekommt sonst ein Feld, das sich nicht ändern
  /// lässt, ohne dass jemand sagt warum (`docs/UX.md` 6).
  Widget _locked(HTokens tokens, HeaderEntry entry) => Semantics(
    label: '${entry.name}, ${widget.lockedLabel}',
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Expanded(
          child: Row(
            children: <Widget>[
              HGlyphIcon(
                HGlyph.lock,
                key: Key('editor-header-locked-${entry.name.toLowerCase()}'),
                size: lockedGlyphSize,
                color: tokens.colors.fg2,
                semanticsLabel: widget.lockedLabel,
              ),
              SizedBox(width: tokens.spacing.x1),
              Flexible(
                child: Text(
                  entry.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg2),
                ),
              ),
            ],
          ),
        ),
        Expanded(
          flex: 2,
          child: Text(
            entry.value,
            key: Key('editor-header-value-${widget.index}'),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: tokens.typography.mono12.tinted(tokens.colors.fg2),
          ),
        ),
        SizedBox(width: tokens.sizes.hitMin),
      ],
    ),
  );

  Widget _editable(HTokens tokens) => Row(
    crossAxisAlignment: CrossAxisAlignment.center,
    children: <Widget>[
      Expanded(
        child: HTextField(
          key: Key('editor-header-name-${widget.index}'),
          controller: _name,
          semanticsLabel: widget.nameLabel,
          onChanged: (String _) => _report(),
        ),
      ),
      SizedBox(width: tokens.spacing.x2),
      Expanded(
        flex: 2,
        child: HTextField(
          key: Key('editor-header-value-${widget.index}'),
          controller: _value,
          semanticsLabel: widget.valueLabel,
          onChanged: (String _) => _report(),
        ),
      ),
      SizedBox(
        width: tokens.sizes.hitMin,
        child: HIconButton(
          key: Key('editor-header-remove-${widget.index}'),
          glyph: HGlyph.trash,
          semanticsLabel: widget.removeLabel,
          onPressed: () => widget.onRemove(widget.index),
        ),
      ),
    ],
  );
}

/// Die Kantenlänge des Schlosses vor einer gesperrten Kopfzeile.
///
/// Dieselbe Größe wie die Zeilenhöhe von `ui12`, damit es auf der Grundlinie
/// des Namens sitzt und nicht darüber schwebt.
const double lockedGlyphSize = 12;
