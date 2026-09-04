// Gerüst der Rumpf-Tests: Bytes bauen, Funde bauen, eine Ansicht in ein Fenster
// hängen. Kein Daemon, kein Provider-Baum -- die vier Ansichten bekommen ihr
// Modell direkt, damit ein roter Test etwas über die Anzeige sagt und nicht
// über den Transport.

import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/l10n/l10n.dart';

/// [text] als UTF-8-Bytes.
Uint8List bytesOf(String text) => Uint8List.fromList(utf8.encode(text));

/// Ein Fund im Rumpf über `[start, end)`.
Finding bodyFinding({
  required int start,
  required int end,
  String kind = 'email',
  FindingTier tier = FindingTier.regex,
}) => Finding(
  kind: kind,
  location: FindingLocation.body,
  spanStart: start,
  spanEnd: end,
  tier: tier,
);

/// Die englischen Texte, ohne einen Baum zu bauen.
final AppLocalizations english = lookupAppLocalizations(const Locale('en'));

/// Hängt [child] in ein Fenster mit Theme, Overlay und Sprache.
Future<void> pumpBody(
  WidgetTester tester,
  Widget child, {
  Size size = const Size(800, 400),
  HThemeMode mode = HThemeMode.dark,
}) async {
  await tester.binding.setSurfaceSize(size);
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(
    Directionality(
      textDirection: TextDirection.ltr,
      child: MediaQuery(
        data: const MediaQueryData(),
        child: Localizations(
          locale: const Locale('en'),
          delegates: AppLocalizations.localizationsDelegates,
          child: HTheme(
            tokens: HThemeMode.dark == mode ? HTokens.dark : HTokens.light,
            child: Overlay(
              initialEntries: <OverlayEntry>[
                OverlayEntry(builder: (BuildContext context) => child),
              ],
            ),
          ),
        ),
      ),
    ),
  );
  await tester.pump();
}

/// Jeder `TextSpan` des Baums unter [finder], flach.
List<TextSpan> spansOf(WidgetTester tester, Finder finder) {
  final List<TextSpan> spans = <TextSpan>[];
  for (final Element element in finder.evaluate()) {
    final Widget widget = element.widget;
    if (widget is! RichText) {
      continue;
    }
    void walk(InlineSpan span) {
      if (span is TextSpan) {
        spans.add(span);
        for (final InlineSpan child in span.children ?? const <InlineSpan>[]) {
          walk(child);
        }
      }
    }

    walk(widget.text);
  }
  return spans;
}
