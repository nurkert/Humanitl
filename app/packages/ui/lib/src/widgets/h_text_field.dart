import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_focus_ring.dart';

/// Ein einzeiliges Eingabefeld.
///
/// Es gab bisher keines, und der Regel-Editor hat sich deshalb eines gebaut.
/// Dieses hier ist dasselbe aus denselben Token: `bg2` als Fläche, die
/// Haarlinie als Rahmen, der Akzent als Cursor, der Fokusring außerhalb des
/// Rahmens ([HFocusRing]) und eine Mindesthöhe statt einer Höhe, damit bei
/// `TextScaler.linear(2.0)` nichts abgeschnitten wird
/// (`docs/UX.md` 5.1, 6 und 9, Punkte 14 und 17).
///
/// Der Platzhalter ist `fg2` und verschwindet mit dem ersten Zeichen; er ist
/// nie die einzige Beschriftung: [semanticsLabel] sagt, was das Feld bedeutet,
/// weil die sichtbare Beschriftung über dem Kasten steht und die Semantik sie
/// nicht mit ihm verbindet.
class HTextField extends StatefulWidget {
  /// Creates an input.
  const HTextField({
    required this.controller,
    required this.semanticsLabel,
    this.hint = '',
    this.mono = true,
    this.enabled = true,
    this.digitsOnly = false,
    this.onChanged,
    this.onSubmitted,
    this.focusNode,
    this.autofocus = false,
    super.key,
  });

  /// Der Text, der bearbeitet wird.
  final TextEditingController controller;

  /// Screen-reader label.
  final String semanticsLabel;

  /// Platzhalter, solange das Feld leer ist.
  final String hint;

  /// Monospace, für alles, was mit etwas anderem verglichen wird.
  final bool mono;

  /// Falsch für etwas, das niemand ändern darf.
  final bool enabled;

  /// Nimmt nur Ziffern an, etwa für einen Port.
  final bool digitsOnly;

  /// Wird bei jeder Änderung gerufen.
  final ValueChanged<String>? onChanged;

  /// Wird gerufen, wenn die Eingabe mit `Enter` abgeschlossen wird.
  final ValueChanged<String>? onSubmitted;

  /// Ein von außen gehaltener Fokusknoten.
  final FocusNode? focusNode;

  /// Nimmt den Fokus, sobald das Feld zum ersten Mal gebaut wird.
  final bool autofocus;

  @override
  State<HTextField> createState() => _HTextFieldState();
}

class _HTextFieldState extends State<HTextField> {
  FocusNode? _owned;
  bool _focused = false;

  FocusNode get _focus => widget.focusNode ?? (_owned ??= FocusNode());

  @override
  void initState() {
    super.initState();
    _focus.addListener(_syncFocus);
    widget.controller.addListener(_redrawHint);
  }

  @override
  void didUpdateWidget(HTextField oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller != widget.controller) {
      oldWidget.controller.removeListener(_redrawHint);
      widget.controller.addListener(_redrawHint);
    }
    if (oldWidget.focusNode != widget.focusNode) {
      oldWidget.focusNode?.removeListener(_syncFocus);
      _focus.addListener(_syncFocus);
    }
  }

  void _syncFocus() {
    if (_focused != _focus.hasFocus) {
      setState(() => _focused = _focus.hasFocus);
    }
  }

  void _redrawHint() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_redrawHint);
    _focus.removeListener(_syncFocus);
    _owned?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle style =
        (widget.mono ? tokens.typography.mono13 : tokens.typography.ui13)
            .tinted(widget.enabled ? tokens.colors.fg0 : tokens.colors.fg1);
    return Semantics(
      textField: true,
      label: widget.semanticsLabel,
      enabled: widget.enabled,
      child: HFocusRing(
        visible: _focused,
        radius: tokens.radii.control,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: widget.enabled ? tokens.colors.bg2 : tokens.colors.bg1,
            borderRadius: HRadius.controlRadius,
            border: Border.all(color: tokens.colors.line),
          ),
          child: Padding(
            padding: EdgeInsets.symmetric(
              horizontal: tokens.spacing.x2,
              vertical: tokens.spacing.x1,
            ),
            child: ConstrainedBox(
              // Eine Untergrenze, nie eine Höhe: bei doppelter Skalierung ist
              // die Zeile höher, als der Kasten wäre (`docs/UX.md` 6).
              constraints: BoxConstraints(
                minHeight: tokens.sizes.hitMin - tokens.spacing.x2,
              ),
              child: Stack(
                alignment: Alignment.centerLeft,
                children: <Widget>[
                  if (widget.controller.text.isEmpty && widget.hint.isNotEmpty)
                    ExcludeSemantics(
                      child: Text(
                        widget.hint,
                        style: style.tinted(tokens.colors.fg2),
                      ),
                    ),
                  EditableText(
                    controller: widget.controller,
                    focusNode: _focus,
                    autofocus: widget.autofocus,
                    readOnly: !widget.enabled,
                    style: style,
                    cursorColor: tokens.colors.accent,
                    backgroundCursorColor: tokens.colors.bg3,
                    selectionColor: HColorDerivation.fade(
                      tokens.colors.accent,
                      0.35,
                    ),
                    inputFormatters: widget.digitsOnly
                        ? <TextInputFormatter>[
                            FilteringTextInputFormatter.digitsOnly,
                          ]
                        : null,
                    onChanged: widget.onChanged,
                    onSubmitted: widget.onSubmitted,
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
