// Der Weg vom Fund zur Fundstelle: ein Chip nennt den Fund, ein Klick bringt
// die Ansicht dorthin -- die Rohansicht scrollt, der Baum klappt den Pfad auf.
// Das Akzeptanzkriterium von HUM-030, an der zusammengesetzten Ansicht geprüft.

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/intercept/body/body_kind.dart';
import 'package:humanitl/features/intercept/body/body_view.dart';
import 'package:humanitl/features/intercept/providers/flow_body_provider.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../fixtures.dart';
import 'harness.dart';

/// Ein Rumpf, dessen Adresse tief verschachtelt und weit unten steht.
///
/// Beides zusammen ist der Fall, für den der Sprung gebaut ist: im Baum liegt
/// sie hinter zwei zugeklappten Knoten, in der Rohansicht unterhalb des
/// sichtbaren Ausschnitts.
final String deepBody = <String>[
  '{',
  '  "head": {"a": 1},',
  '  "filler": [',
  for (int i = 0; i < 40; i++) '    "line $i",',
  '    "last"',
  '  ],',
  '  "tail": {"contact": {"mail": "zz@example.org"}}',
  '}',
].join('\n');

Future<void> pumpCard(
  WidgetTester tester, {
  String? source,
  String contentType = 'application/json',
}) async {
  final TestDaemonClient client = TestDaemonClient();
  final String body = source ?? deepBody;
  final int start = body.indexOf('zz@example.org');
  final FlowDetail detail = detailFor(
    heldFlow(n: 1, deadline: testStart.add(const Duration(minutes: 5))),
    bodyPreview: body,
    contentType: contentType,
    findings: <Finding>[
      bodyFinding(start: start, end: start + 'zz@example.org'.length),
    ],
  );
  client.details[detail.summary.id] = detail;
  await tester.binding.setSurfaceSize(const Size(700, 600));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[daemonClientProvider.overrideWithValue(client)],
      child: WidgetsApp(
        color: HTokens.dark.colors.bg0,
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) => HTheme(
          tokens: HTokens.dark,
          child: Overlay(
            initialEntries: <OverlayEntry>[
              OverlayEntry(
                builder: (BuildContext context) => BodyView(
                  flowId: detail.summary.id,
                  body: detail.request!.body,
                ),
              ),
            ],
          ),
        ),
      ),
    ),
  );
  for (int i = 0; i < 6; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

void main() {
  testWidgets('the switcher offers the views the kind allows', (
    WidgetTester tester,
  ) async {
    await pumpCard(tester);
    expect(find.text(english.interceptBodyPaneTree), findsOneWidget);
    expect(find.text(english.interceptBodyPaneRaw), findsOneWidget);
    expect(find.text(english.interceptBodyPaneHex), findsOneWidget);
    expect(find.text(english.interceptBodyPaneForm), findsNothing);
    expect(find.byKey(const Key('body-tree')), findsOneWidget);
  });

  testWidgets('a chip names the finding and opens the path to it', (
    WidgetTester tester,
  ) async {
    await pumpCard(tester);
    // Zu Beginn ist nur die Wurzel offen: drei Kinder, kein Fundwert.
    ListView list() => tester.widget<ListView>(find.byType(ListView));
    expect(list().semanticChildCount, 4);
    await tester.tap(find.byKey(const Key('body-finding-chip-0')));
    await tester.pump();
    await tester.pump();
    // `tail` und `contact` sind aufgeklappt, also steht die Adresse in der
    // Liste.
    expect(list().semanticChildCount, greaterThan(4));
    expect(find.textContaining('zz@example.org'), findsOneWidget);
  });

  testWidgets('the raw view scrolls to the finding', (
    WidgetTester tester,
  ) async {
    await pumpCard(tester);
    await tester.tap(find.text(english.interceptBodyPaneRaw));
    await tester.pump();
    await tester.pump();
    expect(find.byKey(const Key('body-raw')), findsOneWidget);
    final ScrollableState scrollable = tester.state<ScrollableState>(
      find
          .descendant(
            of: find.byKey(const Key('body-raw')),
            matching: find.byType(Scrollable),
          )
          .last,
    );
    expect(scrollable.position.pixels, 0);
    await tester.tap(find.byKey(const Key('body-finding-chip-0')));
    await tester.pump();
    await tester.pump();
    expect(scrollable.position.pixels, greaterThan(0));
  });

  testWidgets('the chosen view is inherited by the next flow', (
    WidgetTester tester,
  ) async {
    // Die Wahl gehört dem Flow, solange seine Karte steht, und danach dem
    // nächsten: wer einmal auf Roh gestellt hat, will das beim nächsten `J`
    // nicht wieder tun. Eine Karte je jemals gesehener FlowId gäbe es dafür
    // nicht (`docs/UX.md` 7).
    await pumpCard(tester);
    expect(find.byKey(const Key('body-tree')), findsOneWidget);
    await tester.tap(find.text(english.interceptBodyPaneRaw));
    await tester.pump();
    expect(find.byKey(const Key('body-raw')), findsOneWidget);

    final ProviderContainer container = ProviderScope.containerOf(
      tester.element(find.byType(BodyView)),
    );
    expect(container.read(lastBodyPaneProvider), BodyPane.raw);
    expect(container.read(bodyViewModeProvider(testFlowId(2))), BodyPane.raw);
  });

  testWidgets('an empty body says so and offers no switcher', (
    WidgetTester tester,
  ) async {
    await pumpCard(tester, source: '');
    expect(find.text(english.interceptBodyEmpty), findsOneWidget);
    expect(find.text(english.interceptBodyPaneRaw), findsNothing);
  });

  testWidgets('a body that is not JSON keeps the raw text and says why', (
    WidgetTester tester,
  ) async {
    await pumpCard(tester, source: 'this was never json');
    expect(find.text(english.interceptBodyNotJson), findsOneWidget);
    expect(find.byKey(const Key('body-raw')), findsOneWidget);
    expect(find.text(english.interceptBodyEmpty), findsNothing);
  });
}
