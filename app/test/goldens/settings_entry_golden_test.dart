// Golden des sichtbaren Eingangs in die Einstellungen (HUM-275), in den vier
// Varianten aus `support/golden_variants.dart`: das Zahnrad der Kopfleiste und
// darunter der Knopf, den ein `ChangeSetting` auf `llm.api_key_ref` in einer
// Diagnostik-Karte bekommt. Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/settings_opener.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';

import 'support/golden_variants.dart';

void main() {
  const BoxConstraints frame = BoxConstraints.tightFor(width: 360, height: 112);

  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'settings_entry',
    constraints: frame,
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      padding: const EdgeInsets.all(12),
      child: SettingsOpener(
        open: (String? _) {},
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            SettingsButton(onPressed: () {}),
            const SizedBox(height: 12),
            const FixControl(
              fix: FixAction.changeSetting(
                key: FormKeys.llmApiKeyRef,
                value: '',
              ),
            ),
          ],
        ),
      ),
    ),
  );
}
