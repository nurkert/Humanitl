// Der Hinweis des Schreibtischs (HUM-034, seit HUM-276 ein Banner, seit
// HUM-302 eine schwebende Karte): welche Codes er überhaupt nennen darf, ob
// seine Schrift auf seiner Fläche lesbar bleibt, ob er bei doppelter
// Textskalierung wächst statt abzuschneiden (`docs/UX.md` 6), und wo er
// steht: oben rechts über den Bereichen, unter der Kopfzeile der Panes, ohne
// Leiste oder Queue auch nur um ein Pixel zu verschieben.

import 'dart:io' show File;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/h_notice_layer.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/agent_asks.dart';
import 'package:humanitl/features/intercept/widgets/agent_ask_card.dart';
import 'package:humanitl/features/intercept/widgets/queue_pane.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';
import 'package:humanitl/features/shell/providers/rail.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

/// Das Register, in dem jeder Code stehen muss (CONVENTIONS 4.6).
const String registerPath =
    '../daemon/crates/core-types/src/diagnostics/codes.rs';

/// Die Datei, in der die App ihre Codes hält.
const String codesPath = 'lib/core/domain/diagnostic_codes.dart';

/// Die Datei, die die Diagnosen dieses Features baut.
const String trayCodesPath = 'lib/features/tray/tray_diagnostics.dart';

/// Der Kontrast von [foreground] über [layers] auf [base].
double contrastOn(Color foreground, List<Color> layers, Color base) {
  Color surface = base;
  for (final Color layer in layers) {
    surface = HColorDerivation.flatten(layer, surface);
  }
  return HColorDerivation.contrast(
    HColorDerivation.flatten(foreground, surface),
    surface,
  );
}

/// Das Banner in einem Fenster von [width], mit [textScaler].
///
/// Der Provider-Bereich gehoert dazu, seit `FixControl` seinen Installer aus
/// `serviceInstallerProvider` liest (HUM-044): Das Banner baut immer eines,
/// und ohne Bereich haette ein Klick auf „Dienst installieren" hier keinen,
/// den er lesen koennte. Die Anwendung hat ihren ueber `main.dart`.
Widget banner({
  required Diagnostic diagnostic,
  double width = 900,
  TextScaler textScaler = TextScaler.noScaling,
}) => ProviderScope(
  child: WidgetsApp(
    color: HColors.bg0,
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    builder: (BuildContext context, Widget? _) => MediaQuery(
      data: MediaQueryData(textScaler: textScaler),
      child: HTheme(
        tokens: HTokens.dark,
        child: Align(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: width,
            child: AttentionNoticeBanner(
              diagnostic: diagnostic,
              onDismiss: () {},
            ),
          ),
        ),
      ),
    ),
  ),
);

/// Ob der Fokus gerade auf der Schliessen-Taste des Banners liegt.
bool dismissFocused() {
  final BuildContext? context = FocusManager.instance.primaryFocus?.context;
  return context?.findAncestorWidgetOfExactType<HIconButton>()?.key ==
      const Key('attention-notice-dismiss');
}

void main() {
  group('the codes the banner may name', () {
    test('every_one_of_them_is_in_the_register', () {
      final String register = File(registerPath).readAsStringSync();
      for (final String code in <String>[
        DiagnosticCodes.noTray,
        DiagnosticCodes.flowNotHeld,
        DiagnosticCodes.decideRequestInvalid,
      ]) {
        expect(
          register,
          contains(RegExp('^\\s*$code =>', multiLine: true)),
          reason: '$code steht nicht in $registerPath',
        );
        expect(
          File(codesPath).readAsStringSync(),
          contains("'$code'"),
          reason: '$code steht nicht in $codesPath',
        );
      }
    });

    test('the_tray_declares_none_of_its_own', () {
      // `diagnostic_codes.dart` sagt in seinem eigenen Kommentar, jede
      // Konstante dort sei ein registrierter Code und die App erfinde keinen.
      // Eine zweite Stelle mit einem eigenen Code-Literal hebelt das aus.
      expect(
        File(trayCodesPath).readAsStringSync(),
        isNot(contains(RegExp("'[A-Z]+_[0-9]{3}'"))),
        reason: 'Codes stehen in $codesPath, nicht hier',
      );
      expect(
        TrayDiagnostics.trayUnavailable('no watcher').code,
        DiagnosticCodes.noTray,
      );
      expect(
        TrayDiagnostics.findingsNeedTheWindow(
          const FlowId('018f0034-0000-7000-8000-000000000001'),
          2,
        ).code,
        DiagnosticCodes.decideRequestInvalid,
      );
    });

    test('the_documentation_knows_the_tray_code', () {
      expect(
        File('../docs/DIAGNOSTICS.md').readAsStringSync(),
        contains('#### ${DiagnosticCodes.noTray}'),
      );
    });
  });

  for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
    final String theme = tokens.brightness.name;

    test('$theme: everything written on the notice reaches 4.5:1', () {
      // Die Karte liegt seit HUM-302 auf `bg2`, wie jede Karte, und darauf
      // steht jede ihrer Zeilen, auch die Kopierzeile des Vorschlags. Der
      // Schweregrad trägt nur noch Schiene und Badges.
      for (final Color text in <Color>[tokens.colors.fg0, tokens.colors.fg1]) {
        expect(
          contrastOn(text, const <Color>[], tokens.colors.bg2),
          greaterThanOrEqualTo(4.5),
          reason: 'Text $text',
        );
      }
      for (final Color state in <Color>[
        tokens.colors.info,
        tokens.colors.warning,
        tokens.state.error,
      ]) {
        // Die Beschriftung der Badges steht auf der Tönung derselben Farbe,
        // die ihrerseits auf der Karte liegt: ein Wort, 4,5:1.
        expect(
          contrastOn(tokens.stateTextOf(state), <Color>[
            HColorDerivation.tint(state),
          ], tokens.colors.bg2),
          greaterThanOrEqualTo(4.5),
          reason: 'Badge $state',
        );
      }
    });
  }

  testWidgets('it_grows_with_the_text_instead_of_clipping_it', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 1400));
    addTearDown(() => tester.binding.setSurfaceSize(null));
    final Diagnostic diagnostic = TrayDiagnostics.trayUnavailable(
      'no org.kde.StatusNotifierWatcher answers on the session bus',
    );

    await tester.pumpWidget(banner(diagnostic: diagnostic));
    final double plain = tester
        .getSize(find.byType(AttentionNoticeBanner))
        .height;

    await tester.pumpWidget(
      banner(diagnostic: diagnostic, textScaler: const TextScaler.linear(2)),
    );

    // Kein Overflow, und das Banner ist wirklich gewachsen: eine feste Höhe
    // schluckte den Überlauf still (`docs/UX.md` 6).
    expect(tester.takeException(), isNull);
    expect(
      tester.getSize(find.byType(AttentionNoticeBanner)).height,
      greaterThan(plain),
    );
    expect(find.text('This desktop has no tray'), findsOneWidget);
  });

  testWidgets('the_narrow_shell_does_not_overflow_at_double_scale', (
    WidgetTester tester,
  ) async {
    await tester.binding.setSurfaceSize(const Size(1100, 1400));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    // 1100 px ist die schmalste Fensterbreite, die die App zulaesst
    // (`windowMinimumSize`), und das Banner steht darin mit seiner
    // Schliessen-Taste.
    for (final Diagnostic diagnostic in <Diagnostic>[
      TrayDiagnostics.decidedAlready(
        const FlowId('018f0034-0000-7000-8000-000000000001'),
      ),
      TrayDiagnostics.trayUnavailable('no watcher on the session bus'),
    ]) {
      await tester.pumpWidget(
        banner(
          diagnostic: diagnostic,
          width: 1100,
          textScaler: const TextScaler.linear(2),
        ),
      );
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('another_code_keeps_the_daemon_sentence_in_the_cause_slot', (
    WidgetTester tester,
  ) async {
    // HUM-052: Die Überschrift kommt aus ARB, der gemessene Satz des Daemons
    // bleibt der Grund (`docs/UX.md` 4.4) und steht nicht ein zweites Mal
    // als Detail darunter.
    final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
    const Diagnostic diagnostic = Diagnostic(
      code: 'CONFIG_001',
      severity: Severity.error,
      title: 'Config-Datei ungültig',
      why: '/home/a/.config/humanitl/config.toml: expected `=` at line 3',
    );
    await tester.pumpWidget(banner(diagnostic: diagnostic));
    await tester.pump();

    expect(find.text(en.diagCONFIG001Title), findsOneWidget);
    // Genau einmal: im Grund-Platz, nicht noch einmal als Detail darunter.
    expect(find.text(diagnostic.why), findsOneWidget);
  });

  // Die beiden Breiten der Seitenleiste (HUM-254): Der Hinweis steht in
  // beiden gleich, oben rechts über den Bereichen, und schiebt weder Leiste
  // noch Queue (HUM-302).
  for (final bool expanded in <bool>[false, true]) {
    final String width = expanded ? 'expanded' : 'collapsed';

    testWidgets('attention_notice_test: floats top right, rail $width', (
      WidgetTester tester,
    ) async {
      const Size window = Size(1400, 900);
      await pumpTrayShell(
        tester,
        desktop: FakeDesktop(
          missing: TrayDiagnostics.trayUnavailable('no watcher'),
        ),
        uiState: trayUiState(
          initial: <String, bool>{railExpandedKey: expanded},
        ),
        size: window,
      );
      expect(tester.takeException(), isNull);

      final Finder notice = find.byType(AttentionNoticeBanner);
      expect(notice, findsOneWidget);
      final Rect header = tester.getRect(find.byType(HeaderBar));
      final Rect card = tester.getRect(notice);
      final Rect rail = tester.getRect(find.byType(IconRail));
      final Rect queue = tester.getRect(find.byType(QueuePane));

      // Oben rechts, unter der Kopfzeile der Panes, höchstens 420 px breit.
      expect(card.right, window.width - HSpace.x2);
      expect(card.top, header.bottom + HSize.paneHeader + HSpace.x2);
      expect(card.width, noticeMaxWidth);
      // Leiste und Queue beginnen direkt unter der Kopfzeile: Der Hinweis
      // nimmt ihnen keine Höhe, und er steht in keiner ihrer Spalten.
      expect(rail.top, header.bottom);
      expect(queue.top, header.bottom);
      expect(
        find.descendant(of: find.byType(QueuePane), matching: notice),
        findsNothing,
      );
      // Das Schließen steht in der Karte, oben rechts.
      final Rect dismiss = tester.getRect(
        find.byKey(const Key('attention-notice-dismiss')),
      );
      expect(card.contains(dismiss.topLeft), isTrue);
      expect(dismiss.right, card.right - HSpace.x1);
      expect(dismiss.top, card.top + HSpace.x1);

      await tester.tap(find.byKey(const Key('attention-notice-dismiss')));
      await tester.pump();
      expect(notice, findsNothing);
      // Nach dem Schließen steht alles, wo es stand.
      expect(tester.getRect(find.byType(QueuePane)), queue);
      expect(tester.getRect(find.byType(IconRail)), rail);
    });
  }

  testWidgets('attention_notice_test: the keyboard reaches Dismiss, Esc not', (
    WidgetTester tester,
  ) async {
    await pumpTrayShell(
      tester,
      desktop: FakeDesktop(
        missing: TrayDiagnostics.trayUnavailable('no watcher'),
      ),
      uiState: trayUiState(),
    );
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    Future<void> shiftTab() async {
      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.pump();
    }

    // Der Fokus steht beim Start im Intercept-Bildschirm. Seit HUM-302
    // schwebt der Hinweis über den Panes, oben rechts, und steht in der
    // Lesefolge hinter ihren Nähten: `Tab` erreicht ihn nach den Splittern.
    // Er nimmt den Fokus nie von sich aus.
    for (int i = 0; i < 40 && !dismissFocused(); i++) {
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    expect(dismissFocused(), isTrue);

    // In der Lesefolge steht die Taste oben rechts im Banner und damit vor
    // dem Vorschlag darunter: ein `Tab` führt zum Kopierknopf, ein
    // `Shift+Tab` wieder zu ihr.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    expect(
      FocusManager.instance.primaryFocus?.context
          ?.findAncestorWidgetOfExactType<HButton>()
          ?.key,
      const Key('setup-fix-copy'),
    );
    await shiftTab();
    expect(dismissFocused(), isTrue);

    // `Esc` schliesst nichts aus Versehen.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pump();
    expect(find.byType(AttentionNoticeBanner), findsNothing);
  });

  testWidgets('attention_notice_test: waits while the settings sheet is open', (
    WidgetTester tester,
  ) async {
    // Das Blatt hält die Tastatur und steht rechts, wo der Hinweis schwebt;
    // über seinem Inhalt hätte er nichts zu suchen (Review).
    await pumpTrayShell(
      tester,
      desktop: FakeDesktop(
        missing: TrayDiagnostics.trayUnavailable('no watcher'),
      ),
      uiState: trayUiState(),
    );
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    await tester.tap(find.byKey(const Key('header-settings-button')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(SettingsSheet), findsOneWidget);
    expect(find.byType(AttentionNoticeBanner), findsNothing);

    // Zu: Der Hinweis ist wieder da, er war nur verdeckt, nicht geschlossen.
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(SettingsSheet), findsNothing);
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);
  });

  testWidgets('attention_notice_test: stands below the pane headers at 2x', (
    WidgetTester tester,
  ) async {
    // Bei doppelter Schrift ist die Kopfzeile eines Panes 96 px hoch, nicht
    // 48; ein fester Abstand legte die Karte über ihre Bedienelemente
    // (Review).
    tester.platformDispatcher.textScaleFactorTestValue = 2;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
    await pumpTrayShell(
      tester,
      desktop: FakeDesktop(
        missing: TrayDiagnostics.trayUnavailable('no watcher'),
      ),
      uiState: trayUiState(),
      size: const Size(1600, 1100),
    );
    tester.takeException();
    final Rect card = tester.getRect(find.byType(AttentionNoticeBanner));
    final Rect queueHeader = tester.getRect(
      find.byKey(const Key('intercept-queue-header')),
    );
    expect(queueHeader.height, greaterThan(HSize.paneHeader * 1.5));
    expect(card.top, greaterThanOrEqualTo(queueHeader.bottom));
  });

  testWidgets('attention_notice_test: waits while the rule sheet is open', (
    WidgetTester tester,
  ) async {
    // Jede Ebene hält die Tastatur und steht über der Arbeit, auch das
    // Regelblatt einer Bitte des Agenten (Review).
    final ProviderContainer container = await pumpTrayShell(
      tester,
      desktop: FakeDesktop(
        missing: TrayDiagnostics.trayUnavailable('no watcher'),
      ),
      uiState: trayUiState(),
    );
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);

    container
        .read(agentAskRuleDraftProvider.notifier)
        .open(
          AgentAsk(
            id: 'ask-1',
            at: DateTime.utc(2026, 10, 30),
            text: 'please allow pypi.org',
            suggestedHost: 'pypi.org',
            suggestedPath: '/simple/',
          ),
        );
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(AgentAskRuleSheet), findsOneWidget);
    expect(find.byType(AttentionNoticeBanner), findsNothing);

    container.read(agentAskRuleDraftProvider.notifier).close();
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(AgentAskRuleSheet), findsNothing);
    expect(find.byType(AttentionNoticeBanner), findsOneWidget);
  });
}
