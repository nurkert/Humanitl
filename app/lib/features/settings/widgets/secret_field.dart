/// The input of the access key: masked, never filled, never read back
/// (HUM-140).
///
/// `HTextField` has no masked mode, and `packages/ui` is not touched in this
/// issue (HUM-173 works there). This field stands on Flutter's own
/// `EditableText` with `obscureText` and wears the same surface: `bg2`, the
/// hairline, the control radius, the accent cursor. It has no selection
/// toolbar and therefore no "copy": a key that was typed once can be pasted
/// in, but nothing on the screen hands it out again.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../../core/ui/pasted_text.dart';
import '../../../core/ui/ui.dart';

/// A one-line masked input.
class SecretField extends StatefulWidget {
  /// Creates the field over [controller].
  const SecretField({
    required this.controller,
    required this.semanticsLabel,
    required this.hint,
    this.onSubmitted,
    this.enabled = true,
    this.inputKey = const Key('settings-key-input'),
    super.key,
  });

  /// The key of the input itself; a second field on the same sheet (the
  /// form of a new server, HUM-289) needs its own.
  final Key inputKey;

  /// The text being typed. The owner clears it after every attempt.
  final TextEditingController controller;

  /// Screen-reader label, already localised.
  final String semanticsLabel;

  /// Placeholder while the field is empty, already localised.
  final String hint;

  /// Called when the input is finished with `Enter`.
  final ValueChanged<String>? onSubmitted;

  /// False while nothing may be typed.
  final bool enabled;

  @override
  State<SecretField> createState() => _SecretFieldState();
}

class _SecretFieldState extends State<SecretField> {
  final FocusNode _focus = FocusNode(debugLabel: 'settings-secret');

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_changed);
  }

  @override
  void didUpdateWidget(SecretField old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller.removeListener(_changed);
      widget.controller.addListener(_changed);
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    _focus.dispose();
    super.dispose();
  }

  /// The placeholder depends on whether anything was typed.
  void _changed() => setState(() {});

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final TextStyle style = tokens.typography.mono13.tinted(
      widget.enabled ? tokens.colors.fg0 : tokens.colors.fg1,
    );
    return Semantics(
      textField: true,
      obscured: true,
      label: widget.semanticsLabel,
      enabled: widget.enabled,
      child: GestureDetector(
        onTap: _focus.requestFocus,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: widget.enabled ? tokens.colors.bg2 : tokens.colors.bg1,
            borderRadius: HRadius.controlRadius,
            border: Border.all(color: tokens.colors.line),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: HSpace.x2,
              vertical: HSpace.x1,
            ),
            child: Stack(
              children: <Widget>[
                if (widget.controller.text.isEmpty)
                  ExcludeSemantics(
                    child: Text(
                      widget.hint,
                      style: style.tinted(tokens.colors.fg2),
                    ),
                  ),
                EditableText(
                  key: widget.inputKey,
                  controller: widget.controller,
                  focusNode: _focus,
                  readOnly: !widget.enabled,
                  obscureText: true,
                  autocorrect: false,
                  enableSuggestions: false,
                  enableIMEPersonalizedLearning: false,
                  keyboardType: TextInputType.visiblePassword,
                  autofillHints: const <String>[],
                  maxLines: 1,
                  style: style,
                  cursorColor: tokens.colors.accent,
                  backgroundCursorColor: tokens.colors.fg2,
                  onSubmitted: widget.onSubmitted,
                  inputFormatters: <TextInputFormatter>[
                    // Ein Schluessel traegt keinen Leerraum (`ApiKey::new`);
                    // ein Zeilenumbruch aus der Zwischenablage faellt hier weg
                    // statt im Daemon als `CONFIG_020` zurueckzukommen.
                    FilteringTextInputFormatter.deny(RegExp(r'[\r\n]')),
                    // Ein eingefügter Schlüssel verliert Anführungszeichen
                    // und Leerraum außen (HUM-283), wie im Daemon.
                    const PasteDebrisFormatter(),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
