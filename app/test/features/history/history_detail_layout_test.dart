// Wie viel Platz der Rumpf im History-Detail bekommt (HUM-153).
//
// Bei 1400 × 900 steht der Rumpf neben Kopf, Tabs und Kopfzeilen und zeigt
// Titel und mindestens zehn Zeilen seines Baums, ohne dass jemand scrollt.
// Im schmalen Detail bleibt alles untereinander, wie es war. Jeder Test hier
// wird rot, sobald das Detail die eine oder die andere Anordnung verliert.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/text/format.dart';
import 'package:humanitl/features/history/providers/history_detail.dart';
import 'package:humanitl/l10n/l10n.dart';

import 'harness.dart';
import 'long_body.dart';

/// Die englischen Texte, ohne einen Baum zu bauen.
final AppLocalizations _english = lookupAppLocalizations(const Locale('en'));

/// Der vierte aufgezeichnete Flow: ein POST, freigegeben, ohne Fund.
const FlowId _flow = FlowId('018f0004-0000-7000-8000-00000000000a');

/// Das Fenster, in dem das Ziel des Issues gilt.
const Size _window = Size(1400, 900);

/// Die Anordnung ist Layout und hängt nicht an der Plattform; gemessen wird
/// trotzdem auf der, für die das Programm gebaut ist.
final TargetPlatformVariant _linux = TargetPlatformVariant.only(
  TargetPlatform.linux,
);

/// Das Szenario mit dem langen Rumpf an [_flow], im Fenster [size] und mit
/// [textScaler], [_flow] ausgewählt und sein Rumpf zerlegt.
Future<void> _pumpLong(
  WidgetTester tester,
  Size size, {
  TextScaler textScaler = TextScaler.noScaling,
}) async {
  final FakeDaemonClient client = FakeDaemonClient.history(count: 24);
  recordLongJsonRequest(client, _flow);
  final ProviderContainer container = await pumpHistory(
    tester,
    client: client,
    size: size,
    textScaler: textScaler,
  );
  container.read(historySelectionProvider.notifier).select(_flow);
  for (int i = 0; i < 8; i++) {
    await tester.pump(const Duration(milliseconds: 200));
  }
}

/// Die Fläche, in der der Baum ohne Scrollen zu sehen ist: sein eigener
/// Rahmen, geschnitten mit dem Fenster. Das Fenster ist die Fläche, die der
/// Harness mit `setSurfaceSize` setzt, nicht `tester.view`.
Rect _treeWindow(WidgetTester tester) => tester
    .getRect(find.byKey(const Key('body-tree')))
    .intersect(Offset.zero & _window);

/// Wie viele Schlüsselzeilen des Baums ganz in [window] stehen.
int _keyRowsInside(WidgetTester tester, Rect window) {
  int inside = 0;
  for (final String key in longJsonKeys) {
    final Finder row = find.descendant(
      of: find.byKey(const Key('body-tree')),
      matching: find.text(key, findRichText: true),
    );
    for (final Element element in row.evaluate()) {
      final Rect rect = tester.getRect(find.byWidget(element.widget));
      if (window.top <= rect.top && rect.bottom <= window.bottom) {
        inside++;
      }
    }
  }
  return inside;
}

void main() {
  testWidgets(
    'at 1400 x 900 the body shows its title and ten tree lines unscrolled',
    (WidgetTester tester) async {
      await _pumpLong(tester, _window);

      final Rect visible = Offset.zero & _window;
      final String title = _english.interceptSectionBody(
        formatBytes(longJsonBody().length),
        'application/json',
      );
      expect(find.text(title), findsOneWidget);
      final Rect titleRect = tester.getRect(find.text(title));
      expect(
        visible.contains(titleRect.bottomRight),
        isTrue,
        reason: '$titleRect',
      );

      // Der Wurzelknoten ist die erste Zeile, jede Schlüsselzeile darunter
      // eine weitere. Neun davon plus die Wurzel sind zehn.
      final int keyRows = _keyRowsInside(tester, _treeWindow(tester));
      expect(keyRows + 1, greaterThanOrEqualTo(10), reason: '$keyRows keys');
    },
    variant: _linux,
  );

  testWidgets('the wide detail puts the body beside the head, not under it', (
    WidgetTester tester,
  ) async {
    await _pumpLong(tester, _window);

    final Rect head = tester.getRect(
      find.byKey(const Key('history-detail-head-column')),
    );
    final Rect body = tester.getRect(
      find.byKey(const Key('history-detail-body-column')),
    );
    // Beide Spalten beginnen an derselben Oberkante, der Rumpf rechts.
    expect(body.top, head.top);
    expect(body.left, greaterThan(head.right - 1));
    // Die Kopfzeilen bleiben links, unter den Tabs.
    final Rect accept = tester.getRect(find.text('accept'));
    expect(head.contains(accept.center), isTrue, reason: '$accept');
    final Rect tab = tester.getRect(
      find.byKey(const Key('history-tab-request')),
    );
    expect(head.contains(tab.center), isTrue, reason: '$tab');
  }, variant: _linux);

  testWidgets('a narrow detail keeps headers above the body', (
    WidgetTester tester,
  ) async {
    await _pumpLong(tester, const Size(1100, 900));

    expect(find.byKey(const Key('history-detail-body-column')), findsNothing);
    final Rect accept = tester.getRect(find.text('accept'));
    final Rect tree = tester.getRect(find.byKey(const Key('body-tree')));
    expect(tree.top, greaterThan(accept.bottom));
  }, variant: _linux);
  testWidgets('tab walks the head column before the body column', (
    WidgetTester tester,
  ) async {
    await _pumpLong(tester, _window);

    final Rect head = tester.getRect(
      find.byKey(const Key('history-detail-head-column')),
    );
    final Rect body = tester.getRect(
      find.byKey(const Key('history-detail-body-column')),
    );
    // Der Knoten des Request-Tabs: der nächste Focus über seinem Text.
    Focus.of(
      tester.element(
        find
            .descendant(
              of: find.byKey(const Key('history-tab-request')),
              matching: find.byType(Text),
            )
            .first,
      ),
    ).requestFocus();
    await tester.pump();

    // Von den Tabs aus kommen erst die übrigen Knoten der linken Spalte,
    // dann die des Rumpfs, und danach keiner mehr aus der linken Spalte:
    // Tabs, Kopfzeilen, Rumpf, wie im schmalen Detail.
    final List<String> walk = <String>[];
    for (int i = 0; i < 40; i++) {
      final Offset center = FocusManager.instance.primaryFocus!.rect.center;
      walk.add(
        head.contains(center)
            ? 'head'
            : body.contains(center)
            ? 'body'
            : 'other',
      );
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    }
    // Bis das Detail verlassen wird: ein Block links, dann ein Block Rumpf.
    final int leaves = walk.indexOf('other');
    final List<String> inside = leaves < 0 ? walk : walk.sublist(0, leaves);
    final int firstBody = inside.indexOf('body');
    expect(firstBody, greaterThan(0), reason: '$walk');
    expect(
      inside.sublist(0, firstBody).every((String s) => s == 'head'),
      isTrue,
      reason: '$walk',
    );
    expect(
      inside.sublist(firstBody).every((String s) => s == 'body'),
      isTrue,
      reason: '$walk',
    );
  }, variant: _linux);
  testWidgets('large text keeps headers above the body at 1400 x 900', (
    WidgetTester tester,
  ) async {
    // Das Textmaß wächst mit der Schrift: bei 1,5 braucht das Nebeneinander
    // knapp 1950 Pixel, mehr als das Fenster hat.
    await _pumpLong(tester, _window, textScaler: const TextScaler.linear(1.5));

    // Untereinander ist bei dieser Schrift so wenig Platz, dass die
    // Kopfzeilen nicht alle gebaut werden; gemessen wird die Anordnung.
    expect(find.byKey(const Key('history-detail-body-column')), findsNothing);
    expect(find.byKey(const Key('history-detail-head-column')), findsNothing);
    expect(find.byKey(const Key('history-tab-request')), findsOneWidget);
  }, variant: _linux);
}
