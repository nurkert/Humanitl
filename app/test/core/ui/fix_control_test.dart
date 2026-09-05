// `FixControl` (HUM-106): Was ein `Diagnostic` als Abhilfe vorschlägt, muss
// die Oberfläche entweder ausführen oder gar nicht anbieten.
//
// `docs/UX.md` 4.4 sagt, ein `Diagnostic` mit `FixAction` und ohne sichtbare
// Aktion sei ein Defekt; `backlog/CONVENTIONS.md` 4.13 sagt, ein Control, das
// etwas verspricht, was nicht geschieht, sei schlimmer als keines. Für
// `SetEnv` liegt genau ein ausführbarer Teil dazwischen: den Befehl kopieren.
// Das Schreiben in die Konfiguration braucht `SetConfig` und kommt mit
// HUM-069.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'shell_command_test.dart' show shellWords;

/// Ein Wirt mit Theme und Sprache, so schmal wie das Control es braucht.
Widget host(Widget child) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  locale: const Locale('en'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  onGenerateTitle: (BuildContext context) => 'fix control',
  builder: (BuildContext context, Widget? _) => HTheme(
    tokens: HTokens.dark,
    child: Align(
      alignment: Alignment.topLeft,
      child: SizedBox(width: 480, child: child),
    ),
  ),
);

/// Fängt ab, was in die Zwischenablage geschrieben wird.
List<String> captureClipboard(WidgetTester tester) {
  final List<String> written = <String>[];
  tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
    SystemChannels.platform,
    (MethodCall call) async {
      if (call.method == 'Clipboard.setData') {
        written.add(
          (call.arguments as Map<Object?, Object?>)['text']! as String,
        );
      }
      return null;
    },
  );
  addTearDown(
    () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
      SystemChannels.platform,
      null,
    ),
  );
  return written;
}

void main() {
  testWidgets('set_env_offers_the_export_command', (WidgetTester tester) async {
    final List<String> clipboard = captureClipboard(tester);
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(
            key: 'CURL_CA_BUNDLE',
            value: '/etc/humanitl/ca.crt',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    // Das Abzeichen benennt weiterhin, was zu tun ist ...
    expect(find.text('Set CURL_CA_BUNDLE'), findsOneWidget);
    // ... und darunter steht der Befehl, den diese Anwendung ausführen kann.
    // Rot, sobald der Zweig wieder nur ein `HBadge` zeichnet.
    expect(
      find.text('export CURL_CA_BUNDLE=/etc/humanitl/ca.crt'),
      findsOneWidget,
    );
    expect(find.text('Copy export command'), findsOneWidget);

    await tester.tap(find.byKey(const Key('setup-fix-copy')));
    await tester.pump();

    expect(clipboard, <String>['export CURL_CA_BUNDLE=/etc/humanitl/ca.crt']);
    // Ein Klick zeigt eine sichtbare Reaktion (`docs/UX.md` 6, Punkt 4).
    expect(find.text('Copied'), findsOneWidget);
    expect(find.text('Copy export command'), findsNothing);

    // Nach dem Rückmeldefenster steht wieder das Angebot da.
    await tester.pump(HMotion.copyFeedback);
    await tester.pumpAndSettle();
    expect(find.text('Copy export command'), findsOneWidget);
  });

  testWidgets('set_env_quotes_a_hostile_value', (WidgetTester tester) async {
    // Der Wert kommt ueber die Leitung. Ungequotet waere `; rm -rf ~` ein
    // zweiter Befehl in der Zwischenablage des Nutzers.
    //
    // Geprueft wird nicht die Schreibweise, sondern was eine Shell daraus
    // liest: genau drei Woerter. Rot, sobald der Zweig wieder interpoliert.
    const String value = "a'; rm -rf ~; '";
    final List<String> clipboard = captureClipboard(tester);
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(key: 'CURL_CA_BUNDLE', value: value),
        ),
      ),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(const Key('setup-fix-copy')));
    await tester.pump();

    expect(clipboard, hasLength(1));
    expect(shellWords(clipboard.single), <String>[
      'export',
      'CURL_CA_BUNDLE=$value',
    ]);
    // Angezeigt und kopiert ist dieselbe Zeichenkette; wer nur eine der
    // beiden saeuberte, haette den Fehler bloss verschoben.
    expect(find.text(clipboard.single), findsOneWidget);

    await tester.pump(HMotion.copyFeedback);
    await tester.pumpAndSettle();
  });

  testWidgets('set_env_with_a_line_break_offers_no_command', (
    WidgetTester tester,
  ) async {
    // Quotieren genuegte hier nicht: Ein Terminal ohne Klammer-Einfuegen
    // schickte die erste Zeile ab. Also kein Knopf, sondern der Grund.
    // Rot, sobald die Weigerung faellt.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(
            key: 'CURL_CA_BUNDLE',
            value: '/etc/ca.crt\nrm -rf ~',
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
    expect(find.byKey(const Key('setup-fix-no-command')), findsOneWidget);
    // Und zwar der Satz zum Zeilenumbruch, nicht der zum Schluessel. Ohne
    // diese Zusicherung blieben beide Faelle gruen, wenn man die Zweige
    // vertauscht.
    expect(find.textContaining('spans more than one line'), findsOneWidget);
    // Das Abzeichen bleibt: Was zu tun ist, steht weiter da.
    expect(find.text('Set CURL_CA_BUNDLE'), findsOneWidget);
  });

  testWidgets('set_env_with_a_bad_key_offers_no_command', (
    WidgetTester tester,
  ) async {
    // Kein Ersatzschluessel, kein Platzhalter: gar kein Befehl.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.setEnv(key: 'CURL;rm -rf ~', value: '/etc/ca.crt'),
        ),
      ),
    );
    await tester.pumpAndSettle();

    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
    expect(find.byKey(const Key('setup-fix-no-command')), findsOneWidget);
    expect(
      find.textContaining('is not a name a shell can assign to'),
      findsOneWidget,
    );
    expect(find.textContaining('spans more than one line'), findsNothing);
    expect(find.textContaining('export'), findsNothing);
  });

  testWidgets('the_copy_button_takes_the_key_it_is_given', (
    WidgetTester tester,
  ) async {
    // Zwei Karten im selben Streifen truegen sonst denselben Schluessel.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.copyCommand(command: 'humanitld'),
          copyKey: Key('own-copy'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('own-copy')), findsOneWidget);
    expect(find.byKey(const Key('setup-fix-copy')), findsNothing);
  });

  testWidgets('no_fix_draws_nothing', (WidgetTester tester) async {
    await tester.pumpWidget(host(const FixControl(fix: null)));
    await tester.pumpAndSettle();
    expect(find.byType(HButton), findsNothing);
    expect(find.byType(HBadge), findsNothing);
  });

  testWidgets('change_setting_stays_a_badge_without_a_button', (
    WidgetTester tester,
  ) async {
    // `SetConfig` ist bis HUM-069 `unimplemented`. Ein Knopf hier wäre ein
    // Versprechen ohne Wirkung; das Abzeichen sagt nur, was zu tun ist.
    // Rot, sobald jemand dieser Aktion einen Knopf gibt, bevor der RPC steht.
    await tester.pumpWidget(
      host(
        const FixControl(
          fix: FixAction.changeSetting(key: 'llm.endpoint', value: 'off'),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.byType(HBadge), findsOneWidget);
    expect(find.byType(HButton), findsNothing);
  });
}
