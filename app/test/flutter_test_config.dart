// Läuft vor jeder Testdatei. Goldens entstehen nur im CI-Modus von alchemist
// (Text als Blöcke, keine Schrift), damit lokal und in CI dieselben Pixel
// verglichen werden (HUM-019 Akzeptanzkriterien).

import 'dart:async';

import 'package:alchemist/alchemist.dart';

Future<void> testExecutable(FutureOr<void> Function() testMain) {
  return AlchemistConfig.runWithConfig(
    config: const AlchemistConfig(
      platformGoldensConfig: PlatformGoldensConfig(enabled: false),
      ciGoldensConfig: CiGoldensConfig(),
    ),
    run: () async {
      await testMain();
    },
  );
}
