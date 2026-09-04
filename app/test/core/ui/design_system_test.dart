// Die Handoff-Widgets aus `lib/core/ui`, gegen die Regeln geprüft, die
// `docs/UX.md` für das Designsystem aufstellt: Kontrast (6), Tastaturparität
// (5.1), keine Dauer als Literal (2.1) und ein Fokusring, der nicht auf der
// Füllung liegt, die er umgibt (6).
//
// Sie stehen hier und nicht in `packages/ui/test`, weil die Widgets hier
// stehen; sie ziehen mit ihnen um (`docs/UX.md` 9, Punkt 31).

import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/focus_ring.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/h_collapsible.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/core/ui/h_resizable_panes.dart';
import 'package:humanitl/core/ui/hold_to_confirm.dart';
import 'package:humanitl/core/ui/hover_label.dart';
import 'package:humanitl/core/ui/section_placeholder.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Ein Wirt mit Theme, Sprache, Overlay und den Standard-Tastenzuordnungen —
/// alles, was die Anwendung mitbringt und ein einzelnes Widget braucht.
Widget host(Widget child, {HTokens? tokens}) => WidgetsApp(
  color: HColors.bg0,
  debugShowCheckedModeBanner: false,
  locale: const Locale('en'),
  localizationsDelegates: AppLocalizations.localizationsDelegates,
  supportedLocales: AppLocalizations.supportedLocales,
  onGenerateTitle: (BuildContext context) => 'design system',
  builder: (BuildContext context, Widget? _) => HTheme(
    tokens: tokens ?? HTokens.dark,
    child: Overlay(
      initialEntries: <OverlayEntry>[
        OverlayEntry(
          // Ein Bereich, der den Fokus annimmt: ohne ihn hat `Tab` keinen
          // Ausgangspunkt, und die Anwendung hat ihn über ihre Route.
          builder: (BuildContext context) => FocusScope(
            autofocus: true,
            child: Align(alignment: Alignment.topLeft, child: child),
          ),
        ),
      ],
    ),
  ),
);

void main() {
  test('the hold fill is the token of the system, not a fourth alpha', () {
    // `HoldToConfirm` malte 20 % Zustandsfarbe, `HPill` 10 %, und die
    // Textableitung wusste von keinem der beiden (`docs/UX.md` 6).
    expect(holdFillAlpha, HColors.fillHoldAlpha);
    expect(HColorDerivation.fillAlphas, contains(holdFillAlpha));
    expect(holdFill(HColors.blocked).a, closeTo(HColors.fillHoldAlpha, 1e-9));
    // Und die Textvariante jeder Zustandsfarbe hält auf ihr 4,5:1.
    for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
      for (final HFlowState state in HFlowState.values) {
        for (final Color surface in tokens.colors.ladder) {
          expect(
            HColorDerivation.contrast(
              tokens.stateTextColor(state),
              HColorDerivation.flatten(
                holdFill(tokens.stateColor(state)),
                surface,
              ),
            ),
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason: '${tokens.brightness.name} ${state.name}',
          );
        }
      }
    }
  });

  test('a hold over a tinted surface composes to the token', () {
    // Zwei Schichten derselben Farbe addieren sich nicht: 0,20 über einer
    // Tönung von 0,06 sind wirksam 0,248, und dort steht die Beschriftung bei
    // 4,14:1 (`docs/UX.md` 6).
    expect(holdFill(HColors.blocked).a, closeTo(HColors.fillHoldAlpha, 1e-9));
    final Color over = holdFill(
      HColors.blocked,
      beneath: HColors.fillRestAlpha,
    );
    expect(
      over.a,
      closeTo(
        HColorDerivation.alphaOver(
          HColors.fillHoldAlpha,
          HColors.fillRestAlpha,
        ),
        1e-9,
      ),
    );
    for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
      for (final HFlowState state in HFlowState.values) {
        final Color area = tokens.stateColor(state);
        for (final Color surface in tokens.colors.ladder) {
          final Color rest = HColorDerivation.flatten(
            area.withValues(alpha: HColors.fillRestAlpha),
            surface,
          );
          final Color naive = HColorDerivation.flatten(holdFill(area), rest);
          final Color composed = HColorDerivation.flatten(
            holdFill(area, beneath: HColors.fillRestAlpha),
            rest,
          );
          final Color text = tokens.stateTextColor(state);
          expect(
            HColorDerivation.contrast(text, composed),
            greaterThanOrEqualTo(HColorDerivation.textMinContrast),
            reason: '${tokens.brightness.name} ${state.name}',
          );
          // Und die naive Schichtung ist wirklich dunkler, sonst prüfte der
          // Test nichts.
          expect(
            naive.computeLuminance(),
            isNot(closeTo(composed.computeLuminance(), 1e-6)),
          );
        }
      }
    }
  });

  test('no duration in core/ui is a literal', () {
    // Jede Dauer kommt aus `HMotion`; eine Zahl im Widget ist ein Defekt,
    // auch wenn sie stimmt (`docs/UX.md` 2.1).
    final Directory dir = Directory('lib/core/ui');
    expect(dir.existsSync(), isTrue, reason: 'lauf aus `app/`');
    final List<File> sources = dir
        .listSync()
        .whereType<File>()
        .where((File file) => file.path.endsWith('.dart'))
        .toList();
    expect(sources, isNotEmpty);
    for (final File file in sources) {
      expect(
        RegExp(r'Duration\(').hasMatch(file.readAsStringSync()),
        isFalse,
        reason: '${file.path} schreibt eine Dauer selbst',
      );
    }
  });

  testWidgets('a placeholder sentence stands in fg1, not in fg2', (
    WidgetTester tester,
  ) async {
    // `fg2` misst 3,03:1 bis 3,90:1 und ist wirklich deaktivierten Controls
    // vorbehalten (`docs/UX.md` 6).
    for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
      await tester.pumpWidget(
        host(
          const SectionPlaceholder(title: 'Audit', hint: 'Nothing here yet.'),
          tokens: tokens,
        ),
      );
      await tester.pump();
      final Color? colour = tester
          .widget<Text>(find.text('Nothing here yet.'))
          .style
          ?.color;
      expect(colour, tokens.colors.fg1);
      expect(colour, isNot(tokens.colors.fg2));
      expect(
        HColorDerivation.contrast(colour!, tokens.colors.bg1),
        greaterThanOrEqualTo(HColorDerivation.textMinContrast),
      );
    }
  });

  testWidgets('the accent carries no text: the diagnostic card uses the '
      'text variant', (WidgetTester tester) async {
    for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
      await tester.pumpWidget(
        host(
          HDiagnosticCard(
            code: 'DAEMON_001',
            severityLabel: 'Error',
            color: tokens.state.error,
            title: 'Daemon not reachable',
            why: 'The socket is not there.',
            docsUrl: 'https://example.invalid/docs#daemon-001',
            fix: const FixControl(
              fix: FixAction.openUrl(url: 'https://example.invalid/docs'),
            ),
          ),
          tokens: tokens,
        ),
      );
      await tester.pump();
      for (final String text in <String>[
        'https://example.invalid/docs#daemon-001',
        'https://example.invalid/docs',
      ]) {
        expect(
          tester.widget<Text>(find.text(text)).style?.color,
          tokens.colors.accentText,
          reason: '${tokens.brightness.name} $text',
        );
      }
      expect(
        HColorDerivation.contrast(tokens.colors.accentText, tokens.colors.bg2),
        greaterThanOrEqualTo(HColorDerivation.textMinContrast),
      );
    }
  });

  testWidgets('a section keeps one curve, not one per build', (
    WidgetTester tester,
  ) async {
    // `CurvedAnimation` hängt im Konstruktor einen Statuslistener an den
    // Controller, den niemand wieder abnimmt: als Ausdruck in `build` wächst
    // die Liste mit jedem Rebuild (`docs/UX.md` 7).
    await tester.pumpWidget(
      host(
        const SizedBox(
          width: 300,
          child: HCollapsible(title: 'Headers', child: Text('accept: */*')),
        ),
      ),
    );
    await tester.pumpAndSettle();
    final Animation<double> before = tester
        .widget<SizeTransition>(find.byType(SizeTransition))
        .sizeFactor;
    // Ein Rebuild: der Fokus wechselt auf die Kopfzeile.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    final Animation<double> after = tester
        .widget<SizeTransition>(find.byType(SizeTransition))
        .sizeFactor;
    expect(identical(before, after), isTrue);
  });

  testWidgets('core/ui survives double text scale', (
    WidgetTester tester,
  ) async {
    // Die Kopfzeile eines Klappabschnitts hatte eine feste Höhe von 28 px um
    // eine Zeile, die bei doppelter Skalierung 32 px misst: kein Fehlschlag,
    // stiller Beschnitt (`docs/UX.md` 6).
    await tester.pumpWidget(
      host(
        MediaQuery(
          data: const MediaQueryData(textScaler: TextScaler.linear(2)),
          child: SizedBox(
            width: 420,
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  const HCollapsible(
                    title: 'Headers',
                    child: Text('accept: */*'),
                  ),
                  const SectionPlaceholder(
                    title: 'Audit',
                    hint: 'Nothing here yet.',
                  ),
                  HDiagnosticCard(
                    code: 'DAEMON_001',
                    severityLabel: 'Error',
                    color: HTokens.dark.state.error,
                    title: 'Daemon not reachable',
                    why: 'The socket is not there.',
                    width: 400,
                    fix: const FixControl(
                      fix: FixAction.copyCommand(command: 'humanitld'),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(tester.takeException(), isNull);
    // Die Kopfzeile wächst mit ihrer Schrift, statt sie abzuschneiden.
    final Size header = tester.getSize(find.text('Headers'));
    final Size row = tester.getSize(
      find
          .descendant(
            of: find.byType(HCollapsible),
            matching: find.byType(ConstrainedBox),
          )
          .first,
    );
    expect(header.height, greaterThan(HSize.hitMin));
    expect(row.height, greaterThanOrEqualTo(header.height));
  });

  testWidgets('a section folds with the keyboard, not only with the mouse', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      host(
        const SizedBox(
          width: 300,
          child: HCollapsible(title: 'Headers', child: Text('accept: */*')),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(find.text('accept: */*'), findsOneWidget);
    final double open = tester.getSize(find.byType(HCollapsible)).height;
    expect(open, greaterThan(HSize.hitMin));
    // Tab erreicht die Kopfzeile, Enter faltet sie zu (`docs/UX.md` 5.1).
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(
      tester.widget<HFocusRing>(find.byType(HFocusRing)).visible,
      isTrue,
      reason: 'die Kopfzeile ist ein Fokusstopp und zeigt ihren Ring',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pumpAndSettle();
    expect(tester.getSize(find.byType(HCollapsible)).height, HSize.hitMin);
    // Und die Leertaste faltet sie wieder auf.
    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(tester.getSize(find.byType(HCollapsible)).height, open);
    expect(tester.takeException(), isNull);
  });

  testWidgets('both focus rings keep the same distance from their control', (
    WidgetTester tester,
  ) async {
    // Zwei Ringe für dieselbe Sache (`docs/UX.md` 9, Punkt 31): solange es
    // sie beide gibt, laufen sie nicht auseinander.
    final Color accent = HTokens.dark.colors.accent;
    expect(focusRingWidth, HFocusRing.width);
    expect(focusRingReserved(null, accent), HFocusRing.width);
    expect(
      focusRingReserved(HTokens.dark.colors.accentFill, accent),
      HFocusRing.width + HFocusRing.gap,
    );
    await tester.pumpWidget(
      host(
        const Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            FocusRing(
              key: Key('plain'),
              visible: true,
              radius: 4,
              child: SizedBox.square(dimension: 20),
            ),
            FocusRing(
              key: Key('filled'),
              visible: true,
              radius: 4,
              fill: HColors.accent,
              child: SizedBox.square(dimension: 20),
            ),
          ],
        ),
      ),
    );
    await tester.pump();
    expect(
      tester.getSize(find.byKey(const Key('plain'))).height,
      20 + 2 * HFocusRing.width,
    );
    expect(
      tester.getSize(find.byKey(const Key('filled'))).height,
      20 + 2 * (HFocusRing.width + HFocusRing.gap),
    );
  });

  testWidgets('a splitter moves with the arrow keys', (
    WidgetTester tester,
  ) async {
    const double width = 400;
    // Kein Literal mehr: die Breite des Griffs ist ein Token
    // (`docs/UX.md` 2.1).
    const double splitter = HSize.splitter;
    const double available = width - splitter;
    List<double>? seen;
    await tester.pumpWidget(
      host(
        SizedBox(
          width: width,
          height: 200,
          child: HResizablePanes(
            ratios: const <double>[0.5, 0.5],
            minWidths: const <double>[50, 50],
            splitterSemanticsLabel: 'resize',
            onRatiosChanged: (List<double> ratios) => seen = ratios,
            children: const <Widget>[Text('left'), Text('right')],
          ),
        ),
      ),
    );
    await tester.pumpAndSettle();
    expect(
      tester.getSize(find.byType(HResizablePanes)).width - 2 * 150,
      greaterThan(0),
    );
    expect(splitter, HSize.splitter);
    // Die Linie ruht als Haarlinie.
    Finder line() => find.descendant(
      of: find.byType(HResizablePanes),
      matching: find.byWidgetPredicate(
        (Widget widget) => widget is SizedBox && widget.child is ColoredBox,
      ),
    );
    expect(tester.widget<SizedBox>(line()).width, HSize.hairline);

    // Der Splitter ist der einzige Fokusstopp: ein Tab genügt.
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pumpAndSettle();
    expect(
      tester.widget<SizedBox>(line()).width,
      HSize.splitterActive,
      reason: 'der fokussierte Splitter zeigt sich',
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pump();
    expect(seen, isNotNull);
    expect(
      seen![0],
      closeTo(0.5 + HSize.splitterStep / available, 0.001),
      reason: 'rechts vergrößert die linke Fläche um einen Schritt',
    );
    seen = null;
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
    await tester.pump();
    expect(seen, isNotNull);
    expect(seen![0], closeTo(0.5 - HSize.splitterStep / available, 0.001));
    expect(tester.takeException(), isNull);
  });

  testWidgets('the hover label waits its token, not a number', (
    WidgetTester tester,
  ) async {
    expect(
      const HoverLabel(label: 'Rules', child: SizedBox.shrink()).delay,
      HMotion.hoverLabel,
    );
    await tester.pumpWidget(
      host(
        const HoverLabel(label: 'Rules', child: SizedBox.square(dimension: 40)),
      ),
    );
    final TestGesture pointer = await tester.createGesture(
      kind: PointerDeviceKind.mouse,
    );
    await pointer.addPointer(location: Offset.zero);
    addTearDown(pointer.removePointer);
    await pointer.moveTo(tester.getCenter(find.byType(HoverLabel)));
    await tester.pump();
    expect(find.text('Rules'), findsNothing);
    await tester.pump(HMotion.hoverLabel - const Duration(milliseconds: 1));
    expect(find.text('Rules'), findsNothing);
    await tester.pump(const Duration(milliseconds: 2));
    expect(find.text('Rules'), findsOneWidget);
    await pointer.moveTo(Offset.zero);
    await tester.pumpAndSettle();
  });
}
