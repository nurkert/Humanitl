// Kein Fehlerzustand steht als Modal da (HUM-058, Test
// `no_modal_for_diagnostics`; `BACKLOG.md` 5, Modal nur für die drei
// destruktiven Fälle).
//
// Für jedes Szenario des `FakeDaemonClient` startet die App, läuft eine
// Weile, und weder ein `HModal` noch ein Material-`Dialog` steht auf dem
// Schirm. `find.byType(Dialog)` allein reichte nicht: Die Komponenten der
// Bibliothek legen ihre Overlays mit eigenen Typen an, und jedes Modal dieser
// App geht durch `HModal` (`packages/ui`, `h_modal.dart`).
//
// Die drei destruktiven Modale (Block-all, Forever-Regel löschen, Sandbox
// stoppen) öffnet nur eine Geste, und kein Szenario macht sie; deshalb steht
// hier keine Ausnahme.
//
// Gegen welche Umkehrung der Test gerichtet ist: Ein Befund, der als Modal
// aufgeht -- etwa die Karte des Setup-Bildschirms in einem `HModal` statt in
// der Zeile --, macht das Szenario rot, in dem er auftritt.

import 'package:flutter/material.dart' show Dialog;
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import '../../harness/app_harness.dart';

/// Jeder Name, den `FakeDaemonClient.scenario` kennt, und einer, den es nicht
/// kennt (der fällt auf das Standardskript zurück).
const List<String> scenarios = <String>[
  '',
  'daemon_missing',
  'unavailable',
  'incompatible',
  'unauthenticated',
  'empty',
  'rules:broken',
  'big_body',
  'burst:12',
  'history:40',
  'no-such-scenario',
];

void main() {
  for (final String name in scenarios) {
    testWidgets('no_modal_for_diagnostics ${name.isEmpty ? 'default' : name}', (
      WidgetTester tester,
    ) async {
      await pumpApp(
        tester,
        client: FakeDaemonClient.scenario(name),
        reconnect: const Duration(seconds: 2),
      );
      for (int i = 0; i < 8; i++) {
        await tester.pump(const Duration(milliseconds: 500));
      }

      expect(find.byType(HModal), findsNothing);
      expect(find.byType(Dialog), findsNothing);

      // Ohne offene Zeitgeber aus dem Test: der Baum geht, die Fakes hören auf.
      await tester.pumpWidget(const SizedBox.shrink());
      await tester.pump(const Duration(seconds: 1));
    });
  }

  testWidgets('the finder sees a modal when there is one', (
    WidgetTester tester,
  ) async {
    // Die Gegenprobe: Die Palette ist ein `HModal`, und sie wird gefunden;
    // sonst bewiese `findsNothing` oben nichts.
    await pumpApp(tester, client: FakeDaemonClient.scenario('empty'));
    await pressCtrl(tester, LogicalKeyboardKey.keyK);
    await tester.pump();
    expect(find.byType(HModal), findsOneWidget);
  });
}
