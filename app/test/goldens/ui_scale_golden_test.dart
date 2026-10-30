// Goldens der Skalierung (HUM-356): Intercept mit drei gehaltenen Anfragen,
// dunkel, bei 100, 150 und 200 %.
//
// Die Fenster wachsen mit der Stufe, 1280 x 800, 1920 x 1200 und
// 2560 x 1600, sodass die Shell sich jedes Mal in denselben 1280 x 800
// logischen Pixeln anlegt: Die drei Bilder zeigen dasselbe Layout in drei
// Größen, mit Haarlinien auf ganzen Pixeln. Ein Layout, das bei einer Stufe
// anders umbräche oder etwas abschnitte, fiele hier auf.
//
// Erneuern mit `make goldens-update`.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/ui_scale.dart';
import 'package:humanitl/features/shell/providers/theme.dart';

import 'intercept_golden_test.dart'
    show FixedTheme, goldenDetails, overridesFor;

/// Eine Wahl, die feststeht.
class FixedScale extends UiScale {
  /// Bleibt bei [percent].
  FixedScale(this.percent);

  /// Die feste Stufe.
  final int percent;

  @override
  int build() => percent;
}

void main() {
  for (final (int percent, Size window) in <(int, Size)>[
    (100, const Size(1280, 800)),
    (150, const Size(1920, 1200)),
    (200, const Size(2560, 1600)),
  ]) {
    goldenTest(
      'intercept_scale_${percent}_dark',
      fileName: 'intercept_scale_${percent}_dark',
      constraints: BoxConstraints.tight(window),
      builder: () {
        final List<FlowDetail> details = goldenDetails();
        return ProviderScope(
          // overridesFor bringt die eigene UI-Zustandsdatei schon mit
          // (uiStateOverride() in intercept_golden_test.dart); der Satz hier
          // steht für den Wächter in test/harness/ui_state_guard_test.dart.
          overrides: [
            ...overridesFor(details),
            themeModeProvider.overrideWith(() => FixedTheme(HThemeMode.dark)),
            uiScaleProvider.overrideWith(() => FixedScale(percent)),
          ],
          child: const HumanitlApp(),
        );
      },
    );
  }
}
