// Die Aktionsleiste auf Deutsch (HUM-052, Schritt 7): Deutsche Texte sind
// rund ein Drittel länger als englische, und die Leiste hat feste Maße. Jede
// Beschriftung muss in einer Zeile stehen; ein Umbruch in einer 28-px-Zeile
// schneidet den Text ab oder drückt die Leiste auseinander. Die Goldens
// `action_bar_de` und `intercept_card_de` kommen mit HUM-054.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/app.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/intercept/providers/now.dart';
import 'package:humanitl/features/intercept/widgets/action_bar.dart';
import 'package:humanitl/features/shell/providers/connection.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'fixtures.dart';

final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

FlowDetail request({required int findings}) => detailFor(
  heldFlow(
    n: 1,
    deadline: testStart.add(const Duration(minutes: 5)),
    method: Method.post,
    host: 'api.github.com',
    apex: 'github.com',
    path: '/graphql?first=20',
    requestSize: 428,
  ).copyWith(findingCount: findings),
  bodyPreview: '{"query": "mutation { createIssue }"}',
  contentType: 'application/json',
  apex: 'github.com',
  findings: <Finding>[for (int i = 0; i < findings; i++) testFinding()],
);

Future<void> pumpGerman(WidgetTester tester, FlowDetail detail) async {
  tester.platformDispatcher.localesTestValue = const <Locale>[Locale('de')];
  addTearDown(tester.platformDispatcher.clearLocalesTestValue);
  await tester.binding.setSurfaceSize(const Size(1280, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    ProviderScope(
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(
          FakeDaemonClient(
            script: holdScript(<FlowDetail>[detail]),
            clock: () => testStart,
          ),
        ),
        connectionHeartbeatProvider.overrideWithValue(null),
        nowProvider.overrideWith(() => FixedNow(testStart)),
      ],
      child: const HumanitlApp(),
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pump(const Duration(milliseconds: 400));
  await tester.pump();
  await tester.pump();
}

/// How many lines [paragraph] is laid out in.
int linesOf(RenderParagraph paragraph) {
  final int length = paragraph.text.toPlainText().length;
  if (length == 0) {
    return 0;
  }
  return paragraph
      .getBoxesForSelection(TextSelection(baseOffset: 0, extentOffset: length))
      .map((TextBox box) => box.top.round())
      .toSet()
      .length;
}

/// Every label of the action bar that wraps or is cut short, with its text.
///
/// Cut short counts: the release valve and the reason set `maxLines: 1` with
/// an ellipsis, so a label too long for them never wraps but loses its end.
/// [except] names sentences that carry a host or a finding and may be cut in
/// English as well; they still must not wrap.
List<String> wrapped(WidgetTester tester, {Set<String> except = const {}}) =>
    <String>[
      for (final RenderParagraph paragraph
          in tester.renderObjectList<RenderParagraph>(
            find.descendant(
              of: find.byType(ActionBar),
              matching: find.byType(RichText),
            ),
          ))
        if (linesOf(paragraph) > 1 ||
            (paragraph.didExceedMaxLines &&
                !except.contains(paragraph.text.toPlainText())))
          paragraph.text.toPlainText(),
    ];

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  testWidgets('the German action bar keeps every label on one line', (
    WidgetTester tester,
  ) async {
    await pumpGerman(tester, request(findings: 0));

    expect(find.byType(ActionBar), findsOneWidget);
    expect(find.text(de.interceptAllowButton), findsWidgets);
    expect(wrapped(tester), isEmpty);
    expect(tester.takeException(), isNull);
  }, variant: linux);

  testWidgets('with findings as well', (WidgetTester tester) async {
    await pumpGerman(tester, request(findings: 2));

    expect(find.text(de.interceptSendWithFindings(2)), findsWidgets);
    // Die beiden Sätze über den Fund nennen Art und Host; sie werden auch im
    // Englischen gekürzt, wenn der Host lang ist. Umbrechen dürfen sie nicht.
    expect(
      wrapped(
        tester,
        except: <String>{
          'Beim Senden geht ein API-Schlüssel von github an api.github.com.',
          'Angehalten: ein API-Schlüssel von github steht im Body',
        },
      ),
      isEmpty,
    );
    expect(tester.takeException(), isNull);
  }, variant: linux);
}
