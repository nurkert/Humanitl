import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Ausrichtung, die ein Golden mit Blockschrift nicht zeigt (HUM-406).
void main() {
  double baselineOf(WidgetTester tester, Finder finder) {
    final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
      find.descendant(of: finder, matching: find.byType(RichText)).first,
    );
    final double local = paragraph.computeDistanceToActualBaseline(
      TextBaseline.alphabetic,
    );
    return paragraph.localToGlobal(Offset(0, local)).dy;
  }

  testWidgets('a key hint stands on the baseline of the label before it', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      harness(
        Builder(
          builder: (BuildContext context) {
            final HTokens tokens = HTheme.of(context);
            return Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text(
                  'Block',
                  key: const Key('label'),
                  style: tokens.typography.ui13.medium,
                ),
                const SizedBox(width: 8),
                const HKeyHint('B', key: Key('hint')),
              ],
            );
          },
        ),
      ),
    );
    expect(
      baselineOf(tester, find.byKey(const Key('hint'))),
      moreOrLessEquals(baselineOf(tester, find.byKey(const Key('label')))),
    );
  });

  group('HMiddleEllipsis', () {
    const String path =
        '/home/nik/clients/acme-corporation/projects/humanitl-internal-tools';

    Future<String> shown(
      WidgetTester tester,
      double width, {
      String text = path,
    }) async {
      await tester.pumpWidget(
        harness(
          Align(
            alignment: Alignment.centerLeft,
            child: SizedBox(
              width: width,
              child: Align(
                alignment: Alignment.centerLeft,
                child: HMiddleEllipsis(text, key: const Key('path')),
              ),
            ),
          ),
        ),
      );
      return tester
          .widget<Text>(
            find.descendant(
              of: find.byKey(const Key('path')),
              matching: find.byType(Text),
            ),
          )
          .data!;
    }

    testWidgets('shows the whole path when it fits', (
      WidgetTester tester,
    ) async {
      // Die Testschrift ist breit: jedes Zeichen so breit wie hoch.
      expect(
        await shown(tester, 470, text: '/home/nik/acme'),
        '/home/nik/acme',
      );
    });

    testWidgets('cuts in the middle and keeps the end when it does not', (
      WidgetTester tester,
    ) async {
      final String text = await shown(tester, 160);
      expect(text, contains(HMiddleEllipsis.ellipsis));
      expect(path.startsWith(text.split(HMiddleEllipsis.ellipsis).first), true);
      expect(path.endsWith(text.split(HMiddleEllipsis.ellipsis).last), true);
      expect(text, endsWith('tools'));
      expect(text, startsWith('/'));
      final RenderBox box = tester.renderObject<RenderBox>(
        find.byKey(const Key('path')),
      );
      expect(box.size.width, lessThanOrEqualTo(160));
      expect(tester.takeException(), isNull);
    });
    testWidgets('a reader hears the whole path, not the shortened one', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      final String text = await shown(tester, 160);
      expect(text, contains(HMiddleEllipsis.ellipsis));
      expect(find.bySemanticsLabel(path), findsOneWidget);
      expect(
        tester.getSemantics(find.bySemanticsLabel(path)).label,
        path,
        reason: 'the full path, not the shortened text',
      );
      expect(find.bySemanticsLabel(text), findsNothing);
      handle.dispose();
    });

    testWidgets('cuts between graphemes, never inside one', (
      WidgetTester tester,
    ) async {
      // `é` als `e` plus Akzent: zwei UTF-16-Einheiten, ein Zeichen.
      final String accented = '/home/${'e\u0301' * 30}/ende';
      final String text = await shown(tester, 160, text: accented);
      final List<String> parts = text.split(HMiddleEllipsis.ellipsis);
      expect(parts, hasLength(2));
      expect(accented.characters.startsWith(parts.first.characters), isTrue);
      expect(accented.characters.endsWith(parts.last.characters), isTrue);
    });
  });
}
