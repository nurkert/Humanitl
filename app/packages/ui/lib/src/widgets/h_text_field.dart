import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../tokens/colors.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';

/// Ein einzeiliges Eingabefeld.
///
/// Steht auf `TextField` aus `shadcn_flutter`: Auswahl, Zeiger, Kontextmenü,
/// Autofill und die Platzhalter-Behandlung kommen von dort. Die Fläche ist
/// `bg2`, der Rahmen die Haarlinie, der Cursor der Akzent — alles aus
/// [HTokens], über die Dekoration und über das `TextFieldTheme`, das `HTheme`
/// veröffentlicht.
///
/// Dies ist das eine Control, dessen Fokusring **die Bibliothek** zeichnet:
/// `TextField` bringt ihren `FocusOutline` fest eingebaut mit, und zwei Ringe
/// übereinander sind einer zu viel. Er trägt trotzdem unsere Maße — zwei Pixel
/// Akzent, zwei Pixel Abstand —, weil `HTheme` sein `FocusOutlineTheme` aus
/// [HFocusRingMetrics] füllt. Der Unterschied zu [HFocusRing] ist, dass dieser
/// Ring keinen Platz reserviert, sondern über die eigene Kante hinaus
/// zeichnet.
///
/// Der Platzhalter ist `fg2` und verschwindet mit dem ersten Zeichen; er ist
/// nie die einzige Beschriftung: [semanticsLabel] sagt, was das Feld bedeutet,
/// weil die sichtbare Beschriftung über dem Kasten steht und die Semantik sie
/// nicht mit ihm verbindet.
///
/// **[onChanged] meldet nur, was ein Mensch getippt hat.** Das ist der
/// Vertrag, den Flutter für `onChanged` aufschreibt, und der, unter dem die
/// Bildschirme dieses Programms geschrieben sind. Die Bibliothek hält ihn
/// nicht: ihr `TextField` hängt an dem `TextEditingController` und meldet
/// **jede** Änderung, auch eine, die ein Widget selbst zuweist. Der
/// Regel-Editor füllt seine Felder beim Öffnen genau so, und die Meldung
/// liefe dort mitten im Aufbau in den Provider zurück. Deshalb steht zwischen
/// beiden ein Filter: ein `TextInputFormatter` sieht ausschließlich Eingaben
/// eines Menschen, merkt sich deren Ergebnis, und nur dieses Ergebnis wird
/// weitergereicht.
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
  final _HUserEditProbe _probe = _HUserEditProbe();

  /// Reicht [value] weiter, sofern ein Mensch ihn getippt hat.
  void _forward(String value) {
    if (!_probe.wasTyped(value)) {
      return;
    }
    widget.onChanged?.call(value);
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final bool mono = widget.mono;
    final bool enabled = widget.enabled;
    final String hint = widget.hint;
    final TextStyle style =
        (mono ? tokens.typography.mono13 : tokens.typography.ui13).tinted(
          enabled ? tokens.colors.fg0 : tokens.colors.fg1,
        );
    return HTheme.host(
      context,
      Semantics(
        textField: true,
        label: widget.semanticsLabel,
        enabled: enabled,
        child: DefaultSelectionStyle(
          cursorColor: tokens.colors.accent,
          selectionColor: HColorDerivation.fade(tokens.colors.accent, 0.35),
          child: shad.TextField(
            controller: widget.controller,
            focusNode: widget.focusNode,
            autofocus: widget.autofocus,
            enabled: enabled,
            readOnly: !enabled,
            style: style,
            cursorColor: tokens.colors.accent,
            maxLines: 1,
            placeholder: hint.isEmpty
                ? null
                : Text(hint, style: style.tinted(tokens.colors.fg2)),
            // Die Dekoration steht hier und nicht im `TextFieldTheme`, weil
            // ein abgeschaltetes Feld eine andere Fläche trägt: `bg1` statt
            // `bg2`. Deaktiviert heißt sichtbar deaktiviert (`docs/UX.md` 6).
            decoration: BoxDecoration(
              color: enabled ? tokens.colors.bg2 : tokens.colors.bg1,
              borderRadius: HRadius.controlRadius,
              border: Border.all(color: tokens.colors.line),
            ),
            padding: const EdgeInsets.symmetric(
              horizontal: HSpace.x2,
              vertical: HSpace.x1,
            ),
            // Der Filter steht **hinter** allen anderen, damit er den Text
            // sieht, der am Ende im Feld steht.
            inputFormatters: <TextInputFormatter>[
              if (widget.digitsOnly) FilteringTextInputFormatter.digitsOnly,
              _probe,
            ],
            onChanged: _forward,
            onSubmitted: widget.onSubmitted,
          ),
        ),
      ),
    );
  }
}

/// Merkt sich, was zuletzt ein Mensch getippt hat.
///
/// Ein `TextInputFormatter` läuft ausschließlich auf Eingaben, die über die
/// Eingabeverbindung kommen — Tippen, Einfügen, Autofill. Eine Zuweisung an
/// den `TextEditingController` läuft nicht durch ihn. Das ist der einzige
/// Unterschied, an dem sich beides auseinanderhalten lässt, und deshalb steht
/// hier ein Formatter, der nichts formatiert.
class _HUserEditProbe extends TextInputFormatter {
  String? _typed;

  /// Ob [value] das Ergebnis der letzten Eingabe eines Menschen ist.
  ///
  /// Verbraucht die Marke: eine Eingabe wird genau einmal gemeldet.
  bool wasTyped(String value) {
    if (_typed != value) {
      return false;
    }
    _typed = null;
    return true;
  }

  @override
  TextEditingValue formatEditUpdate(
    TextEditingValue oldValue,
    TextEditingValue newValue,
  ) {
    _typed = newValue.text;
    return newValue;
  }
}
