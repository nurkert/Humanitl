// Läuft vor jeder Testdatei. Goldens entstehen nur im CI-Modus von alchemist
// (Text als Blöcke, keine Schrift), damit lokal und in CI dieselben Pixel
// verglichen werden (HUM-019 Akzeptanzkriterien).
//
// Die Plattform-Goldens mit echten Schriften bleiben auch lokal aus, anders
// als `backlog/sprint-4.md` HUM-054 es skizziert: Ihre Bilder sind
// gitignored (`**/goldens/platform/`), und alchemist lässt einen Vergleich
// gegen eine fehlende Datei scheitern. Eingeschaltet wäre `make goldens` auf
// jedem frischen Auscheckstand rot, bis jemand die Bilder dort mit seinen
// eigenen Schriften erzeugt, und grün danach, ohne etwas bewacht zu haben.
// Wer deutsche Texte mit echter Schrift sehen will, schaltet sie für einen
// Lauf hier ein; den Überlauf im Deutschen prüft `no_overflow_in_de` ohne sie.
//
// Das Absperrband (HUM-260) steht in allen Tests der Anwendung und in allen
// Goldens still: es läuft sonst endlos, plant endlos Frames, und jedes
// `pumpAndSettle` über einer gehaltenen Anfrage liefe in seine Zeitgrenze.
// Dass es läuft, prüfen die Tests des Bands in `app/packages/ui`.

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

Future<void> testExecutable(FutureOr<void> Function() testMain) {
  HCautionTape.debugFreeze = true;
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
