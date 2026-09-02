import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/features/settings/gallery_screen.dart';
import 'package:humanitl/main.dart';
import 'package:humanitl/core/ui/ui.dart';

void main() {
  group('gallery entry point', () {
    const List<(String, List<String>, Map<String, String>, bool)> cases =
        <(String, List<String>, Map<String, String>, bool)>[
          ('nothing asked for it', <String>[], <String, String>{}, false),
          ('flag', <String>['--gallery'], <String, String>{}, true),
          (
            'environment',
            <String>[],
            <String, String>{'HUMANITL_GALLERY': '1'},
            true,
          ),
          (
            'environment set to zero',
            <String>[],
            <String, String>{'HUMANITL_GALLERY': '0'},
            false,
          ),
          (
            'environment set to empty',
            <String>[],
            <String, String>{'HUMANITL_GALLERY': ''},
            false,
          ),
          (
            'environment set to false',
            <String>[],
            <String, String>{'HUMANITL_GALLERY': 'false'},
            false,
          ),
        ];

    for (final (
          String name,
          List<String> args,
          Map<String, String> env,
          bool expected,
        )
        in cases) {
      test(name, () {
        expect(galleryRequested(args, environment: env), expected);
      });
    }
  });

  testWidgets('the gallery screen renders the gallery and follows its theme '
      'switcher', (WidgetTester tester) async {
    // The application background of the gallery: the outermost ColoredBox
    // under the page, bg0 of whichever theme is active.
    Color background() => tester
        .widget<ColoredBox>(
          find
              .descendant(
                of: find.byType(HGalleryPage),
                matching: find.byType(ColoredBox),
              )
              .first,
        )
        .color;

    await tester.pumpWidget(const GalleryScreen());
    await tester.pump();
    expect(find.byType(HGalleryPage), findsOneWidget);
    expect(background(), HColors.bg0);

    await tester.tap(find.widgetWithText(HButton, 'Light'));
    await tester.pump();
    expect(background(), HColors.lBg0);

    await tester.tap(find.widgetWithText(HButton, 'Dark'));
    await tester.pump();
    expect(background(), HColors.bg0);
    expect(tester.takeException(), isNull);
  });
}
