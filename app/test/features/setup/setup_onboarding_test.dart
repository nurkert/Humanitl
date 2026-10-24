// Das Setup als kurzes Onboarding (HUM-287): ein grüner Schritt ist eine
// Zeile, der erste Schritt, der den Menschen braucht, steht offen mit seiner
// Aktion als Primärknopf, und mit vier grünen Schritten sagt der Bildschirm
// „Humanitl is ready" und bietet genau eine Aktion an.
//
// Die Lage des Owners vom 2026-10-23 ist der Kern dieser Datei: Der Dienst
// antwortet, der Modellserver lehnt den Schlüssel ab (`LLM_002`), es ist kein
// Ordner gewählt, und die Maschine ist in Ordnung. Dann steht der
// Modellschritt offen, mit dem Weg ins Einstellungsblatt als Primärknopf und
// dem Ordner als nächstem Schritt, und nichts davon braucht Scrollen.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/config_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/work_dir_picker.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/setup/widgets/setup_check_row.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/shell_screen.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import '../../harness/app_harness.dart';

/// Der Befund, mit dem der Doctor einen Modellserver meldet, der den
/// hinterlegten Schlüssel mit 401 ablehnt (`checks.rs`, HUM-283).
const Diagnostic keyRefused = Diagnostic(
  code: 'LLM_002',
  severity: Severity.error,
  why: 'the LLM server answered 401 at https://llm.example.net/api/v1/models',
  fix: FixAction.changeSetting(key: 'llm.api_key_ref', value: ''),
);

/// Die Maschine des Owners am 2026-10-23: alles gemessen und in Ordnung bis
/// auf den Modellserver, der den Schlüssel ablehnt.
DoctorReport ownersDoctor() => DoctorReport(
  checks: <DoctorCheck>[
    for (final DoctorCheck check in fakeDoctorOk().checks)
      if (check.id == DoctorCheckId.llm)
        check.copyWith(
          status: DoctorStatus.fail,
          evidence:
              'https://llm.example.net/api/v1/ answered 401; an access key '
              'is stored',
          diagnostic: keyRefused,
        )
      else
        check,
  ],
);

/// Der Server, den die Einstellungen als aktiven speichern.
const String home = 'http://192.168.1.50:11434';

/// Der zweite gespeicherte Server.
const String lab = 'http://192.168.1.77:11434';

/// Speichert in [client] zwei Server, `home` aktiv und mit Schlüssel, so wie
/// die Einstellungen sie schreiben (HUM-289).
T withServers<T extends FakeDaemonClient>(T client, {String endpoint = home}) {
  client.savedServers = <SavedServer>[
    SavedServer(
      name: 'home',
      endpoint: endpoint,
      models: const <String>['qwen2.5-coder:14b'],
      keyRef: 'secret-service:humanitl/home',
    ),
    const SavedServer(
      name: 'lab',
      endpoint: lab,
      models: <String>['llama3.1:8b'],
    ),
  ];
  unawaited(client.activateLlmServer('home'));
  client.llmKey = const LlmKeyStatus(
    set: true,
    store: LlmKeyStore.secretService,
    keyRef: 'secret-service:humanitl/home',
  );
  return client;
}

/// Der Fake in der Lage des Owners: kein Ordner, Schlüssel abgelehnt.
FakeDaemonClient ownersState() => withServers(
  FakeDaemonClient()
    ..doctorReport = ownersDoctor()
    // Die Probe, die das Setup beim Öffnen selbst macht, bekommt dieselbe
    // Ablehnung wie der Doctor: Der Server will einen anderen Schlüssel.
    ..llmProbeFailure = keyRefused
    ..sandbox = const SandboxStatus(),
  endpoint: 'https://llm.example.net/api/v1/',
);

/// Der Bericht eines echten Daemons über einen eingerichteten Rechner: Den
/// Modellserver hat der Doctor nicht angesprochen (`DOCTOR_013`).
DoctorReport setDoctor() => DoctorReport(
  checks: <DoctorCheck>[
    for (final DoctorCheck check in fakeDoctorOk().checks)
      if (check.id == DoctorCheckId.llm)
        check.copyWith(
          status: DoctorStatus.warn,
          evidence: 'not contacted: nobody asked for a probe',
          diagnostic: const Diagnostic(
            code: DiagnosticCodes.doctorNotContacted,
            severity: Severity.warning,
            why: 'the endpoint is contacted only when somebody asks for it',
          ),
        )
      else
        check,
  ],
);

/// Ein Rechner, auf dem alles eingerichtet ist: Der Server ist gespeichert,
/// und das Setup spricht ihn beim Öffnen selbst an.
FakeDaemonClient everythingSet() =>
    withServers(FakeDaemonClient()..doctorReport = setDoctor());

/// Ein Rechner ohne Ordner, sonst in Ordnung.
FakeDaemonClient withoutFolder() => FakeDaemonClient()
  ..doctorReport = fakeDoctorOk()
  ..sandbox = const SandboxStatus();

/// Ein Fake, dessen Proben warten, bis der Test sie freigibt.
class StallingProbeClient extends FakeDaemonClient {
  /// Erzeugt den Fake über einem eingerichteten Rechner.
  StallingProbeClient() {
    doctorReport = setDoctor();
    withServers(this);
  }

  /// Je Probe die Schranke, die der Test öffnet, in der Reihenfolge der
  /// Aufrufe.
  final List<Completer<void>> gates = <Completer<void>>[];

  @override
  Future<LlmProbe> probeLlm(
    String endpoint, {
    Duration? timeout,
    String? apiKey,
    bool useStoredKey = false,
    String? server,
  }) async {
    final Completer<void> gate = Completer<void>();
    gates.add(gate);
    final Future<LlmProbe> answer = super.probeLlm(
      endpoint,
      timeout: timeout,
      apiKey: apiKey,
      useStoredKey: useStoredKey,
      server: server,
    );
    await gate.future;
    return answer;
  }
}

/// Lässt Antworten, Folgefragen und das nächste Bild durchlaufen.
Future<void> settle(WidgetTester tester) async {
  for (int i = 0; i < 4; i++) {
    await tester.pump();
  }
}

/// Öffnet den Setup-Abschnitt über [client].
Future<void> openSetup(
  WidgetTester tester,
  FakeDaemonClient client, {
  Future<String?> Function()? chooser,
}) async {
  await pumpApp(
    tester,
    client: client,
    overrides: [
      if (chooser != null) directoryChooserProvider.overrideWithValue(chooser),
    ],
  );
  await pressCtrl(tester, LogicalKeyboardKey.digit6);
  await tester.pump();
}

Finder openStep(SetupCheckKind kind) =>
    find.byKey(Key('setup-open-${kind.name}'));

Finder lineOf(SetupCheckKind kind) =>
    find.byKey(Key('setup-step-${kind.name}'));

String stateOf(WidgetTester tester, SetupCheckKind kind) =>
    tester.widget<Text>(find.byKey(Key('setup-state-${kind.name}'))).data!;

HButton buttonOf(WidgetTester tester, Finder finder) =>
    tester.widget<HButton>(finder);

ProviderContainer containerOf(WidgetTester tester) =>
    ProviderScope.containerOf(tester.element(find.byType(ShellScreen)));

/// The steps drawn open right now.
List<SetupCheckKind> openSteps() => <SetupCheckKind>[
  for (final SetupCheckKind kind in SetupCheckKind.values)
    if (openStep(kind).evaluate().isNotEmpty) kind,
];

void main() {
  group('the owner state of 2026-10-23', () {
    testWidgets('the first blocking step is open with its action', (
      WidgetTester tester,
    ) async {
      await openSetup(tester, ownersState());

      // Genau ein Schritt steht offen, und es ist der erste, der den
      // Menschen braucht -- nicht der Ordner, der danach kommt.
      expect(openSteps(), <SetupCheckKind>[SetupCheckKind.llm]);
      expect(stateOf(tester, SetupCheckKind.llm), 'blocks the start');
      expect(stateOf(tester, SetupCheckKind.project), 'blocks the start');

      // Die Überschrift sagt die Lage in Worten, der Befund steht im Schritt
      // und behält Code, Grund und Abhilfe.
      expect(
        find.descendant(
          of: openStep(SetupCheckKind.llm),
          matching: find.text('Your model server wants a valid access key'),
        ),
        findsOneWidget,
      );
      final Finder finding = find.descendant(
        of: openStep(SetupCheckKind.llm),
        matching: find.byType(SetupFinding),
      );
      expect(finding, findsOneWidget);
      expect(
        find.descendant(of: finding, matching: find.text('LLM_002')),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: finding,
          matching: find.text(
            'the LLM server answered 401 at '
            'https://llm.example.net/api/v1/models',
          ),
        ),
        findsOneWidget,
      );

      // Die Abhilfe ist der Weg ins Einstellungsblatt (HUM-275), und sie ist
      // die eine Primäraktion des Schritts; „Ansprechen" tritt zurück.
      final Finder settings = find.descendant(
        of: finding,
        matching: find.byKey(const Key('fix-open-settings')),
      );
      expect(buttonOf(tester, settings).variant, HButtonVariant.primary);
      expect(
        buttonOf(tester, find.byKey(const Key('setup-llm-probe'))).variant,
        HButtonVariant.secondary,
      );
      expect(
        buttonOf(tester, find.byKey(const Key('setup-start'))).variant,
        HButtonVariant.secondary,
      );
      // Und sie hat den Fokus: Enter tut, was der Schritt verlangt.
      final BuildContext? focused =
          tester.binding.focusManager.primaryFocus?.context;
      expect(focused, isNotNull);
      expect(
        find.descendant(
          of: settings,
          matching: find.byElementPredicate((Element e) => e == focused),
        ),
        findsOneWidget,
      );

      // Neben der Aktion steht, was danach kommt.
      expect(
        tester.widget<Text>(find.byKey(const Key('setup-step-next'))).data,
        'Next: Project folder',
      );

      // Der Ordner steht als eine Zeile dahinter, mit seinem Knopf auf der
      // Zeile (HUM-282) und ohne seinen Befund als zweite Karte.
      expect(
        find.descendant(
          of: lineOf(SetupCheckKind.project),
          matching: find.byKey(const Key('fix-choose-folder')),
        ),
        findsOneWidget,
      );
      expect(find.byType(SetupFinding), findsOneWidget);
      expect(find.byType(HDiagnosticCard), findsNothing);
    });

    testWidgets('nothing needs scrolling at 1280x800', (
      WidgetTester tester,
    ) async {
      await openSetup(tester, ownersState());

      final ScrollableState scrollable = tester.state<ScrollableState>(
        // Die äußerste: die des Bildschirms, nicht die eines Textfeldes.
        find
            .descendant(
              of: find.byType(SetupScreen),
              matching: find.byType(Scrollable),
            )
            .first,
      );
      expect(scrollable.position.pixels, 0);
      // Die Unterkante der Ansicht ist die Unterkante des Setups, also über
      // der Statusleiste.
      final double bottom = tester.getRect(find.byType(SetupScreen)).bottom;
      for (final Finder finder in <Finder>[
        find.byKey(const Key('fix-open-settings')),
        find.byKey(const Key('setup-step-next')),
        lineOf(SetupCheckKind.project),
        lineOf(SetupCheckKind.sandbox),
      ]) {
        final Rect rect = tester.getRect(finder);
        expect(
          rect.bottom,
          lessThanOrEqualTo(bottom),
          reason: '$finder ends at ${rect.bottom}, the view at $bottom',
        );
      }
    });
  });

  testWidgets('a green step is one line, and it opens only when asked', (
    WidgetTester tester,
  ) async {
    await openSetup(tester, ownersState());

    // Dienst und Maschine sind grün: je eine Zeile mit ihrem Beleg und ohne
    // ihre Bedienelemente.
    for (final SetupCheckKind kind in <SetupCheckKind>[
      SetupCheckKind.daemon,
      SetupCheckKind.sandbox,
    ]) {
      expect(stateOf(tester, kind), 'ok');
      expect(openStep(kind), findsNothing, reason: '$kind');
      expect(
        find.descendant(
          of: lineOf(kind),
          matching: find.byKey(Key('setup-evidence-${kind.name}')),
        ),
        findsOneWidget,
        reason: '$kind',
      );
    }
    expect(find.byKey(const Key('setup-sandbox-recheck')), findsNothing);

    // Wer mehr sehen will, öffnet ihn; und er faltet sich wieder.
    await tester.tap(find.byKey(const Key('setup-step-toggle-sandbox')));
    await tester.pump();
    expect(openStep(SetupCheckKind.sandbox), findsOneWidget);
    expect(find.byKey(const Key('setup-sandbox-recheck')), findsOneWidget);
    // Ein geöffneter grüner Schritt ist nicht der vordere: Seine Aktion ist
    // kein Primärknopf, und er hat keine Überschrift.
    expect(
      buttonOf(tester, find.byKey(const Key('setup-sandbox-recheck'))).variant,
      HButtonVariant.secondary,
    );
    expect(find.byKey(const Key('setup-headline-sandbox')), findsNothing);

    await tester.tap(find.byKey(const Key('setup-step-toggle-sandbox')));
    await tester.pump();
    expect(openStep(SetupCheckKind.sandbox), findsNothing);
  });

  testWidgets('a step that turns green folds and hands the front on', (
    WidgetTester tester,
  ) async {
    const String folder = '/home/u/projects/app';
    await openSetup(tester, withoutFolder(), chooser: () async => folder);

    // Ohne Ordner ist der Ordner vorn, mit seinem Knopf als Primäraktion.
    expect(openSteps(), <SetupCheckKind>[SetupCheckKind.project]);
    expect(
      find.descendant(
        of: openStep(SetupCheckKind.project),
        matching: find.text('Choose the folder the agent works in'),
      ),
      findsOneWidget,
    );
    final Finder choose = find.byKey(const Key('fix-choose-folder'));
    expect(buttonOf(tester, choose).variant, HButtonVariant.primary);
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-step-next'))).data,
      'Next: start a session',
    );

    await tester.tap(choose);
    await tester.pump();
    await tester.pump();

    // Grün: eine Zeile mit dem Ordner als Beleg, und kein Schritt mehr vorn.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(openSteps(), isEmpty);
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-evidence-project'))).data,
      folder,
    );
    expect(find.byType(WorkDirPicker), findsNothing);
  });

  testWidgets('everything set shows Ready, four green lines and no cards', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);

    // Ohne Klick: Das Setup hat den gespeicherten Server selbst angesprochen,
    // genau einmal, und nichts sonst.
    expect(client.probedEndpoints, <String>[home]);
    expect(client.discoverCalls, isEmpty);

    expect(
      tester.widget<Text>(find.byKey(const Key('setup-title'))).data,
      'Humanitl is ready',
    );
    for (final SetupCheckKind kind in SetupCheckKind.values) {
      expect(stateOf(tester, kind), 'ok', reason: '$kind');
      expect(lineOf(kind), findsOneWidget, reason: '$kind');
    }
    expect(openSteps(), isEmpty);
    expect(find.byType(SetupFinding), findsNothing);
    expect(find.byType(HDiagnosticCard), findsNothing);

    // Genau eine Primäraktion: der Start einer Session.
    final HButton start = buttonOf(
      tester,
      find.byKey(const Key('setup-start')),
    );
    expect(start.enabled, isTrue);
    expect(start.variant, HButtonVariant.primary);
    expect(find.text('Start a session'), findsOneWidget);
    final Iterable<HButton> primaries = tester
        .widgetList<HButton>(
          find.descendant(
            of: find.byType(SetupScreen),
            matching: find.byType(HButton),
          ),
        )
        .where((HButton b) => b.variant == HButtonVariant.primary);
    expect(primaries.length, 1);
  });

  testWidgets('the saved server is contacted when nothing is known about it '
      'and on Check again, and only then', (WidgetTester tester) async {
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);

    // Kein Takt: Die Zeit allein spricht niemanden an.
    await tester.pump(const Duration(seconds: 30));
    expect(client.probedEndpoints, <String>[home]);

    // Eine neue Öffnung fragt nur, wenn über den Server noch nichts bekannt
    // ist; hier steht seine Messung schon.
    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);

    // „Check again" fragt die Maschine und den Server noch einmal.
    await openSetupStep(tester, 'sandbox');
    await tester.tap(find.byKey(const Key('setup-sandbox-recheck')));
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, home]);
    expect(client.discoverCalls, isEmpty);
  });

  testWidgets('a switch in Settings is measured, not the server before it', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');

    // Der Wechsel geht den Weg der Einstellungen und des Schnellwechsels der
    // Statusleiste (HUM-289), nicht über einen Schnappschuss der Sandbox.
    await containerOf(tester).read(llmServersProvider.notifier).activate('lab');
    await settle(tester);

    expect(client.probedEndpoints, <String>[home, lab]);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');
    await openSetupStep(tester, 'llm');
    expect(find.text('Saved server: lab'), findsOneWidget);
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-evidence-llm'))).data,
      contains(lab),
    );
  });

  testWidgets('a key changed in Settings makes the step measure again', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);

    // Ein neuer Schlüssel unter derselben Referenz: Die alte Messung galt dem
    // alten Schlüssel.
    await containerOf(tester)
        .read(llmServersProvider.notifier)
        .storeKey('home', 'a-new-key');
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, home]);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');

    // Und ohne Schlüssel ebenso.
    await containerOf(tester)
        .read(llmKeyProvider.notifier)
        .clear(server: 'home');
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, home, home]);
  });

  testWidgets('a server switched during a probe is contacted at once', (
    WidgetTester tester,
  ) async {
    final StallingProbeClient client = StallingProbeClient();
    await openSetup(tester, client);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);
    expect(stateOf(tester, SetupCheckKind.llm), 'asking');

    // Mitten in der Probe wird in den Einstellungen gewechselt: Der neue
    // Server wird sofort angesprochen, die alte Probe ist überholt.
    await containerOf(tester).read(llmServersProvider.notifier).activate('lab');
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, lab]);

    // Die alte Antwort kommt spät und ändert nichts mehr.
    client.gates.first.complete();
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), 'asking');

    client.gates.last.complete();
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-evidence-llm'))).data,
      contains(lab),
    );
  });

  testWidgets('a server removed while its probe runs is not shown green', (
    WidgetTester tester,
  ) async {
    final StallingProbeClient client = StallingProbeClient();
    await openSetup(tester, client);
    await settle(tester);
    expect(client.probedEndpoints, hasLength(1));

    // Die Konfiguration nennt keinen Endpunkt mehr.
    client.configValues = ConfigValues.empty;
    await containerOf(tester).read(configProvider.notifier).refresh();
    await settle(tester);
    client.gates.first.complete();
    await settle(tester);

    // Die Antwort kam, aber über einen Server, der nicht mehr gespeichert
    // ist: Für den Schritt ist nichts gemessen, und niemand wird angesprochen.
    expect(stateOf(tester, SetupCheckKind.llm), 'not measured');
    expect(client.probedEndpoints, hasLength(1));
  });

  testWidgets('a result somebody asked for stays when Setup opens again', (
    WidgetTester tester,
  ) async {
    const String typed = 'http://192.168.1.10:11434';
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);

    await openSetupStep(tester, 'llm');
    await tester.enterText(find.byKey(const Key('setup-llm-endpoint')), typed);
    await tester.tap(find.byKey(const Key('setup-llm-probe')));
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, typed]);

    // Wieder geöffnet, steht neben der getippten Adresse ihre eigene Messung:
    // Das Setup spricht den gespeicherten Server nicht über sie hinweg an.
    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, typed]);
    await openSetupStep(tester, 'llm');
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-evidence-llm'))).data,
      contains(typed),
    );
  });

  testWidgets('a typed address is never contacted by itself, the saved one '
      'is when the field returns to it', (WidgetTester tester) async {
    const String typed = 'http://192.168.1.10:11434';
    final FakeDaemonClient client = everythingSet();
    await openSetup(tester, client);
    await settle(tester);

    // Jemand tippt eine andere Adresse und fragt nicht.
    await openSetupStep(tester, 'llm');
    await tester.enterText(find.byKey(const Key('setup-llm-endpoint')), typed);
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), 'not measured');

    // In den Einstellungen wird gewechselt, und Setup öffnet wieder: Die
    // getippte Adresse bleibt im Feld, niemand wird von selbst angesprochen,
    // und keine Messung eines anderen Servers steht neben ihr.
    await pressCtrl(tester, LogicalKeyboardKey.digit1);
    await containerOf(tester).read(llmServersProvider.notifier).activate('lab');
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);
    expect(stateOf(tester, SetupCheckKind.llm), 'not measured');

    // Steht im Feld wieder genau die gespeicherte Adresse, gilt die Regel:
    // Über sie ist nichts bekannt, also wird sie angesprochen.
    await openSetupStep(tester, 'llm');
    await tester.enterText(find.byKey(const Key('setup-llm-endpoint')), lab);
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, lab]);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');
  });

  testWidgets(
    'a late answer never replaces the address taken from the search',
    (WidgetTester tester) async {
      final StallingProbeClient client = StallingProbeClient()
        ..llmServers = <LlmServer>[
          const LlmServer(
            host: '192.168.2.20',
            port: 11434,
            flavor: LlmFlavor.ollama,
            models: <String>['qwen2.5-coder:14b'],
          ),
        ];
      const String found = 'http://192.168.2.20:11434';
      await openSetup(tester, client);
      await settle(tester);
      expect(client.probedEndpoints, <String>[home]);

      // Während der gespeicherte Server noch gefragt wird, nimmt jemand eine
      // Adresse aus der Suche.
      await tester.tap(find.byKey(const Key('setup-llm-discover')));
      await tester.pump();
      await tester.tap(find.byKey(const Key('setup-discover-start')));
      await tester.pump();
      await tester.pump(const Duration(seconds: 1));
      await tester.pump(const Duration(milliseconds: 400));
      await tester.tap(
        find.byKey(const Key('setup-discover-use-192.168.2.20-11434')),
      );
      await settle(tester);
      expect(client.probedEndpoints, <String>[home, found]);

      // Die Suche antwortet zuerst, der gespeicherte Server danach: Seine späte
      // Antwort ersetzt die neuere nicht.
      client.gates.last.complete();
      await settle(tester);
      client.gates.first.complete();
      await settle(tester);
      expect(stateOf(tester, SetupCheckKind.llm), 'ok');
      expect(
        tester.widget<Text>(find.byKey(const Key('setup-evidence-llm'))).data,
        contains(found),
      );
    },
  );

  testWidgets('after a lost connection the saved server is measured anew', (
    WidgetTester tester,
  ) async {
    final StallingProbeClient client = StallingProbeClient();
    await pumpApp(
      tester,
      client: client,
      heartbeat: const Duration(seconds: 1),
    );
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await settle(tester);
    client.gates.first.complete();
    await settle(tester);
    expect(client.probedEndpoints, <String>[home]);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');

    // Der Daemon fällt weg: Die Messung von vorher gilt nicht mehr.
    client.goOffline();
    await tester.pump(const Duration(seconds: 1));
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), isNot('ok'));

    // Er kommt zurück: genau eine neue Probe, und bereit ist der Schritt erst,
    // wenn sie geantwortet hat.
    client.goOnline();
    await tester.tap(find.byKey(const Key('shell-frozen-reconnect')));
    await settle(tester);
    expect(client.probedEndpoints, <String>[home, home]);
    expect(stateOf(tester, SetupCheckKind.llm), 'asking');
    expect(
      tester.widget<Text>(find.byKey(const Key('setup-title'))).data,
      isNot('Humanitl is ready'),
    );

    client.gates.last.complete();
    await settle(tester);
    expect(stateOf(tester, SetupCheckKind.llm), 'ok');
    expect(client.probedEndpoints, <String>[home, home]);
  });

  testWidgets('a step opened by hand folds when it turns green', (
    WidgetTester tester,
  ) async {
    const String folder = '/home/u/projects/app';
    await openSetup(tester, ownersState(), chooser: () async => folder);

    // Der Ordner steht hinter dem Modellschritt; jemand öffnet ihn von Hand
    // und wählt dort den Ordner.
    await openSetupStep(tester, 'project');
    expect(openStep(SetupCheckKind.project), findsOneWidget);
    final Finder choose = find.descendant(
      of: openStep(SetupCheckKind.project),
      matching: find.byKey(const Key('fix-choose-folder')),
    );
    await tester.ensureVisible(choose);
    await tester.pump();
    await tester.tap(choose);
    await tester.pump();
    await tester.pump();

    // Grün: wieder eine Zeile, und von Hand lässt er sich weiter öffnen.
    expect(stateOf(tester, SetupCheckKind.project), 'ok');
    expect(openStep(SetupCheckKind.project), findsNothing);
    await openSetupStep(tester, 'project');
    expect(openStep(SetupCheckKind.project), findsOneWidget);
  });

  testWidgets('with a session running the one action leads to the queue', (
    WidgetTester tester,
  ) async {
    await openSetup(tester, everythingSet());
    await settle(tester);
    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();
    expect(containerOf(tester).read(navigationProvider), Section.intercept);

    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pumpAndSettle();
    expect(
      buttonOf(tester, find.byKey(const Key('setup-start'))).enabled,
      isFalse,
    );
    final Finder toQueue = find.byKey(const Key('setup-open-intercept'));
    expect(buttonOf(tester, toQueue).variant, HButtonVariant.primary);

    await tester.tap(toQueue);
    await tester.pump();
    expect(containerOf(tester).read(navigationProvider), Section.intercept);
  });

  group('the step in front', () {
    SetupState stateWith(Map<SetupCheckKind, SetupCheckState> states) =>
        SetupState(
          checks: <SetupCheck>[
            for (final SetupCheckKind kind in SetupCheckKind.values)
              SetupCheck(kind: kind, state: states[kind] ?? SetupCheckState.ok),
          ],
        );

    test('is the first step that is not green, in the order of the work', () {
      final SetupState state = stateWith(<SetupCheckKind, SetupCheckState>{
        SetupCheckKind.llm: SetupCheckState.unmeasured,
        SetupCheckKind.project: SetupCheckState.failed,
      });
      // Nicht der gravere: der erste. Ohne Modell startet auch ein Ordner
      // nichts, und die Reihenfolge ist die der Arbeit.
      expect(setupCurrentStep(state), SetupCheckKind.llm);
      expect(setupStepAfter(state, SetupCheckKind.llm), SetupCheckKind.project);
      expect(setupStepAfter(state, SetupCheckKind.project), isNull);
    });

    test('is none when all four are green', () {
      expect(setupCurrentStep(stateWith(const {})), isNull);
    });

    test('a warning and a step being asked are in front too', () {
      expect(
        setupCurrentStep(
          stateWith(<SetupCheckKind, SetupCheckState>{
            SetupCheckKind.sandbox: SetupCheckState.warn,
          }),
        ),
        SetupCheckKind.sandbox,
      );
      expect(
        setupCurrentStep(
          stateWith(<SetupCheckKind, SetupCheckState>{
            SetupCheckKind.daemon: SetupCheckState.checking,
          }),
        ),
        SetupCheckKind.daemon,
      );
    });
  });

  group('where the app opens', () {
    // Spezifikation 4: Setup beim ersten Start und immer, wenn ein Schritt
    // den Start sperrt, sonst die Warteschlange. Ein erster Start hat keinen
    // Ordner, und ein fehlender Ordner sperrt (`CONFIG_013`); der erste Start
    // ist damit der erste Fall des zweiten.
    testWidgets('a first start without a folder opens Setup', (
      WidgetTester tester,
    ) async {
      await pumpApp(tester, client: withoutFolder());
      await tester.pumpAndSettle();
      expect(containerOf(tester).read(navigationProvider), Section.setup);
    });

    testWidgets('the owner state opens Setup', (WidgetTester tester) async {
      await pumpApp(tester, client: ownersState());
      await tester.pumpAndSettle();
      expect(containerOf(tester).read(navigationProvider), Section.setup);
    });

    testWidgets('nothing blocking opens Intercept', (
      WidgetTester tester,
    ) async {
      // Der gewöhnliche zweite Start: Die Maschine ist gemessen, der
      // Modellserver wurde nicht angesprochen. Nichts sperrt, also beginnt
      // die Arbeit in der Warteschlange, auch wenn der Start noch auf das
      // Ansprechen wartet.
      await pumpApp(tester, client: FakeDaemonClient.empty());
      await tester.pumpAndSettle();
      expect(containerOf(tester).read(navigationProvider), Section.intercept);
    });
  });
}
