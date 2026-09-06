// Wo ein Befund im Sandbox-Bildschirm steht, und wo er nie steht (HUM-068).
//
// Zwei Zusagen halten diese Tests fest. Die erste ist eine Ortsangabe: Ein
// Befund der Sitzung steht **im** Bildschirm, an der Stelle, an der er
// entstanden ist, und nie in einem Modal — ein Modal nimmt einem Menschen die
// Sicht auf das, worüber es spricht, und verlangt eine Antwort auf eine
// Meldung, die keine Frage ist (`docs/UX.md` 4.4). Die zweite ist eine Farbe:
// Blockierend ist `state.error`, nie das Rot des Blockierens — Rot heißt in
// diesem Programm „diese Anfrage ist nicht hinausgegangen", und ein Befund ist
// keine Entscheidung (`docs/UX.md` Regel 6).

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/sandbox/sandbox_screen.dart';

import 'harness.dart';

/// Ein Befund der Sitzung, wie ihn `Sandbox(Start)` mitschickt.
Diagnostic finding(String code, Severity severity) => Diagnostic(
  code: code,
  severity: severity,
  title: 'a finding of the session',
  why: 'the daemon said so, and the screen shows the sentence of the daemon',
);

/// Die Karten, die der Bildschirm gerade zeigt.
Iterable<HDiagnosticCard> cards(WidgetTester tester) =>
    tester.widgetList<HDiagnosticCard>(find.byType(HDiagnosticCard));

/// Zehn Befunde, wie sie eine Sitzung mit einem gehärteten Profil sammelt.
///
/// Zwei davon tragen denselben Code. Das ist der Normalfall und kein
/// Sonderfall — zwei verbotene Mounts sind zwei `SANDBOX_006` —, und bis zum
/// 2026-09-06 stürzte der Bildschirm daran mit „Duplicate keys" ab.
List<Diagnostic> manyFindings() => <Diagnostic>[
  for (int index = 0; index < 9; index += 1)
    finding('SANDBOX_02$index', Severity.warning),
  finding('SANDBOX_020', Severity.warning),
];

void main() {
  testWidgets('a_session_finding_stands_in_the_screen_and_not_in_a_modal', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    client.sandbox = client.sandbox.copyWith(
      diagnostics: <Diagnostic>[
        finding('SANDBOX_020', Severity.warning),
        finding('SANDBOX_013', Severity.blocking),
      ],
    );
    await pumpSandbox(tester, client: client);

    // Beide stehen da, als Karten im Fluss des Bildschirms.
    expect(cards(tester).length, greaterThanOrEqualTo(2));
    expect(
      cards(tester).map((HDiagnosticCard card) => card.code),
      containsAll(<String>['SANDBOX_020', 'SANDBOX_013']),
    );

    // Und nichts davon in einem Modal: `HModal` ist der Typ, den dieses
    // Programm für Fragen benutzt, und ein Befund ist keine.
    expect(find.byType(HModal), findsNothing);
  });

  /// Der Deckel ist gemessen und nicht behauptet.
  ///
  /// Der erste Entwurf hatte den `LayoutBuilder` **in** der Spalte, und eine
  /// vertikale Spalte reicht ihren Kindern `maxHeight: double.infinity`: Ein
  /// Drittel davon ist wieder unendlich, der Deckel wirkte nicht, und der
  /// Test von damals merkte es nicht, weil er drei Befunde auf einer großen
  /// Fläche zeigte. Dieser Test nimmt zehn Befunde auf einer kleinen.
  testWidgets('ten_findings_stay_inside_a_third_of_a_small_screen', (
    WidgetTester tester,
  ) async {
    const Size small = Size(900, 600);
    final SandboxTestClient client = SandboxTestClient();
    client.sandbox = client.sandbox.copyWith(diagnostics: manyFindings());
    await pumpSandbox(tester, client: client, size: small);

    // Der Block bleibt im Drittel, obwohl zehn Karten darin stehen.
    final double height = tester
        .getSize(find.byType(SingleChildScrollView).first)
        .height;
    expect(
      height,
      lessThanOrEqualTo(small.height / 3 + 1),
      reason: 'the findings take a third at most, the rest belongs to the tab',
    );

    // Und der Fuß des Bildschirms steht noch auf dem Schirm: Was darunter
    // liegt, hat niemand aus dem Fenster geschoben.
    expect(
      tester.getBottomLeft(find.byType(SandboxStatusBar)).dy,
      lessThanOrEqualTo(small.height),
    );

    // Kein Überlauf: Ein `RenderFlex overflowed` wäre eine Exception.
    expect(tester.takeException(), isNull);
  });

  testWidgets(
    'a_blocking_finding_wears_the_error_hue_and_never_the_blocked_red',
    (WidgetTester tester) async {
      final SandboxTestClient client = SandboxTestClient();
      client.sandbox = client.sandbox.copyWith(
        diagnostics: <Diagnostic>[
          finding('SANDBOX_013', Severity.blocking),
          finding('SANDBOX_021', Severity.info),
          finding('SANDBOX_020', Severity.warning),
        ],
      );
      await pumpSandbox(tester, client: client);

      final HTokens tokens = HTheme.of(
        tester.element(find.byType(HDiagnosticCard).first),
      );
      final Map<String, Color> byCode = <String, Color>{
        for (final HDiagnosticCard card in cards(tester)) card.code: card.color,
      };

      expect(byCode['SANDBOX_013'], tokens.state.error);
      expect(byCode['SANDBOX_020'], tokens.state.held);
      expect(byCode['SANDBOX_021'], tokens.colors.accent);
      // Die eine Farbe, die keine davon sein darf.
      for (final Color used in byCode.values) {
        expect(
          used,
          isNot(tokens.state.blocked),
          reason: 'red means blocked, and a finding is not a decision',
        );
      }
    },
  );
}
