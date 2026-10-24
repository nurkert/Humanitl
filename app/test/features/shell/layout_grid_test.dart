// Das Layoutraster der Shell (HUM-286, `docs/UX.md` "Layout grid"): Jede
// sichtbare Kopfzeile eines Panes und eines Blatts endet auf derselben Linie,
// und der aktive Eintrag der Rail beginnt direkt unter der Kopfleiste, in
// derselben Reihe wie die Kopfzeilen daneben. Das gilt auch bei doppelter
// Schrift: Die Reihe wächst dann einmal, für alle.
//
// Gemessen wird an der ganzen App und nicht an einem Widget allein: Die
// Fehler, die HUM-286 behebt, entstehen genau dort, wo zwei Flächen
// nebeneinander stehen, die jede für sich richtig aussieht.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/providers/flows.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/shell/section.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/features/shell/widgets/icon_rail.dart';

import '../../harness/app_harness.dart';
import '../intercept/fixtures.dart';

/// Zwei angehaltene Anfragen, damit Queue, Karte und Kontext-Pane alle eine
/// Kopfzeile zeigen. Die erste, ausgewählte, hat eine URL, die in der Karte
/// zwei Zeilen braucht: der Fall, in dem eine wachsende Kopfzeile aus der
/// Reihe liefe.
List<FlowDetail> _details() => <FlowDetail>[
  detailFor(
    heldFlow(
      n: 1,
      deadline: testStart.add(const Duration(minutes: 5)),
      host: 'api.github.com',
      apex: 'github.com',
      path:
          '/repos/humanitl/humanitl/issues/286/comments/with/a/long/path'
          '?first=20&after=Y3Vyc29yOnYyOpHOAAECAwQFBgcICQ',
    ),
  ),
  detailFor(
    heldFlow(
      n: 2,
      deadline: testStart.add(const Duration(minutes: 8)),
      host: 'registry.npmjs.org',
      apex: 'npmjs.org',
      path: '/react',
    ),
  ),
];

List<Override> _overrides(List<FlowDetail> details) => <Override>[
  nowProvider.overrideWith(
    () => FixedNow(testStart.add(const Duration(seconds: 40))),
  ),
  flowsProvider.overrideWith(
    () => FixedFlows(<FlowId, Flow>{
      for (final FlowDetail detail in details)
        detail.summary.id: detail.summary,
    }),
  ),
];

Future<void> _pump(
  WidgetTester tester, {
  Size size = const Size(1280, 800),
  double textScale = 1,
}) async {
  if (textScale != 1) {
    tester.platformDispatcher.textScaleFactorTestValue = textScale;
    addTearDown(tester.platformDispatcher.clearTextScaleFactorTestValue);
  }
  final List<FlowDetail> details = _details();
  final TestDaemonClient client = TestDaemonClient();
  for (final FlowDetail detail in details) {
    client.details[detail.summary.id] = detail;
  }
  await pumpApp(
    tester,
    client: client,
    overrides: _overrides(details),
    size: size,
  );
  await tester.pumpAndSettle();
}

/// Die Höhe der gemeinsamen Reihe bei [textScale].
///
/// Hier ausgeschrieben und nicht aus `HPaneHeader.rowHeight` gelesen: Eine
/// Änderung dort soll diesen Test rot machen und nicht seine Erwartung
/// mitnehmen.
double _row(double textScale) =>
    (HTokens.dark.sizes.paneHeaderHeight * textScale.clamp(1, 2))
        .roundToDouble();

/// Die Unterkante der Kopfleiste des Fensters: die Linie, unter der Rail und
/// Panes beginnen.
double _headerBarBottom(WidgetTester tester) =>
    tester.getRect(find.byType(HeaderBar)).bottom;

/// Jede sichtbare Kopfzeile, mit ihrem Rechteck.
List<Rect> _paneHeaders(WidgetTester tester) => <Rect>[
  for (final Element element in find.byType(HPaneHeader).evaluate())
    tester.getRect(find.byElementPredicate((Element e) => e == element)),
];

/// Prüft, dass jede sichtbare Kopfzeile in der Reihe unter der Kopfleiste
/// steht und auf derselben Linie endet, dass die Rail ohne Abstand unter der
/// Kopfleiste beginnt und dass die Markierung des aktiven Eintrags
/// [active] dessen ganze Höhe füllt, Kante an Kante.
void _expectOneGrid(
  WidgetTester tester,
  String where,
  Section active, {
  double textScale = 1,
}) {
  final double row = _row(textScale);
  final double top = _headerBarBottom(tester);
  final double bottom = top + row;
  final List<Rect> headers = _paneHeaders(tester);
  expect(headers, isNotEmpty, reason: '$where: no pane header on screen');
  for (final Rect header in headers) {
    expect(
      header.top,
      top,
      reason: '$where: a pane header starts under the window header',
    );
    expect(
      header.bottom,
      bottom,
      reason: '$where: every pane header ends on the same line',
    );
  }
  final double rowTop = top + active.index * row;
  final Rect mark = tester.getRect(
    find.byKey(const Key('icon-rail-active-mark')),
  );
  expect(
    mark.top,
    rowTop,
    reason: '$where: the highlight touches the top edge of its item',
  );
  expect(mark.height, row, reason: '$where: the highlight fills the item');
  expect(
    tester.getRect(_railEntry(active)),
    Rect.fromLTWH(0, rowTop, railWidth, row),
    reason: '$where: the rail starts directly under the window header',
  );
}

/// Prüft, dass das Blatt [sheet] unter der Kopfleiste des Fensters beginnt
/// und seine Kopfzeile auf der Linie von [header] endet.
///
/// Gemessen an der Linie selbst und nicht am Widget-Typ: Die oberste
/// waagerechte Haarlinie des Blatts ist die Unterkante seiner Kopfzeile,
/// gleich wie die Kopfzeile gebaut ist.
void _expectSheetOnGrid(
  WidgetTester tester,
  Finder sheet,
  Finder header,
  String where,
) {
  final List<Rect> lines = <Rect>[
    for (final Element element
        in find
            .descendant(
              of: sheet,
              matching: find.byWidgetPredicate(
                (Widget widget) => widget is HHairline && !widget.vertical,
              ),
            )
            .evaluate())
      tester.getRect(find.byElementPredicate((Element e) => e == element)),
  ]..sort((Rect a, Rect b) => a.top.compareTo(b.top));
  expect(lines, isNotEmpty, reason: '$where: the sheet header has a divider');
  expect(
    tester.getRect(sheet).top,
    _headerBarBottom(tester),
    reason: '$where: the sheet hangs under the window header, like a pane',
  );
  expect(
    lines.first.bottom,
    tester.getRect(header).bottom,
    reason: '$where: the sheet header ends on the line of the pane header',
  );
}

Finder _railEntry(Section section) => find.byWidgetPredicate(
  (Widget widget) => widget is RailEntry && widget.section == section,
);

void main() {
  testWidgets('Intercept: queue, card and context header share one line', (
    WidgetTester tester,
  ) async {
    await _pump(tester);
    expect(find.byKey(const Key('intercept-queue-header')), findsOneWidget);
    expect(find.byKey(const Key('intercept-card-header')), findsOneWidget);
    expect(find.byKey(const Key('intercept-domain-header')), findsOneWidget);
    _expectOneGrid(tester, 'Intercept', Section.intercept);
  });

  testWidgets('the settings sheet header continues the pane header line', (
    WidgetTester tester,
  ) async {
    await _pump(tester);
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    _expectSheetOnGrid(
      tester,
      find.byKey(const Key('settings-sheet')),
      find.byKey(const Key('intercept-queue-header')),
      'settings',
    );
    _expectOneGrid(tester, 'Intercept with settings', Section.intercept);
  });

  for (final Section section in <Section>[
    Section.history,
    Section.rules,
    Section.sandbox,
  ]) {
    testWidgets('${section.name}: the header sits on the grid', (
      WidgetTester tester,
    ) async {
      await _pump(tester);
      await tester.tap(_railEntry(section));
      await tester.pumpAndSettle();
      _expectOneGrid(tester, section.name, section);
    });
  }

  testWidgets('narrow Rules: the editor sheet hangs under the window header', (
    WidgetTester tester,
  ) async {
    // Unter `rulesSheetBelow` wird der Editor zum Blatt; der Screen hängt es
    // ein, nicht die Liste unter seiner Kopfzeile.
    await _pump(tester, size: const Size(900, 800));
    await tester.tap(_railEntry(Section.rules));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('rules-new')));
    await tester.pumpAndSettle();
    _expectSheetOnGrid(
      tester,
      find.byKey(const Key('rules-editor-sheet')),
      find.byKey(const Key('rules-header')),
      'rules editor',
    );
  });

  testWidgets('Sandbox: the command sheet continues the header line', (
    WidgetTester tester,
  ) async {
    await _pump(tester);
    await tester.tap(_railEntry(Section.sandbox));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(const Key('sandbox-show-argv')));
    await tester.pumpAndSettle();
    _expectSheetOnGrid(
      tester,
      find.byType(HSheet),
      find.byKey(const Key('sandbox-header')),
      'argv',
    );
  });

  testWidgets('at twice the text the row grows once, for everybody', (
    WidgetTester tester,
  ) async {
    await _pump(tester, textScale: 2);
    _expectOneGrid(tester, 'Intercept x2', Section.intercept, textScale: 2);
    expect(tester.takeException(), isNull, reason: 'intercept');
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    _expectSheetOnGrid(
      tester,
      find.byKey(const Key('settings-sheet')),
      find.byKey(const Key('intercept-queue-header')),
      'settings x2',
    );
    // Bei doppelter Schrift läuft eine Zeile des Einstellungsformulars über
    // (eine `Row` mit `MainAxisSize.min` im Rumpf, 469 px breit). Das ist der
    // Rumpf des Blatts und nicht seine Kopfzeile, und es steht vor HUM-286;
    // es ist als eigenes Issue gemeldet. Hier wird es abgeräumt, damit es die
    // Prüfung der übrigen Bildschirme nicht verdeckt.
    tester.takeException();
    await pressCtrl(tester, LogicalKeyboardKey.comma);
    await tester.pumpAndSettle();
    for (final Section section in <Section>[
      Section.history,
      Section.rules,
      Section.sandbox,
    ]) {
      await tester.tap(_railEntry(section));
      await tester.pumpAndSettle();
      _expectOneGrid(tester, '${section.name} x2', section, textScale: 2);
      expect(tester.takeException(), isNull, reason: section.name);
    }
    expect(tester.takeException(), isNull);
  });

  testWidgets('a low window at twice the text keeps the active mark in view', (
    WidgetTester tester,
  ) async {
    await _pump(tester, size: const Size(1100, 700), textScale: 2);
    tester.takeException();
    // The scrolling part of the rail, without the toggle at its foot.
    final Rect rail = tester.getRect(
      find
          .descendant(
            of: find.byKey(const Key('icon-rail')),
            matching: find.byType(Scrollable),
          )
          .first,
    );
    for (final LogicalKeyboardKey key in <LogicalKeyboardKey>[
      LogicalKeyboardKey.digit6,
      LogicalKeyboardKey.digit1,
    ]) {
      await pressCtrl(tester, key);
      await tester.pumpAndSettle();
      tester.takeException();
      final Rect mark = tester.getRect(
        find.byKey(const Key('icon-rail-active-mark')),
      );
      expect(
        mark.top >= _headerBarBottom(tester) && mark.bottom <= rail.bottom,
        isTrue,
        reason: '$key: the active mark $mark is fully visible in $rail',
      );
    }
  });
}
