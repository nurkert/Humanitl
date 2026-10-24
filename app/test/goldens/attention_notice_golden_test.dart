// Golden des Banners „This desktop has no tray" (HUM-276), in den vier
// Varianten aus `support/golden_variants.dart`: über die ganze Breite der
// schmalsten Fensterbreite, die die App zulässt (1100 px), auf dem Grund der
// Shell, mit der Schliessen-Taste oben rechts. Erneuern mit
// `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';

import 'support/golden_variants.dart';

void main() {
  const BoxConstraints frame = BoxConstraints.tightFor(
    width: 1100,
    height: 140,
  );

  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'attention_notice_banner',
    constraints: frame,
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          AttentionNoticeBanner(
            diagnostic: TrayDiagnostics.trayUnavailable(
              'no org.kde.StatusNotifierWatcher answers on the session bus',
            ),
            onDismiss: () {},
          ),
        ],
      ),
    ),
  );
}
