// Die Bereiche des Katalogs, deren Tasten in Widgets gebunden sind
// (HUM-420): Einstellungen, Listen, Befehlspalette, Dialoge und
// Bedienelemente.
//
// Für jeden Eintrag eines solchen Bereichs steht hier, wo seine Taste gilt;
// gedrückt wird die Taste aus dem Katalog, und das Widget muss sie
// annehmen. Ein Eintrag ohne Zeile hier ist rot, ein Eintrag, dessen Taste
// niemand bindet, ebenso. Die Bereiche mit einer Map prüft
// `test/core/shortcuts/shortcut_catalog_test.dart`, die Griffe des Terminals
// `test/features/sandbox/terminal_clipboard_test.dart`.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/shortcuts/shortcut_catalog.dart';
import 'package:humanitl/core/ui/h_resizable_panes.dart';
import 'package:humanitl/features/intercept/widgets/note_field.dart';

import '../goldens/support/golden_variants.dart';
import '../harness/app_harness.dart';
import '../harness/shortcut_press.dart';
import 'settings/model_servers_test.dart' show focusWithin, tapKey, twoServers;
import 'settings/settings_sheet_test.dart' show openSettings;

/// Was vor dem Drücken geschieht, und was danach gelten muss.
typedef Case = ({
  Future<void> Function(WidgetTester tester) setUp,
  void Function(WidgetTester tester)? check,
});

Case at(
  Future<void> Function(WidgetTester tester) setUp, [
  void Function(WidgetTester tester)? check,
]) => (setUp: setUp, check: check);

/// Die Einstellungen, die Zeile `home` mit der Tastatur.
Future<void> onServerRow(WidgetTester tester) async {
  await openSettings(tester, twoServers());
  await tapKey(tester, 'settings-server-home');
}

/// Dasselbe, und mit `Enter` in die Detailansicht.
Future<void> inServerDetail(WidgetTester tester) async {
  await onServerRow(tester);
  await tester.sendKeyEvent(LogicalKeyboardKey.enter);
  await tester.pumpAndSettle();
  expect(focusWithin(const Key('settings-server-name')), isTrue);
}

/// Die Namen der Serverzeilen, von oben.
List<String> serverRows(WidgetTester tester) {
  final List<(double, String)> rows = <(double, String)>[
    for (final String name in <String>['home', 'cloud'])
      (
        tester
            .getTopLeft(find.byKey(ValueKey<String>('settings-server-$name')))
            .dy,
        name,
      ),
  ]..sort(((double, String) a, (double, String) b) => a.$1.compareTo(b.$1));
  return <String>[for (final (double _, String name) in rows) name];
}

/// Die Zeile an [index] (von oben) mit der Tastatur.
Future<void> Function(WidgetTester) onServerRowAt(int index) =>
    (WidgetTester tester) async {
      await openSettings(tester, twoServers());
      await tapKey(tester, 'settings-server-${serverRows(tester)[index]}');
    };

/// Der Fokus steht auf der Zeile an [index].
void Function(WidgetTester) focusOnRow(int index) =>
    (WidgetTester tester) => expect(
      focusWithin(
        ValueKey<String>('settings-server-${serverRows(tester)[index]}'),
      ),
      isTrue,
      reason:
          'the keys of the list move the focus, not the default focus '
          'traversal',
    );

/// Was die Trennlinie zuletzt gemeldet hat.
List<double>? movedTo;

void intoDetail(WidgetTester tester) =>
    expect(focusWithin(const Key('settings-server-name')), isTrue);

/// Die Zeilen der Palette, von oben.
List<Widget> paletteRows(WidgetTester tester) => tester
    .widgetList(
      find.byWidgetPredicate(
        (Widget w) => w.runtimeType.toString() == '_PaletteRow',
      ),
    )
    .toList();

/// Welche Zeile der Palette gewählt ist.
int selectedPaletteRow(WidgetTester tester) =>
    paletteRows(tester)
        .indexWhere((Widget row) => (row as dynamic).selected as bool);

void dividerMoved(WidgetTester tester) =>
    expect(movedTo, isNotNull, reason: 'the arrow moved the divider');

Future<void> inPalette(WidgetTester tester) async {
  await pumpApp(tester, client: FakeDaemonClient.empty());
  await pressCtrl(tester, LogicalKeyboardKey.keyK);
  await tester.pumpAndSettle();
}

Future<void> inNoteField(WidgetTester tester) async {
  await tester.pumpWidget(
    goldenFrame(variant: GoldenVariant.darkEn, child: const NoteField()),
  );
  await tester.pumpAndSettle();
}

/// Zwei Bereiche um eine Trennlinie auf [axis], die Linie mit dem Fokus.
Future<void> Function(WidgetTester) onDivider(Axis axis) =>
    (WidgetTester tester) async {
      final FocusNode first = FocusNode();
      addTearDown(first.dispose);
      await tester.pumpWidget(
        goldenFrame(
          variant: GoldenVariant.darkEn,
          child: SizedBox(
            width: 400,
            height: 400,
            child: HResizablePanes(
              axis: axis,
              ratios: const <double>[0.5, 0.5],
              minWidths: const <double>[50, 50],
              splitterSemanticsLabel: 'resize',
              onRatiosChanged: (List<double> ratios) => movedTo = ratios,
              children: <Widget>[
                Focus(focusNode: first, child: const SizedBox.expand()),
                const SizedBox.expand(),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      movedTo = null;
      first.requestFocus();
      await tester.pump();
      await tester.sendKeyEvent(LogicalKeyboardKey.tab);
      await tester.pump();
    };

void main() {
  final TargetPlatformVariant linux = TargetPlatformVariant.only(
    TargetPlatform.linux,
  );

  final Map<ShortcutScope, Map<AppShortcut, List<Case>>> table =
      <ShortcutScope, Map<AppShortcut, List<Case>>>{
        ShortcutScope.settings: <AppShortcut, List<Case>>{
          AppShortcut.settingsSearch: <Case>[at(onServerRow)],
          AppShortcut.settingsClose: <Case>[
            at(
              (WidgetTester t) => openSettings(t, twoServers()),
              (WidgetTester t) => expect(
                find.byKey(const Key('settings-overlay')),
                findsNothing,
              ),
            ),
          ],
          AppShortcut.serverAdd: <Case>[at(onServerRow)],
          AppShortcut.serverSave: <Case>[at(inServerDetail)],
          AppShortcut.serverOpen: <Case>[
            at(onServerRow, intoDetail),
            at(onServerRow, intoDetail),
          ],
          // Ohne die Bindung der Zeile nähme die Vorgabe von `WidgetsApp` die
          // Leertaste als `ActivateIntent` und ginge in die Detailansicht.
          AppShortcut.serverChoose: <Case>[
            at(
              onServerRow,
              (WidgetTester t) => expect(
                focusWithin(const ValueKey<String>('settings-server-home')),
                isTrue,
              ),
            ),
          ],
        },
        ShortcutScope.lists: <AppShortcut, List<Case>>{
          AppShortcut.listNext: <Case>[at(onServerRowAt(0), focusOnRow(1))],
          AppShortcut.listPrevious: <Case>[at(onServerRowAt(1), focusOnRow(0))],
          AppShortcut.listFirst: <Case>[at(onServerRowAt(1), focusOnRow(0))],
          // Die letzte Zeile ist „Server hinzufügen“.
          AppShortcut.listLast: <Case>[
            at(
              onServerRowAt(0),
              (WidgetTester t) =>
                  expect(focusWithin(const Key('settings-server-add')), isTrue),
            ),
          ],
          // Ohne die eigene Bindung käme `Esc` beim Overlay an und schlösse
          // es: Geprüft wird deshalb, wohin der Fokus geht.
          AppShortcut.listBack: <Case>[
            at(
              inServerDetail,
              (WidgetTester t) => expect(
                focusWithin(const ValueKey<String>('settings-server-home')),
                isTrue,
              ),
            ),
          ],
        },
        ShortcutScope.palette: <AppShortcut, List<Case>>{
          // Pfeile und `Esc` nähmen sonst die Vorgaben von `WidgetsApp` und
          // des Modals: Geprüft wird, was die Palette daraus macht.
          AppShortcut.paletteNext: <Case>[
            at(inPalette, (WidgetTester t) => expect(selectedPaletteRow(t), 1)),
          ],
          AppShortcut.palettePrevious: <Case>[
            at(
              inPalette,
              (WidgetTester t) =>
                  expect(selectedPaletteRow(t), paletteRows(t).length - 1),
            ),
          ],
          AppShortcut.paletteRun: <Case>[at(inPalette), at(inPalette)],
          AppShortcut.paletteClose: <Case>[
            at(
              inPalette,
              (WidgetTester t) =>
                  expect(find.byKey(const Key('palette-input')), findsNothing),
            ),
          ],
        },
        ShortcutScope.controls: <AppShortcut, List<Case>>{
          AppShortcut.closeLayer: <Case>[
            at(
              inNoteField,
              (WidgetTester t) => expect(
                FocusManager.instance.primaryFocus?.context
                    ?.findAncestorWidgetOfExactType<NoteField>(),
                isNull,
                reason: 'Esc closes the note field and gives the keyboard back',
              ),
            ),
          ],
          // Erst die senkrechte, dann die waagerechte Achse, wie im Katalog.
          AppShortcut.dividerBack: <Case>[
            at(onDivider(Axis.vertical), dividerMoved),
            at(onDivider(Axis.horizontal), dividerMoved),
          ],
          AppShortcut.dividerForward: <Case>[
            at(onDivider(Axis.vertical), dividerMoved),
            at(onDivider(Axis.horizontal), dividerMoved),
          ],
        },
      };

  for (final MapEntry<ShortcutScope, Map<AppShortcut, List<Case>>> scope
      in table.entries) {
    test('every entry of ${scope.key.name} has a case, one per key', () {
      expect(
        scope.value.keys.toSet(),
        AppShortcut.of(scope.key).toSet(),
        reason: 'a new entry needs a place where its key is pressed',
      );
      for (final MapEntry<AppShortcut, List<Case>> entry
          in scope.value.entries) {
        expect(entry.value, hasLength(entry.key.activators.length));
      }
    });

    for (final MapEntry<AppShortcut, List<Case>> entry in scope.value.entries) {
      for (int i = 0; i < entry.value.length; i++) {
        final ShortcutActivator activator = entry.key.activators[i];
        testWidgets('bound_widget_answers: ${entry.key.name} '
            '(${activator.debugDescribeKeys()})', (WidgetTester tester) async {
          final Case c = entry.value[i];
          await c.setUp(tester);
          expect(
            await pressActivator(tester, activator),
            isTrue,
            reason: '${entry.key} is answered where it is bound',
          );
          await tester.pumpAndSettle();
          c.check?.call(tester);
        }, variant: linux);
      }
    }
  }
}
