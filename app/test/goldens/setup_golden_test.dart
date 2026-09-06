// Goldens des Setup-Bildschirms bei 1280×800, dunkel und hell (HUM-044).
// Erneuern mit `flutter test --update-goldens test/goldens`.
//
// Drei Lagen, weil sie drei verschiedene Aussagen sind und verschieden
// aussehen muessen:
//
//  * `setup_blocked`: kein Daemon. Der Bildschirm steht allein, die erste
//    Zeile traegt den Befund mit seinem Vorschlag, und der Start-Knopf ist aus.
//  * `setup_unmeasured`: ein Daemon, aber eine Maschine, die niemand lesen
//    konnte. Jede Zeile traegt einen Ring statt einer Scheibe, und der Knopf
//    ist aus -- mit dem Satz darunter, der sagt, wie viele Zeilen ungemessen
//    sind. Bis zum 2026-09-06 war er hier an, weil `canStart` nur `failed` und
//    `checking` sperrte; die Spezifikation verlangt alle vier Zeilen gruen
//    (HUM-044, Kriterium `start_button_enabled_only_when_all_ok`). Im Bild ist
//    davon nichts zu sehen: Knopf und Satz liegen bei 1280x800 unter der
//    Falz.
//  * `setup_ready`: alles gemessen und in Ordnung.

import 'package:alchemist/alchemist.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/providers/theme.dart';
import 'package:humanitl/features/setup/providers/discover_provider.dart';
import 'package:humanitl/features/shell/section.dart';

import '../harness/ui_state.dart';

/// Die App über [client], im Theme [mode].
Widget setup(HThemeMode mode, FakeDaemonClient client) => ProviderScope(
  overrides: <Override>[
    uiStateOverride(),
    daemonClientProvider.overrideWithValue(client),
    connectionHeartbeatProvider.overrideWithValue(null),
    connectionReconnectProvider.overrideWithValue(null),
    themeModeProvider.overrideWith(() => _FixedTheme(mode)),
    navigationProvider.overrideWith(() => _FixedSection(Section.setup)),
  ],
  child: const HumanitlApp(),
);

class _FixedTheme extends ThemeModeSetting {
  _FixedTheme(this.mode);

  final HThemeMode mode;

  @override
  HThemeMode build() => mode;
}

/// Ein Abschnitt, der stehen bleibt: die Weiche der Shell darf im Golden nicht
/// umschalten, sonst haengt das Bild an einer Zeitscheibe.
/// Ein Suchergebnis, das im Golden feststeht.
class _FoundServers extends SetupLlmDiscover {
  @override
  LlmDiscoverState build() => const LlmDiscoverState(
    finished: true,
    servers: <LlmServer>[
      LlmServer(
        host: '192.168.2.20',
        port: 11434,
        flavor: LlmFlavor.ollama,
        models: <String>['qwen2.5-coder:14b', 'llama3.1:8b'],
        latencyMs: 12,
      ),
      LlmServer(
        host: '192.168.2.40',
        port: 8000,
        authRequired: true,
        latencyMs: 31,
      ),
      LlmServer(host: '192.168.2.60', port: 8080, latencyMs: 8),
    ],
  );
}

class _FixedSection extends Navigation {
  _FixedSection(this.section);

  final Section section;

  @override
  Section build() => section;
}

void main() {
  const BoxConstraints window = BoxConstraints.tightFor(
    width: 1280,
    height: 800,
  );

  /// Die Vorgabe des Fakes: elf Zeilen, keine davon gemessen.
  FakeDaemonClient unmeasured() => FakeDaemonClient();

  FakeDaemonClient ready() => FakeDaemonClient()..doctorReport = fakeDoctorOk();

  for (final HThemeMode mode in HThemeMode.values) {
    if (mode == HThemeMode.system) {
      continue;
    }
    final String name = mode.name;

    goldenTest(
      'setup_blocked_$name',
      fileName: 'setup_blocked_$name',
      constraints: window,
      builder: () => setup(mode, FakeDaemonClient.unavailable()),
    );

    goldenTest(
      'setup_unmeasured_$name',
      fileName: 'setup_unmeasured_$name',
      constraints: window,
      builder: () => setup(mode, unmeasured()),
    );

    goldenTest(
      'setup_ready_$name',
      fileName: 'setup_ready_$name',
      constraints: window,
      builder: () => setup(mode, ready()),
    );
  }

  /// Das Such-Blatt mit drei Funden: der eine, der antwortet, der hinter einer
  /// Anmeldung und der, der sich nicht ausgewiesen hat. Es ist das einzige
  /// Bild dieses Bildschirms, in dem ein Vorgang steht, den ein Mensch
  /// ausgelöst hat, und es hält fest, wie die drei Fälle nebeneinander
  /// aussehen (HUM-076).
  ///
  /// Der Zustand wird gesetzt und nicht erlaufen: Ein Golden, das erst tippt
  /// und dann auf einen Strom wartet, hängt an einer Zeitscheibe.
  for (final HThemeMode mode in <HThemeMode>[
    HThemeMode.dark,
    HThemeMode.light,
  ]) {
    goldenTest(
      'setup_discover_${mode.name}',
      fileName: 'setup_discover_${mode.name}',
      constraints: window,
      pumpBeforeTest: (WidgetTester tester) async {
        await tester.pump();
        await tester.tap(find.byKey(const Key('setup-llm-discover')));
        await tester.pump();
        // Über das Einfliegen der Zeilen hinaus: Ein Bild bei t=0 zeigt drei
        // vollständig durchsichtige Zeilen, also nichts.
        await tester.pump(const Duration(milliseconds: 300));
      },
      builder: () => ProviderScope(
        overrides: <Override>[
          uiStateOverride(),
          daemonClientProvider.overrideWithValue(
            FakeDaemonClient()..doctorReport = fakeDoctorOk(),
          ),
          connectionHeartbeatProvider.overrideWithValue(null),
          connectionReconnectProvider.overrideWithValue(null),
          themeModeProvider.overrideWith(() => _FixedTheme(mode)),
          navigationProvider.overrideWith(() => _FixedSection(Section.setup)),
          setupLlmDiscoverProvider.overrideWith(_FoundServers.new),
        ],
        child: const HumanitlApp(),
      ),
    );
  }

  testWidgets('the three goldens really are three different screens', (
    WidgetTester tester,
  ) async {
    // Ein Golden beweist nur, dass sich nichts geaendert hat. Dass die drei
    // Lagen ueberhaupt verschieden sind, steht hier.
    await tester.binding.setSurfaceSize(const Size(1280, 800));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    for (final (FakeDaemonClient client, String row, String word)
        in <(FakeDaemonClient, String, String)>[
          // Ohne Daemon sperrt die erste Zeile, und die vierte hat niemand
          // gelesen; mit Daemon entscheidet die vierte.
          (FakeDaemonClient.unavailable(), 'daemon', 'blocks the start'),
          (unmeasured(), 'sandbox', 'not measured'),
          (ready(), 'sandbox', 'ok'),
        ]) {
      await tester.pumpWidget(setup(HThemeMode.dark, client));
      await tester.pump();
      await tester.pump();
      expect(
        tester.widget<Text>(find.byKey(Key('setup-state-$row'))).data,
        word,
        reason: word,
      );
      await tester.pumpWidget(const SizedBox.shrink());
    }
  });
}
