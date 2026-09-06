// Die Suche nach Modellservern im eigenen Netz (HUM-076).
//
// Drei Zusagen halten diese Tests fest: Ohne Klick entsteht keine einzige
// Verbindung -- weder beim Oeffnen des Bildschirms noch beim Oeffnen des
// Blattes; ein Fund traegt genau das, was der Server gesagt hat, und ein
// Server, der sich nicht ausgewiesen hat, ist nicht uebernehmbar; und ein
// uebernommener Server steht danach im Feld und ist gemessen, nicht geraten.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';

import '../../harness/app_harness.dart';

/// Oeffnet den Setup-Abschnitt der Shell.
Future<void> openSetup(WidgetTester tester, FakeDaemonClient client) async {
  await pumpApp(tester, client: client);
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
}

/// Oeffnet das Such-Blatt, ohne zu suchen.
Future<void> openSheet(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('setup-llm-discover')));
  await tester.pump();
}

/// Startet die Suche, laesst den Strom auslaufen und die Bewegung zur Ruhe
/// kommen.
///
/// Die letzte Pause ist keine Vorsicht, sondern noetig: Das Blatt waechst mit
/// `AnimatedSize`, und wer waehrend dieser Bewegung tippt, trifft die Stelle,
/// an der der Knopf gleich sein wird, und nicht die, an der er ist.
Future<void> search(WidgetTester tester) async {
  await tester.tap(find.byKey(const Key('setup-discover-start')));
  await tester.pump();
  // Der Fake versetzt jede Zeile um 30 ms; eine Sekunde deckt jede Vorgabe.
  await tester.pump(const Duration(seconds: 1));
  await tester.pump(const Duration(milliseconds: 400));
}

void main() {
  testWidgets('opening_the_setup_contacts_nothing', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient();
    await openSetup(tester, client);

    // Ein Netzscan, der beim Oeffnen eines Bildschirms losliefe, waere genau
    // der Vorgang, den die Spezifikation ausschliesst.
    expect(client.discoverCalls, isEmpty);

    await openSheet(tester);

    // Auch das Blatt selbst sucht nicht: Es erklaert erst.
    expect(client.discoverCalls, isEmpty);
    expect(find.byKey(const Key('setup-discover-start')), findsOneWidget);
    expect(find.byKey(const Key('setup-discover-servers')), findsNothing);

    await search(tester);

    expect(client.discoverCalls, hasLength(1));
  });

  testWidgets('a_found_server_shows_what_it_answered', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmServers = <LlmServer>[
        const LlmServer(
          host: '192.168.2.20',
          port: 11434,
          flavor: LlmFlavor.ollama,
          models: <String>['qwen2.5-coder:14b', 'llama3.1:8b'],
          latencyMs: 14,
        ),
      ];
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(find.text('http://192.168.2.20:11434'), findsOneWidget);
    expect(find.text('qwen2.5-coder:14b'), findsOneWidget);
    expect(find.text('ollama'), findsOneWidget);
    expect(find.text('14 ms'), findsOneWidget);
    expect(
      find.byKey(const Key('setup-discover-use-192.168.2.20-11434')),
      findsOneWidget,
    );
  });

  /// Ein Server, der auf einem der vier Ports horcht, sich aber nicht
  /// ausgewiesen hat, steht in der Liste und ist trotzdem nicht uebernehmbar:
  /// Ein Klick trueg eine Adresse in die Konfiguration, die nie als
  /// Modellserver geantwortet hat.
  testWidgets('a_server_without_an_api_cannot_be_taken_over', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmServers = <LlmServer>[
        const LlmServer(host: '192.168.2.30', port: 8080),
      ];
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(find.text('http://192.168.2.30:8080'), findsOneWidget);
    expect(find.text('did not identify itself'), findsOneWidget);
    expect(
      find.byKey(const Key('setup-discover-use-192.168.2.30-8080')),
      findsNothing,
    );
  });

  /// Ein Server hinter einer Anmeldung bleibt in der Liste, ohne Modelle, die
  /// ihn niemand gefragt hat.
  testWidgets('a_server_behind_a_login_is_listed_without_models', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmServers = <LlmServer>[
        // Der Dienst nennt einen Server hinter einer Anmeldung `unknown`:
        // Die Probe fragt zuerst `/api/tags`, und ein `401` von dort sagt
        // nichts darüber, was dahinter steht.
        const LlmServer(host: '192.168.2.40', port: 8000, authRequired: true),
      ];
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(find.text('asks for credentials'), findsOneWidget);
    expect(
      find.byKey(const Key('setup-discover-use-192.168.2.40-8000')),
      findsOneWidget,
      reason:
          'a server that answered is offered; what it needs stands next to it',
    );
  });

  testWidgets('taking_a_server_over_fills_the_field_and_measures_it', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmServers = <LlmServer>[
        const LlmServer(
          host: '192.168.2.20',
          port: 11434,
          flavor: LlmFlavor.ollama,
          models: <String>['qwen2.5-coder:14b'],
        ),
      ];
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(client.probedEndpoints, isEmpty);
    await tester.tap(
      find.byKey(const Key('setup-discover-use-192.168.2.20-11434')),
    );
    await tester.pump();
    await tester.pump();

    // Das Blatt ist zu, die Adresse steht im Feld, und sie ist gemessen: Eine
    // Zeile, die eine Adresse zeigt und daneben die Messung einer anderen,
    // waere die Luege, die CONVENTIONS 4.13 ausschliesst.
    expect(find.byKey(const Key('setup-discover-start')), findsNothing);
    expect(
      tester
          .widget<EditableText>(
            find.descendant(
              of: find.byKey(const Key('setup-llm-endpoint')),
              matching: find.byType(EditableText),
            ),
          )
          .controller
          .text,
      'http://192.168.2.20:11434',
    );
    expect(client.probedEndpoints, <String>['http://192.168.2.20:11434']);
  });

  /// Ein Netz, das der Dienst zurueckweist, erscheint als Befund und nicht als
  /// leere Liste: „nichts gefunden" waere eine andere Aussage als „hier wird
  /// nicht gesucht".
  testWidgets('a_refused_network_is_a_finding_and_not_an_empty_list', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmDiscoverFailure = const Diagnostic(
        code: 'LLM_008',
        severity: Severity.error,
        title: 'Die Suche im Netz kann nicht stattfinden',
        why: 'there is no default route on this host',
      );
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(find.byKey(const Key('setup-discover-failure')), findsOneWidget);
    expect(find.byKey(const Key('setup-discover-empty')), findsNothing);
  });

  testWidgets('a_search_without_an_answer_says_what_to_look_at', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..llmServers = <LlmServer>[];
    await openSetup(tester, client);
    await openSheet(tester);
    await search(tester);

    expect(find.byKey(const Key('setup-discover-empty')), findsOneWidget);
    expect(find.byKey(const Key('setup-discover-servers')), findsNothing);
  });
}
