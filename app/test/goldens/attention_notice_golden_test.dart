// Golden des Hinweises „This desktop has no tray" (HUM-276), in den vier
// Varianten aus `support/golden_variants.dart`: seit HUM-302 eine schwebende
// Karte in ihrer größten Breite (`noticeMaxWidth`), oben rechts auf dem Grund
// der Shell, mit dem Schließen in der Ecke der Karte. Erneuern mit
// `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/core/ui/h_notice_layer.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';

import 'support/golden_variants.dart';

void main() {
  const BoxConstraints frame = BoxConstraints.tightFor(
    width: noticeMaxWidth + 16,
    height: 260,
  );

  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'attention_notice_banner',
    constraints: frame,
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      child: Padding(
        padding: const EdgeInsets.all(8),
        child: Align(
          alignment: Alignment.topRight,
          child: AttentionNoticeBanner(
            diagnostic: TrayDiagnostics.trayUnavailable(
              'no org.kde.StatusNotifierWatcher answers on the session bus',
            ),
            onDismiss: () {},
          ),
        ),
      ),
    ),
  );
}
