// Goldens der Seitenleiste, eingeklappt und ausgeklappt (HUM-254), je in den
// vier Varianten aus `support/golden_variants.dart`. Erneuern mit
// `make goldens-update`.
//
// Der aktive Bereich ist „Rules" und nicht der erste: So zeigt das Bild, dass
// Füllung und Akzent-Rail in beiden Breiten an derselben Zeile stehen und nicht
// am Rand der Leiste kleben.

import 'package:flutter/widgets.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';

import 'support/golden_variants.dart';

void main() {
  // Breit genug für die ausgeklappte Leiste, hoch genug für sechs Einträge und
  // den Knopf am Fuß; beide Bilder im selben Rahmen, damit sie sich vergleichen
  // lassen.
  const BoxConstraints frame = BoxConstraints.tightFor(width: 240, height: 420);

  Widget rail(GoldenVariant variant, {required bool expanded}) => goldenFrame(
    variant: variant,
    background: variant.tokens.colors.bg0,
    child: Align(
      alignment: Alignment.topLeft,
      child: IconRail(
        active: Section.rules,
        onSelect: (_) {},
        expanded: expanded,
        onToggle: () {},
      ),
    ),
  );

  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'icon_rail_collapsed',
    constraints: frame,
    builder: (GoldenVariant variant) => rail(variant, expanded: false),
  );

  goldenVariants(
    'icon_rail_expanded',
    constraints: frame,
    builder: (GoldenVariant variant) => rail(variant, expanded: true),
  );
}
