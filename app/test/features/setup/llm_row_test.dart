// Die Modell-Zeile des Setup-Bildschirms (HUM-044, HUM-039).
//
// Sie ist die einzige Zeile, hinter der eine Verbindung ins Netz steht, und
// die Tests hier halten drei Zusagen fest: Nichts geht hinaus, solange jemand
// tippt; was der Server nennt, wird gedeckelt gezeigt und nie zu einem Befehl;
// und die Befunde `LLM_006` und `LLM_007` stehen in der Zeile, nicht nur der
// gruene Fall.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';

import '../../harness/app_harness.dart';

/// Ein Fake, dessen `ProbeLlm` etwas wirft, das keine [DaemonException] ist.
///
/// Genau das kann passieren: `probeLlm` wandelt die Antwort noch innerhalb
/// seines `try` um (`grpc_daemon_client.dart`, `response.toDomain`), und ein
/// `StateError`, eine `FormatException` oder ein `TimeoutException` von dort
/// geht an den beiden `on`-Zweigen vorbei.
class ThrowingProbeClient extends FakeDaemonClient {
  @override
  Future<LlmProbe> probeLlm(String endpoint, {Duration? timeout}) async {
    throw StateError('the answer had no flavor this build knows');
  }
}

/// Oeffnet den Setup-Abschnitt der Shell.
Future<void> openSetup(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
}

/// Der Text der Zustandszeile der Modell-Zeile.
String llmState(WidgetTester tester) =>
    tester.widget<Text>(find.byKey(const Key('setup-state-llm'))).data!;

/// Tippt [endpoint] in das Feld, ohne zu senden.
Future<void> type(WidgetTester tester, String endpoint) async {
  await tester.enterText(find.byKey(const Key('setup-llm-endpoint')), endpoint);
  await tester.pump();
}

/// Drueckt den Knopf, der wirklich eine Verbindung aufbaut.
Future<void> probe(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('setup-llm-probe')));
  await tester.pump();
  await tester.pump();
}

void main() {
  testWidgets('llm_row_uses_probe', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await openSetup(tester, client);

    // Vor der Bitte: nichts gemessen, und nichts angesprochen.
    expect(llmState(tester), 'not measured');
    expect(client.probedEndpoints, isEmpty);

    // Tippen ist keine Bitte. Das ist die Zusage, die diese Zeile traegt:
    // sonst ginge `h`, `ht`, `htt`, ... in DNS, bevor jemand entschieden hat.
    await type(tester, 'http://192.168.1.10:11434');
    expect(client.probedEndpoints, isEmpty);

    await probe(tester);
    expect(client.probedEndpoints, <String>['http://192.168.1.10:11434']);
    expect(llmState(tester), 'ok');
    expect(find.byKey(const Key('setup-llm-models')), findsOneWidget);
  });

  testWidgets('llm_row_shows_llm_006_and_007', (WidgetTester tester) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await openSetup(tester, client);

    // `LLM_006`: der Endpunkt liegt nicht im eigenen Netz. Ein Befund, kein
    // Fehler -- der Start bleibt moeglich, und die Zeile sagt es trotzdem.
    await type(tester, 'http://api.example.com:11434');
    await probe(tester);
    expect(llmState(tester), 'worth knowing');
    expect(find.text('LLM_006'), findsOneWidget);

    // `LLM_007`: was keine URL ist, beantwortet der Daemon nicht mit einer
    // erfundenen Modellliste.
    await type(tester, 'not-a-url');
    await probe(tester);
    expect(llmState(tester), 'blocks the start');
    expect(find.text('LLM_007'), findsOneWidget);
    expect(find.byKey(const Key('setup-llm-models')), findsNothing);
  });

  testWidgets('model_chip_is_clamped', (WidgetTester tester) async {
    // Die Namen kommen woertlich von einem Server im LAN. Weder ihre Zahl noch
    // ihre Laenge begrenzt der Daemon; die Oberflaeche tut es.
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmProbe = LlmProbe(
        endpoint: 'http://192.168.1.10:11434',
        models: <String>[for (int i = 0; i < 20; i++) '${'x' * 200}-$i'],
        flavor: LlmFlavor.ollama,
        endpointIsPrivate: true,
      );
    await openSetup(tester, client);
    await type(tester, 'http://192.168.1.10:11434');
    await probe(tester);

    final Iterable<HBadge> chips = tester
        .widgetList<HBadge>(
          find.descendant(
            of: find.byKey(const Key('setup-llm-models')),
            matching: find.byType(HBadge),
          ),
        )
        .toList();
    // Sechs Namen und ein Abzeichen, das sagt, wie viele fehlen.
    expect(chips.length, LlmModelLimits.count + 1);
    expect(chips.last.text, '+14 more');
    for (final HBadge chip in chips.take(LlmModelLimits.count)) {
      expect(
        chip.text.runes.length,
        lessThanOrEqualTo(LlmModelLimits.nameLength),
        reason: chip.text,
      );
    }
    // Und kein Name wird je zu einem Befehl. Gepruft wird der Chip-Bereich
    // selbst und nicht nur die Diagnose-Karte: Die Karte gibt es in diesem
    // Test gar nicht -- die Probe hat keinen Befund geliefert --, also waere
    // eine Erwartung auf ihren Kopierknopf leer. Die LAN-Namen stehen hier,
    // also steht hier auch der Riegel (`backlog/sprint-3.md`, HUM-044,
    // feindliche Eingabe 1).
    final Finder chipRow = find.byKey(const Key('setup-llm-models'));
    expect(
      find.descendant(of: chipRow, matching: find.byType(FixControl)),
      findsNothing,
      reason: 'a model name never becomes a FixAction',
    );
    expect(
      find.descendant(of: chipRow, matching: find.byType(HButton)),
      findsNothing,
      reason: 'a model name never becomes a control',
    );
    expect(
      tester.widget<Wrap>(chipRow).children.every((Widget it) => it is HBadge),
      isTrue,
      reason: 'a model name is a badge and nothing else',
    );
    // Die Karte bleibt die zweite, eigene Erwartung.
    expect(find.byKey(const Key('setup-fix-copy-llm')), findsNothing);
  });

  testWidgets('a result is not shown next to another address', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await openSetup(tester, client);
    await type(tester, 'http://192.168.1.10:11434');
    await probe(tester);
    expect(llmState(tester), 'ok');

    // Wer weitertippt, bekommt das alte Ergebnis nicht als Aussage ueber die
    // neue Adresse.
    await type(tester, 'http://192.168.1.11:11434');
    expect(llmState(tester), 'not measured');
    expect(find.byKey(const Key('setup-llm-models')), findsNothing);
  });

  /// Der Aufrufer wartet die Probe nicht ab (`unawaited`), also sieht niemand
  /// einen Wurf, den sie nicht selbst faengt: Die Zeile stuende fuer immer auf
  /// `checking`, Feld und Knopf sind dabei gesperrt, `forget()` haette nichts
  /// zu vergessen, und nur ein Neustart der Anwendung raeumte das weg.
  testWidgets('a probe that throws something else ends the row', (
    WidgetTester tester,
  ) async {
    final ThrowingProbeClient client = ThrowingProbeClient();
    await openSetup(tester, client);
    await type(tester, 'http://192.168.1.10:11434');
    await probe(tester);

    // Die Zeile ist fertig, nicht am Fragen.
    expect(llmState(tester), isNot('checking'));
    expect(llmState(tester), 'blocks the start');
    expect(find.text(DiagnosticCodes.daemonUnreachable), findsWidgets);

    // Und die Zeile ist wieder bedienbar: Feld und Knopf haengen an `busy`.
    expect(
      tester.widget<HButton>(find.byKey(const Key('setup-llm-probe'))).enabled,
      isTrue,
    );
  });

  /// Die Adresse, die niemand beantwortet (HUM-039).
  ///
  /// Der Dienst misst das selbst: `humanitl doctor --probe-llm` gegen
  /// `http://10.255.255.1:1` antwortet am 2026-09-06 nach 3 s mit
  /// `LLM_001: Humanitl could not reach 10.255.255.1:1 from this machine` und
  /// dem Befehl `curl -sS http://10.255.255.1:1/api/tags`. Hier steht die
  /// andere Hälfte: dass die Zeile genau das zeigt — den Code und einen
  /// Befehl, den ein Mensch kopieren kann, statt einer Zeile „ging nicht".
  testWidgets('an address that nobody answers shows LLM_001 and its curl', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      // `Severity.blocking`, wie der Dienst ihn baut
      // (`daemon/crates/proxy/src/llm_probe.rs`, `Unreachable`): Ein Modell,
      // das nicht antwortet, hält den Start auf, und die Zeile muss das zeigen.
      ..llmProbeFailure = const Diagnostic(
        code: 'LLM_001',
        severity: Severity.blocking,
        title: 'Sprachmodell nicht erreichbar',
        why:
            'Humanitl could not reach 10.255.255.1:1 from this machine (no '
            'answer within 3000 ms). The agent will not be able to talk to the '
            'model.',
        fix: FixAction.copyCommand(
          command: 'curl -sS http://10.255.255.1:1/api/tags',
        ),
      );
    await openSetup(tester, client);
    await type(tester, 'http://10.255.255.1:1');
    await probe(tester);

    expect(llmState(tester), 'blocks the start');
    expect(find.text('LLM_001'), findsWidgets);
    expect(
      find.text('curl -sS http://10.255.255.1:1/api/tags'),
      findsOneWidget,
      reason: 'the command stands there to be copied, not described',
    );
    expect(find.byKey(const Key('setup-fix-copy-llm')), findsOneWidget);
    // Und keine Modellliste neben einer Adresse, die nichts gesagt hat.
    expect(find.byKey(const Key('setup-llm-models')), findsNothing);
  });

  test('a probe that is still running is not an answer', () {
    // Reine Funktion, ohne Baum: Solange die Probe laeuft, ist die Zeile
    // `checking` und sperrt den Start, statt das letzte Ergebnis zu zeigen.
    final SetupState state = setupChecks(
      daemon: const DaemonLinkConnecting(),
      doctor: const AsyncValue<DoctorReport>.loading(),
      sandbox: const AsyncValue<SandboxStatus>.loading(),
      sandboxCall: SandboxCall.plan,
      llm: const LlmProbeState(busy: true),
    );
    expect(state[SetupCheckKind.llm].state, SetupCheckState.checking);
    expect(state.canStart, isFalse);
  });
}
