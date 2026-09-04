import 'package:flutter/widgets.dart';
import 'package:shadcn_flutter/shadcn_flutter.dart' as shad;

import '../theme/h_theme.dart';
import '../theme/shadcn_theme.dart';
import '../tokens/spacing.dart';
import '../tokens/tokens.dart';
import 'h_control.dart';
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
/// Jedes Segment ist ein eigener Fokusstopp mit dem Ring von `HFocusRing` und
/// mindestens [HSize.hitMin] hoch (5.1, 6).
///
/// Der Rahmen um die Reihe ist ein `OutlinedContainer` der Bibliothek: er
/// bringt Fläche, Rahmen, Ecke und das Beschneiden mit, damit die Füllung
/// eines Segments an der Ecke nicht über den Rahmen läuft.
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
    return HTheme.host(
      context,
      _HSegmentFrame(
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
    return HTheme.host(
      context,
      Wrap(
        spacing: tokens.spacing.x1,
        runSpacing: tokens.spacing.x1,
        children: <Widget>[
          for (final HSegmentOption<T> option in options)
            _HSegmentFrame(
              child: HSegment<T>(
                option: option,
                selected: selected.contains(option.value),
                enabled: enabled,
                onSelect: onToggle,
              ),
            ),
        ],
      ),
    );
  }
}

/// Der Rahmen um eine Reihe von Segmenten oder um einen einzelnen Chip.
class _HSegmentFrame extends StatelessWidget {
  const _HSegmentFrame({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return shad.OutlinedContainer(
      backgroundColor: const Color(0x00000000),
      borderColor: tokens.colors.line,
      borderWidth: HSize.hairline,
      borderRadius: HRadius.controlRadius,
      child: child,
    );
  }
}

/// Ein einzelnes Segment. Öffentlich, weil [HSegmented] und [HChoiceChips]
/// dasselbe Verhalten teilen und eine Kopie davon sofort auseinanderliefe.
class HSegment<T> extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Widget? leading = option.leading;
    return Semantics(
      button: true,
      selected: selected,
      enabled: enabled,
      label: option.semanticsLabel ?? option.label,
      excludeSemantics: true,
      child: HControl(
        enabled: enabled,
        onPressed: enabled ? () => onSelect(option.value) : null,
        radius: tokens.radii.control,
        leading: leading,
        leadingGap: tokens.spacing.x1,
        fill: (HTokens tokens, Set<WidgetState> states) =>
            HShadcnButtonStyle.segmentFill(tokens, states, selected: selected),
        style: (HTokens tokens, Color fill) =>
            HShadcnButtonStyle.segment(tokens, selected: selected, fill: fill),
        builder: (BuildContext context, Set<WidgetState> states, Color fill) =>
            ConstrainedBox(
              constraints: BoxConstraints(minHeight: tokens.sizes.hitMin),
              child: Center(
                widthFactor: 1,
                heightFactor: 1,
                child: Text(option.label),
              ),
            ),
      ),
    );
  }
}
