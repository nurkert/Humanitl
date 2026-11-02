// Goldens der Pause des Abfangens (HUM-407) in den vier Varianten aus
// `support/golden_variants.dart`: die Pille eingeschaltet und pausiert, die
// pausierte mit dem Absperrband darunter, und die Rückfrage vor der Pause.
// Erneuern mit `make goldens-update`.

import 'package:flutter/widgets.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/connection.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/providers/intercept_pause.dart';
import 'package:humanitl/features/shell/widgets/intercept_pill.dart';
// `Override` lebt in riverpod 3 im Nebeneingang `misc.dart`.
import 'package:flutter_riverpod/misc.dart';

import 'support/golden_variants.dart';

/// Ein Stand, den der Test vorgibt, statt ihn beim Daemon zu erfragen.
class _FixedPause extends InterceptPause {
  _FixedPause(this.status);

  final InterceptStatus status;

  @override
  InterceptStatus build() => status;
}

List<Override> _overrides(InterceptStatus status) => <Override>[
  linkLiveProvider.overrideWithValue(true),
  interceptPausableProvider.overrideWithValue(true),
  interceptPauseProvider.overrideWith(() => _FixedPause(status)),
];

/// Ein Streifen der Kopfleiste: die Pille, darunter das Band, wenn pausiert.
Widget _strip(GoldenVariant variant, InterceptStatus status) => goldenFrame(
  variant: variant,
  overrides: _overrides(status),
  child: Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    children: <Widget>[
      Padding(
        padding: const EdgeInsets.all(8),
        child: Align(
          alignment: Alignment.centerLeft,
          child: InterceptPill(onAskPause: () {}),
        ),
      ),
      if (status.paused) const HCautionTape(),
    ],
  ),
);

void main() {
  const BoxConstraints strip = BoxConstraints.tightFor(width: 320, height: 52);
  final InterceptStatus paused = InterceptStatus(
    paused: true,
    pausedSince: DateTime.utc(2026, 11, 1, 9),
  );

  // Je ein Aufruf mit dem Namen als Literal: `golden_variants_complete_test`
  // liest die Namen aus dem Quelltext.
  goldenVariants(
    'intercept_pill_on',
    constraints: strip,
    builder: (GoldenVariant variant) => _strip(variant, InterceptStatus.on),
  );

  goldenVariants(
    'intercept_pill_paused',
    constraints: strip,
    builder: (GoldenVariant variant) => _strip(variant, paused),
  );

  goldenVariants(
    'intercept_pause_modal',
    constraints: const BoxConstraints.tightFor(width: 640, height: 400),
    builder: (GoldenVariant variant) => goldenFrame(
      variant: variant,
      background: variant.tokens.colors.bg0,
      overrides: _overrides(InterceptStatus.on),
      child: InterceptPauseDialog(onClose: () {}),
    ),
  );
}
