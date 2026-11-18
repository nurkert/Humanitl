// Eine Einstellungskomponente, zwei Wirte (HUM-535): Sheet und Reiter
// zeichnen dieselben Zeilen aus demselben Katalog, weil beide nur Rahmen um
// `SettingsPanel` sind. `Esc` schließt im Sheet wie bisher, im Reiter nichts;
// der Reiter zieht die Tastatur nicht an sich und hat keinen Knopf zum
// Schließen. Die übrigen Wege des Sheets prüft `settings_overlay_test.dart`.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/misc.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/client_providers.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/settings/settings_sheet.dart';
import 'package:humanitl/features/settings/settings_tab.dart';

import '../../goldens/support/golden_variants.dart';
import '../../harness/settings_harness.dart';
import 'change_setting_fix_test.dart' show focusIsWithin;
import 'settings_overlay_test.dart' show everything, page, query, type;

/// [host] allein über dem Fake [client], in einem Fenster der Shell.
Future<void> pumpHost(
  WidgetTester tester,
  Widget host, {
  FakeDaemonClient? client,
}) async {
  tester.view
    ..physicalSize = const Size(1280, 800)
    ..devicePixelRatio = 1;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    goldenFrame(
      variant: GoldenVariant.darkEn,
      background: GoldenVariant.darkEn.tokens.colors.bg0,
      overrides: <Override>[
        daemonClientProvider.overrideWithValue(client ?? everything()),
      ],
      child: host,
    ),
  );
  await tester.pump();
  await tester.pump();
  await tester.pumpAndSettle();
}

/// Die Zeilen der offenen Seite in ihrer Reihenfolge, je als Kennung und
/// erster Text (der Titel).
List<(String, String)> rows(WidgetTester tester) {
  final List<(String, String)> found = <(String, String)>[];
  for (final Element row
      in find
          .byWidgetPredicate(
            (Widget widget) => switch (widget.key) {
              ValueKey<String>(:final String value) => value.startsWith(
                'setting-row-',
              ),
              _ => false,
            },
          )
          .evaluate()) {
    final String id = (row.widget.key! as ValueKey<String>).value.substring(
      'setting-row-'.length,
    );
    final Text title = tester.widget<Text>(
      find
          .descendant(
            of: find.byKey(row.widget.key!),
            matching: find.byType(Text),
          )
          .first,
    );
    found.add((id, title.data ?? title.textSpan?.toPlainText() ?? ''));
  }
  return found;
}

/// Die Zeilen der Kategorie [category] im Wirt [host].
Future<List<(String, String)>> rowsOf(
  WidgetTester tester,
  Widget host,
  String category,
) async {
  await pumpHost(tester, host);
  await showSettingsCategory(tester, category);
  expect(page(category), findsOneWidget, reason: category);
  return rows(tester);
}

void main() {
  final TargetPlatformVariant desktops = TargetPlatformVariant(<TargetPlatform>{
    TargetPlatform.linux,
    TargetPlatform.macOS,
  });

  testWidgets('sheet and tab draw the same rows in the same order', (
    WidgetTester tester,
  ) async {
    for (final String category in <String>[
      SettingsCategory.requests,
      SettingsCategory.interface,
      SettingsCategory.sandbox,
    ]) {
      final List<(String, String)> sheet = await rowsOf(
        tester,
        SettingsSheet(onClose: () {}),
        category,
      );
      // Ein neuer Baum: Der Reiter erbt nichts vom Sheet.
      await tester.pumpWidget(const SizedBox());
      final List<(String, String)> tab = await rowsOf(
        tester,
        const SettingsTab(),
        category,
      );
      await tester.pumpWidget(const SizedBox());
      expect(sheet, isNotEmpty, reason: category);
      expect(
        sheet.every(((String, String) row) => row.$2.isNotEmpty),
        isTrue,
        reason: '$category: every row has its title',
      );
      expect(tab, sheet, reason: category);
    }
  }, variant: desktops);

  testWidgets('Esc in the tab empties the search and closes nothing', (
    WidgetTester tester,
  ) async {
    int dismissed = 0;
    await pumpHost(
      tester,
      // Ein Fenster um den Reiter, das `Esc` selbst nutzen würde: Der Reiter
      // reicht die Taste auf leerer Suche durch, statt sie zu schlucken.
      Shortcuts(
        shortcuts: const <ShortcutActivator, Intent>{
          SingleActivator(LogicalKeyboardKey.escape): DismissIntent(),
        },
        child: Actions(
          actions: <Type, Action<Intent>>{
            DismissIntent: CallbackAction<DismissIntent>(
              onInvoke: (DismissIntent _) => dismissed++,
            ),
          },
          child: const SettingsTab(),
        ),
      ),
    );
    expect(find.byKey(const Key('settings-tab')), findsOneWidget);
    expect(find.byKey(const Key('settings-close')), findsNothing);

    await type(tester, 'theme');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(query(tester), isEmpty, reason: 'the search goes first');
    expect(dismissed, 0, reason: 'emptying the search is all');

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byKey(const Key('settings-tab')), findsOneWidget);
    expect(find.byKey(const Key('settings-panel')), findsOneWidget);
    expect(dismissed, 1, reason: 'the window around the tab gets the key');
  }, variant: desktops);

  testWidgets('Esc in the sheet empties the search, then closes', (
    WidgetTester tester,
  ) async {
    int closed = 0;
    await pumpHost(tester, SettingsSheet(onClose: () => closed++));
    expect(find.byKey(const Key('settings-close')), findsOneWidget);

    await type(tester, 'theme');
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(query(tester), isEmpty);
    expect(closed, 0);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(closed, 1);
  }, variant: desktops);

  testWidgets('the sheet takes the keyboard, the tab does not', (
    WidgetTester tester,
  ) async {
    await pumpHost(tester, SettingsSheet(onClose: () {}));
    expect(focusIsWithin(const Key('settings-search')), isTrue);
    await tester.pumpWidget(const SizedBox());

    // Der Reiter neben einem Bedienelement des Startfensters, das die
    // Tastatur hat: Sie bleibt dort.
    final FocusNode outside = FocusNode(debugLabel: 'start-window');
    addTearDown(outside.dispose);
    await pumpHost(
      tester,
      Column(
        children: <Widget>[
          Focus(focusNode: outside, autofocus: true, child: const SizedBox()),
          const Expanded(child: SettingsTab()),
        ],
      ),
    );
    expect(outside.hasPrimaryFocus, isTrue);
    expect(focusIsWithin(const Key('settings-panel')), isFalse);
  }, variant: desktops);
}
