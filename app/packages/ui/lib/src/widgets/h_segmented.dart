import 'package:flutter/widgets.dart';

import '../theme/h_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import '../tokens/typography.dart';
import 'h_animated_fill.dart';
import 'h_focus_ring.dart';
import 'h_hairline.dart';

/// Eine Wahlmöglichkeit in [HSegmented] oder [HChoiceChips].
@immutable
class HSegmentOption<T> {
  /// Creates an option.
  const HSegmentOption({
    required this.value,
    required this.label,
    this.leading,
    this.semanticsLabel,
  });

  /// Was die Wahl bedeutet.
  final T value;

  /// Die sichtbare Beschriftung, vom Aufrufer bereits übersetzt.
  final String label;

  /// Ein Glyph vor der Beschriftung, für eine Wahl, die eine Farbe trägt.
  final Widget? leading;

  /// Was ein Screenreader statt [label] sagt, wenn die Beschriftung ein
  /// Zeichen ist.
  final String? semanticsLabel;
}

/// Eine Reihe sich ausschließender Wahlmöglichkeiten.
///
/// Die gewählte trägt die höchste Fläche und den Primärtext, nie die
/// Akzentfüllung: der Akzent gehört der einen Handlung des Screens, und ein
/// Formular mit vier gefüllten Segmenten hätte fünf (`docs/UX.md` 3.1).
/// Jedes Segment ist ein eigener Fokusstopp mit dem Ring von [HFocusRing] und
/// mindestens [HSize.hitMin] hoch (5.1, 6).
class HSegmented<T> extends StatelessWidget {
  /// Creates a segmented control.
  const HSegmented({
    required this.options,
    required this.selected,
    required this.onSelect,
    this.enabled = true,
    super.key,
  });

  /// Die Wahlmöglichkeiten in der Reihenfolge, in der sie stehen.
  final List<HSegmentOption<T>> options;

  /// Die gewählte.
  final T selected;

  /// Wird mit der Wahl gerufen.
  final ValueChanged<T> onSelect;

  /// Falsch für etwas, das niemand ändern darf.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: HRadius.controlRadius,
        border: Border.all(color: tokens.colors.line),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          for (int i = 0; i < options.length; i++) ...<Widget>[
            if (i > 0) const HHairline(vertical: true, length: HSize.hitMin),
            HSegment<T>(
              option: options[i],
              selected: options[i].value == selected,
              enabled: enabled,
              onSelect: onSelect,
            ),
          ],
        ],
      ),
    );
  }
}

/// Eine Gruppe von Wahlmöglichkeiten, von denen beliebig viele an sein dürfen.
class HChoiceChips<T> extends StatelessWidget {
  /// Creates a chip group.
  const HChoiceChips({
    required this.options,
    required this.selected,
    required this.onToggle,
    this.enabled = true,
    super.key,
  });

  /// Die Wahlmöglichkeiten.
  final List<HSegmentOption<T>> options;

  /// Welche davon an sind.
  final Set<T> selected;

  /// Wird mit der angetippten Wahl gerufen.
  final ValueChanged<T> onToggle;

  /// Falsch für etwas, das niemand ändern darf.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Wrap(
      spacing: tokens.spacing.x1,
      runSpacing: tokens.spacing.x1,
      children: <Widget>[
        for (final HSegmentOption<T> option in options)
          DecoratedBox(
            decoration: BoxDecoration(
              borderRadius: HRadius.controlRadius,
              border: Border.all(color: tokens.colors.line),
            ),
            child: HSegment<T>(
              option: option,
              selected: selected.contains(option.value),
              enabled: enabled,
              onSelect: onToggle,
            ),
          ),
      ],
    );
  }
}

/// Ein einzelnes Segment. Öffentlich, weil [HSegmented] und [HChoiceChips]
/// dasselbe Verhalten teilen und eine Kopie davon sofort auseinanderliefe.
class HSegment<T> extends StatefulWidget {
  /// Creates one segment.
  const HSegment({
    required this.option,
    required this.selected,
    required this.enabled,
    required this.onSelect,
    super.key,
  });

  /// Die Wahl, für die dieses Segment steht.
  final HSegmentOption<T> option;

  /// Ob sie gewählt ist.
  final bool selected;

  /// Ob das Segment auf Eingaben reagiert.
  final bool enabled;

  /// Wird mit dem Wert der Wahl gerufen.
  final ValueChanged<T> onSelect;

  @override
  State<HSegment<T>> createState() => _HSegmentState<T>();
}

class _HSegmentState<T> extends State<HSegment<T>> {
  bool _focused = false;
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final HSegmentOption<T> option = widget.option;
    final Color fill = widget.selected
        ? tokens.colors.bg3
        : _hovered
        ? tokens.colors.bg2
        : const Color(0x00000000);
    final Widget? leading = option.leading;
    return Semantics(
      button: true,
      selected: widget.selected,
      enabled: widget.enabled,
      label: option.semanticsLabel ?? option.label,
      excludeSemantics: true,
      child: FocusableActionDetector(
        enabled: widget.enabled,
        mouseCursor: widget.enabled
            ? SystemMouseCursors.click
            : MouseCursor.defer,
        onShowHoverHighlight: (bool value) => setState(() => _hovered = value),
        onFocusChange: (bool value) => setState(() => _focused = value),
        actions: <Type, Action<Intent>>{
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (ActivateIntent intent) {
              widget.onSelect(option.value);
              return null;
            },
          ),
        },
        child: HFocusRing(
          visible: _focused && widget.enabled,
          radius: tokens.radii.control,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: widget.enabled ? () => widget.onSelect(option.value) : null,
            child: HAnimatedFill(
              color: fill,
              builder: (BuildContext context, Color animated) => Container(
                constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
                padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x2),
                color: animated,
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    if (leading != null) ...<Widget>[
                      leading,
                      SizedBox(width: tokens.spacing.x1),
                    ],
                    Text(
                      option.label,
                      // Deaktiviert heißt sichtbar deaktiviert: `fg2` ist die
                      // Stufe, die `docs/UX.md` 6 dafür freihält.
                      style: tokens.typography.ui12.medium.tinted(
                        !widget.enabled
                            ? tokens.colors.fg2
                            : widget.selected
                            ? tokens.colors.fg0
                            : tokens.colors.fg1,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
