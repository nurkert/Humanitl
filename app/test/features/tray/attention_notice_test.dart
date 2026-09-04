// Die Hinweiskarte des Schreibtischs (HUM-034): welche Codes sie überhaupt
// nennen darf, ob ihre Schrift auf ihrer eigenen Fläche lesbar bleibt, und ob
// sie bei doppelter Textskalierung wächst statt abzuschneiden
// (`docs/UX.md` 6).

import 'dart:io' show File;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/tray/tray_diagnostics.dart';
import 'package:humanitl/features/tray/widgets/attention_notice.dart';
import 'package:humanitl/l10n/l10n.dart';

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

/// Die Karte in einem Fenster von [width], mit [textScaler].
Widget card({
  required Diagnostic diagnostic,
  double width = 900,
  TextScaler textScaler = TextScaler.noScaling,
}) => WidgetsApp(
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
          child: AttentionNoticeCard(diagnostic: diagnostic, onDismiss: () {}),
        ),
      ),
    ),
  ),
);

void main() {
  group('the codes the card may name', () {
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
    // Die Karte malt ihre eigene Fläche; alles darin steht auf `bg2`.
    final Color surface = tokens.colors.bg2;

    test('$theme: everything written on the card reaches 4.5:1', () {
      // Überschrift und Ursache.
      expect(
        contrastOn(tokens.colors.fg0, const <Color>[], surface),
        greaterThanOrEqualTo(4.5),
      );
      expect(
        contrastOn(tokens.colors.fg1, const <Color>[], surface),
        greaterThanOrEqualTo(4.5),
      );
      // Das Detail steht in einem eigenen Kasten auf `bg1`.
      expect(
        contrastOn(tokens.colors.fg1, const <Color>[], tokens.colors.bg1),
        greaterThanOrEqualTo(4.5),
      );
      // Der Link auf die Dokumentation trägt die Textvariante des Akzents.
      expect(
        contrastOn(tokens.colors.accentText, const <Color>[], surface),
        greaterThanOrEqualTo(4.5),
      );
    });

    test('$theme: the two badges and the rail reach their floor', () {
      // Der Schweregrad `info` färbt Rail und Badges im Akzent.
      final Color accent = tokens.colors.accent;
      // Die Rail ist Fläche: 3:1.
      expect(
        contrastOn(accent, const <Color>[], surface),
        greaterThanOrEqualTo(3),
      );
      // Die Beschriftung des Badges steht auf der Tönung derselben Farbe und
      // ist ein Wort: 4,5:1.
      expect(
        contrastOn(tokens.stateTextOf(accent), <Color>[
          HColorDerivation.tint(accent),
        ], surface),
        greaterThanOrEqualTo(4.5),
      );
      // Und die beiden anderen Schweregrade, die dieselbe Karte tragen kann.
      for (final Color state in <Color>[
        tokens.state.held,
        tokens.state.error,
      ]) {
        expect(
          contrastOn(state, const <Color>[], surface),
          greaterThanOrEqualTo(3),
          reason: 'Rail $state',
        );
        expect(
          contrastOn(tokens.stateTextOf(state), <Color>[
            HColorDerivation.tint(state),
          ], surface),
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

    await tester.pumpWidget(card(diagnostic: diagnostic));
    final double plain = tester
        .getSize(find.byType(AttentionNoticeCard))
        .height;

    await tester.pumpWidget(
      card(diagnostic: diagnostic, textScaler: const TextScaler.linear(2)),
    );

    // Kein Overflow, und die Karte ist wirklich gewachsen: eine feste Höhe
    // schluckte den Überlauf still (`docs/UX.md` 6).
    expect(tester.takeException(), isNull);
    expect(
      tester.getSize(find.byType(AttentionNoticeCard)).height,
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
    // (`windowMinimumSize`), und die Karte steht darin neben ihrer
    // Schliessen-Taste.
    await tester.pumpWidget(
      card(
        diagnostic: TrayDiagnostics.decidedAlready(
          const FlowId('018f0034-0000-7000-8000-000000000001'),
        ),
        width: 1100,
        textScaler: const TextScaler.linear(2),
      ),
    );

    expect(tester.takeException(), isNull);
  });
}
