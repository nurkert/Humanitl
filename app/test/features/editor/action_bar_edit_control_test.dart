/// Das Control „Edit + Allow" in der Aktionsleiste (HUM-047).
///
/// Es steht hier und nicht bei den Tests der Warteschlange, weil es der
/// Eingang zum Editor ist und mit ihm kam. Geprüft wird, was der dritte Knopf
/// in der Leiste kostet: Mit ihm lief die breite Zeile bei 652 px um 142 px
/// über, und `actionBarWrapWidth` steht deshalb auf 800 statt 640.
library;

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import '../intercept/fixtures.dart';
import '../intercept/harness.dart';

void main() {
  // Fensterbreiten, bei denen der mittlere Pane zwischen etwa 570 und 850 px
  // breit ist: genau das Band, in dem die breite Zeile mit drei Controls
  // überlaufen kann, wenn die Schwelle zu niedrig liegt.
  for (final double width in <double>[
    1300,
    1350,
    1400,
    1450,
    1500,
    1550,
    1600,
    1650,
    1700,
    1800,
    1900,
  ]) {
    testWidgets('the bar with the edit control fits at ${width.toInt()} px', (
      WidgetTester tester,
    ) async {
      final FakeDaemonClient client = fakeDaemon(
        holdScript(<FlowDetail>[held(1)]),
      );

      await pumpIntercept(tester, client: client, size: Size(width, 900));
      await playScript(tester);

      // Ein Überlauf meldet Flutter im Test als Ausnahme; eine Leiste, die
      // überläuft, versteckt eine Entscheidung, statt sie umzubrechen
      // (`docs/UX.md` 6).
      expect(tester.takeException(), isNull);
      expect(find.byKey(const Key('intercept-edit')), findsOne);
      expect(find.byKey(const Key('intercept-allow')), findsOne);
      expect(find.byKey(const Key('intercept-block')), findsOne);
    });
  }
}
