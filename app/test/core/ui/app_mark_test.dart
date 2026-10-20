// Das App-Icon als Widget (HUM-259): die Faust in #14161C auf der gelben
// Kachel mit Verlauf, dieselbe Zeichnung wie `packaging/deb/humanitl.svg`.

import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ui/app_mark.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/shell/widgets/header_bar.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Das Icon in 256 px, so groß wie die View-Box des SVG: Ein Pixel hier ist
/// eine Einheit dort.
Future<ByteData> render() async {
  final ui.PictureRecorder recorder = ui.PictureRecorder();
  const AppMarkPainter().paint(Canvas(recorder), const Size.square(256));
  final ui.Picture picture = recorder.endRecording();
  final ui.Image image = await picture.toImage(256, 256);
  picture.dispose();
  final ByteData? bytes = await image.toByteData(
    format: ui.ImageByteFormat.rawStraightRgba,
  );
  image.dispose();
  return bytes!;
}

/// Der Pixel bei ([x], [y]) als Farbe.
Color pixel(ByteData bytes, int x, int y) {
  final int i = (y * 256 + x) * 4;
  return Color.fromARGB(
    bytes.getUint8(i + 3),
    bytes.getUint8(i),
    bytes.getUint8(i + 1),
    bytes.getUint8(i + 2),
  );
}

void main() {
  testWidgets('the_fist_is_black_on_the_yellow_tile', (
    WidgetTester tester,
  ) async {
    late ByteData bytes;
    await tester.runAsync(() async {
      bytes = await render();
    });

    // Außerhalb der abgerundeten Ecke ist nichts gemalt.
    expect(pixel(bytes, 10, 10).a, 0);
    // Mitten im Handballen: SVG (240, 450) nach der Transformation
    // translate(128 130) scale(0.34) translate(-224 -256).
    expect(pixel(bytes, 133, 196), appMarkFist);
    // Kachel oberhalb des Mittelfingers und unterhalb des Handgelenks.
    final Color top = pixel(bytes, 128, 20);
    final Color bottom = pixel(bytes, 128, 240);
    expect(top.a, 1.0);
    expect(bottom.a, 1.0);
    // Der Verlauf läuft von hell oben nach dunkel unten, beide nahe an ihren
    // Endfarben.
    expect(top.computeLuminance(), greaterThan(bottom.computeLuminance()));
    expect((top.r - appMarkTileTop.r).abs(), lessThan(0.03));
    expect((bottom.r - appMarkTileBottom.r).abs(), lessThan(0.03));
    expect((bottom.b - appMarkTileBottom.b).abs(), lessThan(0.03));
  });

  testWidgets('the_wordmark_carries_the_app_mark', (WidgetTester tester) async {
    await tester.pumpWidget(
      WidgetsApp(
        color: HColors.bg0,
        locale: const Locale('en'),
        localizationsDelegates: AppLocalizations.localizationsDelegates,
        supportedLocales: AppLocalizations.supportedLocales,
        builder: (BuildContext context, Widget? _) => HTheme(
          tokens: HTokens.dark,
          child: const Center(child: Wordmark(markSize: 24)),
        ),
      ),
    );

    final Finder mark = find.descendant(
      of: find.byType(Wordmark),
      matching: find.byType(AppMark),
    );
    expect(mark, findsOneWidget);
    expect(tester.getSize(mark), const Size.square(24));
    expect(find.text('Humanitl'), findsOneWidget);
  });
}
