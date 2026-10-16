// Golden der Zustandsfarben (HUM-005, HUM-054): jeder der acht sichtbaren
// Zustände eines Flusses mit Glyph, Beschriftung, der Farbe selbst, ihrer
// Tönung und ihrer Textfassung. In den vier Varianten aus
// `support/golden_variants.dart`.
//
// Das Bild ist die Prüfung der Tokens: Eine Farbe, die sich in
// `packages/ui/lib/src/tokens/colors.dart` ändert, ändert hier mindestens
// einen Streifen, auch wenn kein Bildschirm sie gerade zeigt.
//
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'support/golden_variants.dart';

/// Eine Zeile je Zustand.
class TokensPalette extends StatelessWidget {
  /// Die Palette der aktuellen Tokens.
  const TokensPalette({super.key});

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final HFlowState state in HFlowState.values)
          _StateRow(
            state: state,
            label: l10n.flowStateLabel(state),
            color: tokens.stateColor(state),
            tokens: tokens,
          ),
      ],
    );
  }
}

class _StateRow extends StatelessWidget {
  const _StateRow({
    required this.state,
    required this.label,
    required this.color,
    required this.tokens,
  });

  final HFlowState state;
  final String label;
  final Color color;
  final HTokens tokens;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 36,
      child: Row(
        children: <Widget>[
          SizedBox(
            width: HSize.stateRail,
            height: 28,
            child: ColoredBox(color: color),
          ),
          SizedBox(width: tokens.spacing.x3),
          HStateGlyph(state: state, semanticsLabel: label),
          SizedBox(width: tokens.spacing.x2),
          SizedBox(
            width: 220,
            child: Text(
              label,
              maxLines: 1,
              style: tokens.typography.ui13.tinted(tokens.stateTextOf(color)),
            ),
          ),
          SizedBox(width: tokens.spacing.x3),
          _Swatch(color: color),
          SizedBox(width: tokens.spacing.x2),
          _Swatch(color: tokens.tint(color)),
          SizedBox(width: tokens.spacing.x2),
          _Swatch(color: tokens.stateTextOf(color)),
        ],
      ),
    );
  }
}

class _Swatch extends StatelessWidget {
  const _Swatch({required this.color});

  final Color color;

  @override
  Widget build(BuildContext context) =>
      SizedBox(width: 64, height: 20, child: ColoredBox(color: color));
}

void main() {
  goldenVariants(
    'tokens_palette',
    constraints: const BoxConstraints.tightFor(width: 600, height: 312),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      padding: const EdgeInsets.all(HSpace.x3),
      child: const TokensPalette(),
    ),
  );
}
