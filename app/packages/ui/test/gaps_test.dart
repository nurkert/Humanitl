import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl_ui/humanitl_ui.dart';

import 'harness.dart';

/// Ein Intent, den nur der Bildschirm bindet — die Sonde für die Frage, ob
/// eine Pfeiltaste an einem fokussierten Control vorbeikommt.
class _ScreenIntent extends Intent {
  const _ScreenIntent();
}

/// Die Lücken, die `docs/UX.md` 9 aufzählt, als Verhalten geprüft.
///
/// Was hier steht, hat je einen Screen dazu gebracht, sich ein eigenes Widget
/// zu bauen: der Fokusring, die Zeile mit Glyph, Fokus und Aktionsslot, das
/// Skelett, die drei Formular-Controls und die fünf Glyphen.
void main() {
  /// Die Füllung der Zeile: die Farbe ihres animierten Containers.
  Color rowFill(WidgetTester tester) => tester
      .widget<HAnimatedFill>(
        find.descendant(
          of: find.byType(HRow),
          matching: find.byType(HAnimatedFill),
        ),
      )
      .color;

  group('HFocusRing', () {
    testWidgets('keeps two pixels of surface between control and ring', (
      WidgetTester tester,
    ) async {
      // Der Ring ist der Akzent, und der Primärbutton ist mit dem Akzent
      // gefüllt: läge der Ring auf seiner Kante, stünde er bei 1,00:1 gegen
      // die eigene Füllung, und ein Tastaturnutzer sähe keinen Unterschied
      // (`docs/UX.md` 6). Die zwei Pixel Fläche dazwischen sind der Kontrast.
      await tester.pumpWidget(
        harness(
          keyboard(
            HButton(
              variant: HButtonVariant.primary,
              preview: HButtonPreview.focused,
              onPressed: () {},
              child: const Text('Allow'),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      final Size ring = tester.getSize(find.byType(HFocusRing));
      final Size fill = tester.getSize(find.byType(HAnimatedFill));
      expect(HFocusRing.gap, greaterThanOrEqualTo(HFocusRing.width));
      expect(
        ring.height - fill.height,
        2 * (HFocusRing.width + HFocusRing.gap),
      );
      expect(ring.width - fill.width, 2 * (HFocusRing.width + HFocusRing.gap));
      // Und nur dort: der Ring wächst über einer Füllung, gegen die er
      // verschwände, nicht über jeder.
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        final Color accent = tokens.colors.accent;
        expect(HFocusRing.needsGap(tokens.colors.accentFill, accent), isTrue);
        expect(HFocusRing.needsGap(tokens.colors.bg2, accent), isFalse);
        expect(HFocusRing.needsGap(null, accent), isFalse);
        // Eine durchscheinende Füllung ist nicht der Nachbar des Rings.
        expect(
          HFocusRing.needsGap(
            accent.withValues(alpha: HColors.fillPressedAlpha),
            accent,
          ),
          isFalse,
        );
        expect(HFocusRing.reservedFor(null, accent), HFocusRing.width);
        expect(
          HFocusRing.reservedFor(tokens.colors.accentFill, accent),
          HFocusRing.width + HFocusRing.gap,
        );
      }
      expect(tester.takeException(), isNull);
    });

    testWidgets('reserves its two pixels and paints only when focused', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        harness(
          keyboard(
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: const <Widget>[
                HFocusRing(
                  key: Key('off'),
                  visible: false,
                  child: SizedBox.square(dimension: 20),
                ),
                HFocusRing(
                  key: Key('on'),
                  visible: true,
                  child: SizedBox.square(dimension: 20),
                ),
                HFocusRing.inline(
                  key: Key('inline'),
                  visible: true,
                  child: SizedBox.square(dimension: 20),
                ),
              ],
            ),
          ),
        ),
      );
      // Der Platz ist reserviert, ob der Ring zu sehen ist oder nicht; sonst
      // verschöbe der Fokus das Control (`docs/UX.md` 6).
      expect(
        tester.getSize(find.byKey(const Key('off'))).height,
        20 + 2 * HFocusRing.width,
      );
      expect(
        tester.getSize(find.byKey(const Key('on'))).height,
        20 + 2 * HFocusRing.width,
      );
      // Die Inline-Form reserviert nichts: eine Zeile von Rand zu Rand hat
      // kein Außen.
      expect(tester.getSize(find.byKey(const Key('inline'))).height, 20);

      CustomPaint paintOf(String key) => tester.widget<CustomPaint>(
        find.descendant(
          of: find.byKey(Key(key)),
          matching: find.byType(CustomPaint),
        ),
      );
      expect(paintOf('off').foregroundPainter, isNull);
      expect(paintOf('on').foregroundPainter, isNotNull);
      expect(HFocusRing.width, 2);
    });
  });

  group('HRow', () {
    testWidgets('takes focus, shows the ring and reveals the action slot', (
      WidgetTester tester,
    ) async {
      int taps = 0;
      bool focused = false;
      final FocusNode node = FocusNode();
      addTearDown(node.dispose);
      await tester.pumpWidget(
        harness(
          keyboard(
            HRow(
              state: HFlowState.held,
              focusNode: node,
              onTap: () => taps++,
              onFocusChange: (bool value) => focused = value,
              stateGlyph: const HStateGlyph(state: HFlowState.held),
              title: const Text('registry.npmjs.org'),
              actionSlot: const HBadge(key: Key('slot'), text: 'B'),
              semanticsLabel: 'held flow',
              semanticsValue: '1:47 left',
            ),
          ),
        ),
      );
      // Bei Ruhe ist der Slot reserviert und leer.
      expect(find.byKey(const Key('slot')), findsNothing);
      expect(
        tester.getSize(find.byType(HRow)).height,
        HSize.row,
        reason: 'the reserved slot does not change the height',
      );

      node.requestFocus();
      await tester.pumpAndSettle();
      expect(focused, isTrue);
      expect(
        tester
            .widget<HFocusRing>(
              find.descendant(
                of: find.byType(HRow),
                matching: find.byType(HFocusRing),
              ),
            )
            .visible,
        isTrue,
      );
      // Fokus deckt dieselbe Aktion auf wie Hover (`docs/UX.md` 3.4).
      expect(find.byKey(const Key('slot')), findsOneWidget);

      // Die Taste macht dasselbe wie der Zeiger (`docs/UX.md` 5.1).
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(taps, 1);
      expect(tester.takeException(), isNull);
    });

    testWidgets('hover fills bg2 and reveals the slot, selection fills bg3', (
      WidgetTester tester,
    ) async {
      final HTokens tokens = HTokens.dark;
      await tester.pumpWidget(
        harness(
          keyboard(
            HRow(
              state: HFlowState.held,
              onTap: () {},
              title: const Text('registry.npmjs.org'),
              actionSlot: const HBadge(key: Key('slot'), text: 'B'),
              semanticsLabel: 'held flow',
            ),
          ),
        ),
      );
      final TestGesture gesture = await tester.createGesture(
        kind: PointerDeviceKind.mouse,
      );
      await gesture.addPointer(location: const Offset(700, 500));
      addTearDown(gesture.removePointer);
      await tester.pump();
      expect(rowFill(tester), isNot(tokens.colors.bg2));

      await gesture.moveTo(tester.getCenter(find.byType(HRow)));
      await tester.pumpAndSettle();
      expect(rowFill(tester), tokens.colors.bg2);
      expect(find.byKey(const Key('slot')), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the selection replaces the state rail over all four pixels', (
      WidgetTester tester,
    ) async {
      final HTokens tokens = HTokens.dark;
      Widget build({required bool selected, required bool inSelection}) =>
          harness(
            keyboard(
              HRow(
                state: HFlowState.held,
                selected: selected,
                inSelection: inSelection,
                tintedRail: true,
                onTap: () {},
                title: const Text('registry.npmjs.org'),
                semanticsLabel: 'held flow',
              ),
            ),
          );
      Color railColor() {
        final ColoredBox box = tester.widget<ColoredBox>(
          find
              .descendant(
                of: find.byType(HRow),
                matching: find.byType(ColoredBox),
              )
              .first,
        );
        return box.color;
      }

      Size railSize() => tester.getSize(
        find
            .descendant(
              of: find.byType(HRow),
              matching: find.byType(ColoredBox),
            )
            .first,
      );

      await tester.pumpWidget(build(selected: false, inSelection: false));
      await tester.pumpAndSettle();
      // Die ruhende Rail der Queue ist eine Zehn-Prozent-Tönung, die eine
      // benannte Ausnahme von 3:1 (`docs/UX.md` 3.3, Regel 10).
      expect(railColor().a, closeTo(HColors.tintAlpha, 1e-6));
      expect(railSize(), const Size(HSize.stateRail, HSize.row));

      await tester.pumpWidget(build(selected: true, inSelection: false));
      await tester.pumpAndSettle();
      expect(railColor(), tokens.colors.accent);
      expect(
        railSize().width,
        HSize.stateRail,
        reason: 'the selection replaces the rail, it does not overlay it',
      );

      // Ein Mitglied ohne Cursor trägt dieselbe Rail und keine Füllung.
      await tester.pumpWidget(build(selected: false, inSelection: true));
      await tester.pumpAndSettle();
      expect(railColor(), tokens.colors.accent);
      expect(rowFill(tester).a, 0);
      expect(tester.takeException(), isNull);
    });

    testWidgets('carries the three densities and never animates its height', (
      WidgetTester tester,
    ) async {
      for (final double density in <double>[
        HSize.row,
        HSize.rowHistory,
        HSize.rowBody,
      ]) {
        await tester.pumpWidget(
          harness(
            keyboard(
              HRow(
                state: HFlowState.allowed,
                minHeight: density,
                title: const Text('api.github.com'),
                semanticsLabel: 'flow',
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.getSize(find.byType(HRow)).height, density);
      }

      // Ein Zustandswechsel ändert die Höhe in keinem Frame.
      Widget build({required bool selected}) => harness(
        keyboard(
          HRow(
            state: HFlowState.held,
            selected: selected,
            onTap: () {},
            title: const Text('api.github.com'),
            semanticsLabel: 'flow',
          ),
        ),
      );
      await tester.pumpWidget(build(selected: false));
      await tester.pumpAndSettle();
      expect(tester.getSize(find.byType(HRow)).height, HSize.row);
      await tester.pumpWidget(build(selected: true));
      for (int frame = 0; frame < 4; frame++) {
        await tester.pump(const Duration(milliseconds: 40));
        expect(
          tester.getSize(find.byType(HRow)).height,
          HSize.row,
          reason: 'frame $frame',
        );
      }
    });
  });

  group('focusable controls', () {
    testWidgets('HIconButton takes focus and fires on Enter', (
      WidgetTester tester,
    ) async {
      int taps = 0;
      final FocusNode node = FocusNode();
      addTearDown(node.dispose);
      await tester.pumpWidget(
        harness(
          keyboard(
            Focus(
              focusNode: node,
              child: HIconButton(
                glyph: HGlyph.trash,
                autofocus: true,
                onPressed: () => taps++,
                semanticsLabel: 'delete',
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(taps, 1);
      expect(
        tester.widget<HFocusRing>(find.byType(HFocusRing)).visible,
        isTrue,
      );
      expect(
        tester.getSize(find.byType(HIconButton)).height,
        greaterThanOrEqualTo(HSize.hitMin),
      );
    });

    testWidgets('a tappable HBadge is a focus stop', (
      WidgetTester tester,
    ) async {
      int taps = 0;
      final FocusNode node = FocusNode();
      addTearDown(node.dispose);
      await tester.pumpWidget(
        harness(
          keyboard(
            HBadge(text: '3 findings', focusNode: node, onTap: () => taps++),
          ),
        ),
      );
      node.requestFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pump();
      expect(taps, 1);
      expect(
        tester.widget<HFocusRing>(find.byType(HFocusRing)).visible,
        isTrue,
      );
    });

    testWidgets('both halves of HPill are focus stops', (
      WidgetTester tester,
    ) async {
      int left = 0;
      int right = 0;
      await tester.pumpWidget(
        harness(
          keyboard(
            HPill(
              left: const Text('Send'),
              onLeft: () => left++,
              onRight: () => right++,
              leftSemanticsLabel: 'send once',
              rightSemanticsLabel: 'scope',
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      // Zwei Fokusstopps, einer je Hälfte.
      expect(
        find.descendant(
          of: find.byType(HPill),
          matching: find.byType(FocusableActionDetector),
        ),
        findsNWidgets(2),
      );
      final FocusScopeNode scope = FocusScope.of(
        tester.element(find.byType(HPill)),
      );
      // Einzeln geprüft und in der Leserichtung: `left + right == 2` bestünde
      // auch, wenn eine Hälfte zweimal geantwortet hätte und die andere gar
      // nicht.
      scope.nextFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(left, 1, reason: 'der erste Stopp ist die linke Hälfte');
      expect(right, 0);
      scope.nextFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.enter);
      await tester.pump();
      expect(left, 1);
      expect(right, 1, reason: 'der zweite Stopp ist die rechte Hälfte');
      expect(tester.takeException(), isNull);
    });
  });

  group('ein Zustandsspeicher', () {
    testWidgets(
      'der Ring kommt vom Knoten, auch ohne Tasten- oder Mausereignis',
      (WidgetTester tester) async {
        // `Clickable` meldet den Fokus über den Highlight-Modus von Flutter,
        // und der steht bis zum ersten Tasten- oder Mausereignis der Sitzung
        // auf `touch`. Käme der Ring von dort, bliebe er hier aus. Auf dem
        // Desktop ist er nie optional (`docs/UX.md` 6).
        final FocusNode node = FocusNode(debugLabel: 'button');
        addTearDown(node.dispose);
        await tester.pumpWidget(
          harness(
            HButton(
              focusNode: node,
              onPressed: () {},
              child: const Text('Send'),
            ),
          ),
        );
        expect(
          tester.widget<HFocusRing>(find.byType(HFocusRing)).visible,
          isFalse,
        );
        node.requestFocus();
        await tester.pumpAndSettle();
        expect(
          tester.widget<HFocusRing>(find.byType(HFocusRing)).visible,
          isTrue,
        );
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets('ein Control, das erlischt, baut nicht mitten im Aufbau neu', (
      WidgetTester tester,
    ) async {
      // `Clickable` trägt `disabled` in seinem `initState` und in seinem
      // `didUpdateWidget` in den geteilten Zustandsspeicher — beides läuft
      // mitten im Aufbau des Baumes. Ein Hörer, der daraufhin `setState`
      // ruft, bricht dort ab. Deshalb liest dieses Paket `disabled` aus
      // seinem eigenen Feld und baut nur neu, wenn sich die gezeichnete
      // Menge wirklich ändert.
      Widget build({required bool enabled}) => harness(
        Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            HButton(
              onPressed: enabled ? () {} : null,
              child: const Text('Send'),
            ),
            HIconButton(
              glyph: HGlyph.close,
              onPressed: enabled ? () {} : null,
              semanticsLabel: 'close',
            ),
            HCheckbox(
              label: 'keep',
              value: true,
              enabled: enabled,
              onChanged: (bool value) {},
            ),
          ],
        ),
      );
      await tester.pumpWidget(build(enabled: true));
      await tester.pumpAndSettle();
      await tester.pumpWidget(build(enabled: false));
      await tester.pumpAndSettle();
      await tester.pumpWidget(build(enabled: true));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });
  });

  group('die Pfeiltasten gehören dem Bildschirm', () {
    testWidgets(
      'eine fokussierte Zeile und ein fokussierter Knopf lassen sie durch',
      (WidgetTester tester) async {
        // `Clickable` bindet unter jedem Control vier Pfeiltasten auf
        // `DirectionalFocusIntent`, und das innerste `Shortcuts` gewinnt. Ohne
        // die Gegenbindung erreichte `ArrowDown` den Bildschirm nie, solange
        // eine Zeile oder ein Knopf den Fokus hält — und die Warteschlange ist
        // ein einziger Fokusstopp mit Navigation darin (`docs/UX.md` 5.2).
        int reached = 0;
        final FocusNode row = FocusNode(debugLabel: 'row');
        addTearDown(row.dispose);
        final FocusNode button = FocusNode(debugLabel: 'button');
        addTearDown(button.dispose);
        await tester.pumpWidget(
          harness(
            keyboard(
              Shortcuts(
                shortcuts: const <ShortcutActivator, Intent>{
                  SingleActivator(LogicalKeyboardKey.arrowDown):
                      _ScreenIntent(),
                },
                child: Actions(
                  actions: <Type, Action<Intent>>{
                    _ScreenIntent: CallbackAction<_ScreenIntent>(
                      onInvoke: (_ScreenIntent intent) {
                        reached++;
                        return null;
                      },
                    ),
                  },
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      HRow(
                        state: HFlowState.held,
                        focusNode: row,
                        onTap: () {},
                        title: const Text('api.github.com'),
                        semanticsLabel: 'flow',
                      ),
                      HButton(
                        focusNode: button,
                        onPressed: () {},
                        child: const Text('Allow'),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ),
        );

        row.requestFocus();
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
        expect(reached, 1, reason: 'die Zeile hält die Taste fest');

        button.requestFocus();
        await tester.pumpAndSettle();
        await tester.sendKeyEvent(LogicalKeyboardKey.arrowDown);
        await tester.pump();
        expect(reached, 2, reason: 'der Knopf hält die Taste fest');
        expect(tester.takeException(), isNull);
      },
    );
  });

  group('form controls', () {
    testWidgets('HTextField edits, shows its hint and rings on focus', (
      WidgetTester tester,
    ) async {
      final TextEditingController controller = TextEditingController();
      addTearDown(controller.dispose);
      final FocusNode node = FocusNode();
      addTearDown(node.dispose);
      String? changed;
      await tester.pumpWidget(
        harness(
          overlay: true,
          keyboard(
            SizedBox(
              width: 240,
              child: HTextField(
                controller: controller,
                focusNode: node,
                semanticsLabel: 'host pattern',
                hint: '**.npmjs.org',
                onChanged: (String value) => changed = value,
              ),
            ),
          ),
        ),
      );
      expect(find.text('**.npmjs.org'), findsOneWidget);
      expect(placeholderVisible(tester, '**.npmjs.org'), isTrue);
      node.requestFocus();
      await tester.pumpAndSettle();
      // Der Ring dieses einen Controls kommt aus der Bibliothek: `TextField`
      // bringt ihn fest eingebaut mit, und zwei Ringe übereinander sind einer
      // zu viel. Seine Maße sind unsere (`HTheme`, `FocusOutlineTheme`).
      expect(libraryFocusRing(tester, find.byType(HTextField)), isTrue);
      await tester.enterText(find.byType(EditableText), 'api.github.com');
      await tester.pump();
      expect(changed, 'api.github.com');
      // Der Platzhalter weicht dem ersten Zeichen. Die Bibliothek lässt ihn
      // im Baum stehen und blendet ihn aus, damit das Feld beim ersten
      // Zeichen nicht seine Höhe wechselt.
      expect(placeholderVisible(tester, '**.npmjs.org'), isFalse);
      expect(tester.takeException(), isNull);
    });

    testWidgets('HSegmented chooses one, HChoiceChips any number', (
      WidgetTester tester,
    ) async {
      String chosen = 'allow';
      final Set<String> toggled = <String>{'GET'};
      await tester.pumpWidget(
        harness(
          keyboard(
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                HSegmented<String>(
                  selected: chosen,
                  onSelect: (String value) => chosen = value,
                  options: const <HSegmentOption<String>>[
                    HSegmentOption<String>(value: 'allow', label: 'allow'),
                    HSegmentOption<String>(value: 'block', label: 'block'),
                  ],
                ),
                HChoiceChips<String>(
                  selected: toggled,
                  onToggle: (String value) {
                    if (!toggled.remove(value)) {
                      toggled.add(value);
                    }
                  },
                  options: const <HSegmentOption<String>>[
                    HSegmentOption<String>(value: 'GET', label: 'GET'),
                    HSegmentOption<String>(value: 'POST', label: 'POST'),
                  ],
                ),
              ],
            ),
          ),
        ),
      );
      await tester.tap(find.text('block'));
      await tester.pump();
      expect(chosen, 'block');
      await tester.tap(find.text('POST'));
      await tester.pump();
      expect(toggled, <String>{'GET', 'POST'});
      for (final Element element in find.byType(HSegment<String>).evaluate()) {
        expect(
          (element.renderObject! as RenderBox).size.height,
          greaterThanOrEqualTo(HSize.hitMin),
        );
      }
    });

    testWidgets('HCheckbox toggles with the keyboard and carries its hint', (
      WidgetTester tester,
    ) async {
      bool value = false;
      final FocusNode node = FocusNode();
      addTearDown(node.dispose);
      await tester.pumpWidget(
        harness(
          keyboard(
            HCheckbox(
              label: 'Keep the rule after the session ends',
              hint:
                  'A rule that outlives the session is one nobody sees again.',
              value: value,
              focusNode: node,
              onChanged: (bool next) => value = next,
            ),
          ),
        ),
      );
      expect(
        find.text('A rule that outlives the session is one nobody sees again.'),
        findsOneWidget,
      );
      node.requestFocus();
      await tester.pumpAndSettle();
      await tester.sendKeyEvent(LogicalKeyboardKey.space);
      await tester.pump();
      expect(value, isTrue);
    });
  });

  group('disabled', () {
    testWidgets('a dead control looks dead: fg2, never the accent', (
      WidgetTester tester,
    ) async {
      // `docs/UX.md` 6 hält `fg2` für wirklich deaktivierte Controls frei.
      // Kästchen und Segment sahen aus wie ein aktives Control.
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        await tester.pumpWidget(
          harness(
            brightness: tokens.brightness,
            Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                HCheckbox(
                  label: 'off',
                  hint: 'costs nothing',
                  value: true,
                  enabled: false,
                  onChanged: (bool value) {},
                ),
                HSegmented<int>(
                  selected: 1,
                  enabled: false,
                  onSelect: (int value) {},
                  options: const <HSegmentOption<int>>[
                    HSegmentOption<int>(value: 1, label: 'session'),
                  ],
                ),
              ],
            ),
          ),
        );
        await tester.pumpAndSettle();
        for (final String text in <String>['off', 'costs nothing', 'session']) {
          expect(
            paintedTextColor(tester, text),
            tokens.colors.fg2,
            reason: '${tokens.brightness.name} $text',
          );
        }
        // Und die Fläche des Hakens trägt nicht den Akzent: der gehört dem,
        // was man anfassen kann (3.3).
        final BoxDecoration tick =
            tester
                    .widget<DecoratedBox>(
                      find
                          .descendant(
                            of: find.byType(HCheckbox),
                            matching: find.byType(DecoratedBox),
                          )
                          .first,
                    )
                    .decoration
                as BoxDecoration;
        expect(tick.color, tokens.colors.fg2);
        expect(tick.color, isNot(tokens.colors.accent));
        expect(tick.color, isNot(tokens.colors.accentFill));
      }
      expect(tester.takeException(), isNull);
    });

    testWidgets('an active checkbox fills with accentFill, not with accent', (
      WidgetTester tester,
    ) async {
      // Dieselbe Trennung wie beim Primärbutton: Weiß auf dem hellen Akzent
      // misst 3,73:1 (`docs/UX.md` 6).
      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        await tester.pumpWidget(
          harness(
            brightness: tokens.brightness,
            HCheckbox(label: 'on', value: true, onChanged: (bool value) {}),
          ),
        );
        await tester.pumpAndSettle();
        final BoxDecoration tick =
            tester
                    .widget<DecoratedBox>(
                      find
                          .descendant(
                            of: find.byType(HCheckbox),
                            matching: find.byType(DecoratedBox),
                          )
                          .first,
                    )
                    .decoration
                as BoxDecoration;
        expect(tick.color, tokens.colors.accentFill);
        expect(
          HColorDerivation.contrast(tokens.colors.onAccent, tick.color!),
          greaterThanOrEqualTo(HColorDerivation.textMinContrast),
        );
      }
      expect(tester.takeException(), isNull);
    });
  });

  group('das Kästchen', () {
    testWidgets('malt seinen Rahmen in vier Zuständen aus unseren Token', (
      WidgetTester tester,
    ) async {
      // Die Komponente der Bibliothek rechnet
      // `enabled ?? onChanged != null`. Weil hier niemand entscheidet —
      // `IgnorePointer` und `ExcludeFocus` liegen darum —, ergäbe das ohne
      // ausdrückliches `enabled: true` immer `false`, und sie malte den Zweig
      // `!enabled ? colorScheme.muted`: **jedes** nicht angehakte Kästchen
      // stünde dann in `bg2`, also in der Farbe eines toten. Geprüft wird
      // deshalb die gemalte Rahmenfarbe und nicht, dass es das Widget gibt.
      BoxDecoration tick() =>
          tester
                  .widget<DecoratedBox>(
                    find
                        .descendant(
                          of: find.byType(HCheckbox),
                          matching: find.byType(DecoratedBox),
                        )
                        .first,
                  )
                  .decoration
              as BoxDecoration;

      for (final HTokens tokens in <HTokens>[HTokens.dark, HTokens.light]) {
        for (final bool enabled in <bool>[true, false]) {
          for (final bool value in <bool>[false, true]) {
            await tester.pumpWidget(
              harness(
                brightness: tokens.brightness,
                HCheckbox(
                  label: 'keep',
                  value: value,
                  enabled: enabled,
                  onChanged: (bool next) {},
                ),
              ),
            );
            await tester.pumpAndSettle();
            final Color border = tick().border!.top.color;
            final String where =
                '${tokens.brightness.name} enabled=$enabled value=$value';
            if (!enabled) {
              // Deaktiviert heißt sichtbar deaktiviert: `fg2`, die Stufe, die
              // `docs/UX.md` 6 dafür freihält.
              expect(border, tokens.colors.fg2, reason: where);
            } else if (value) {
              expect(border, tokens.colors.accentFill, reason: where);
            } else {
              expect(border, tokens.colors.lineStrong, reason: where);
            }
            expect(
              border,
              isNot(tokens.colors.bg2),
              reason: '$where: ein lebendes Kästchen sieht nicht tot aus',
            );
          }
        }
      }
      expect(tester.takeException(), isNull);
    });
  });

  group('reduzierte Bewegung', () {
    testWidgets('a fill keeps its duration when animations are off', (
      WidgetTester tester,
    ) async {
      // `AnimatedContainer` baut seinen Controller ohne `animationBehavior`
      // und stünde nach 6 ms voll da; `docs/UX.md` 2.10 nennt die
      // Tastenfüllung namentlich unter dem, was seine Dauer behält.
      tester.binding.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(
        tester.binding.platformDispatcher.clearAccessibilityFeaturesTestValue,
      );
      final HTokens tokens = HTokens.dark;
      Widget button(HButtonPreview? preview) => harness(
        HButton(preview: preview, onPressed: () {}, child: const Text('Send')),
      );
      Color painted() => paintedFill(tester, find.byType(HButton));

      await tester.pumpWidget(button(null));
      await tester.pumpAndSettle();
      expect(painted(), tokens.colors.bg2);
      await tester.pumpWidget(button(HButtonPreview.pressed));
      await tester.pump();
      await tester.pump(HMotion.press ~/ 10);
      expect(
        painted(),
        isNot(tokens.colors.lineStrong),
        reason: 'nach 12 ms ist die Füllung unterwegs, nicht angekommen',
      );
      await tester.pump(HMotion.press);
      expect(painted(), tokens.colors.lineStrong);
      expect(tester.takeException(), isNull);
    });
  });

  group('waiting', () {
    testWidgets('nothing under 150 ms, then the skeleton, then at least 400', (
      WidgetTester tester,
    ) async {
      Widget build({required bool loading}) => harness(
        keyboard(
          HWait(
            loading: loading,
            skeleton: const HSkeleton(rows: 3),
            child: const Text('three rules matched'),
          ),
        ),
      );
      await tester.pumpWidget(build(loading: true));
      await tester.pump(const Duration(milliseconds: 100));
      // Unter der Schwelle passiert nichts: eine Anzeige, die kürzer als eine
      // Reaktionszeit sichtbar ist, wird als Flackern gelesen.
      expect(find.byType(HSkeleton), findsNothing);
      expect(find.text('three rules matched'), findsOneWidget);

      await tester.pump(const Duration(milliseconds: 60));
      expect(find.byType(HSkeleton), findsOneWidget);
      expect(find.text('three rules matched'), findsNothing);

      // Die Antwort kommt sofort danach; das Skelett bleibt seine
      // Mindeststandzeit stehen.
      await tester.pumpWidget(build(loading: false));
      await tester.pump(const Duration(milliseconds: 100));
      expect(find.byType(HSkeleton), findsOneWidget);
      await tester.pump(const Duration(milliseconds: 320));
      expect(find.byType(HSkeleton), findsNothing);
      expect(find.text('three rules matched'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('the skeleton stands in the density of the rows to come', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        harness(
          keyboard(const HSkeleton(rows: 4, rowHeight: HSize.rowHistory)),
        ),
      );
      expect(
        tester.getSize(find.byType(HSkeleton)).height,
        4 * HSize.rowHistory,
      );
      // Keine Bewegung: das Skelett ist gezeichnet, nicht animiert.
      expect(find.byType(AnimatedContainer), findsNothing);
      expect(find.byType(HAnimatedFill), findsNothing);
      expect(find.byType(FadeTransition), findsNothing);
      expect(find.byType(HHairline), findsNWidgets(4));
    });
  });

  group('HModal', () {
    testWidgets('catches the focus and answers Escape', (
      WidgetTester tester,
    ) async {
      int dismissed = 0;
      int behind = 0;
      await tester.pumpWidget(
        harness(
          keyboard(
            Stack(
              children: <Widget>[
                // Der Screen dahinter: `Tab` darf ihn nicht erreichen.
                HButton(onPressed: () => behind++, child: const Text('behind')),
                SizedBox(
                  height: 300,
                  child: HModal(
                    title: const Text('Delete forever rule?'),
                    onDismiss: () => dismissed++,
                    scrimSemanticsLabel: 'dismiss',
                    actions: <Widget>[
                      HButton(onPressed: () {}, child: const Text('stay')),
                      HButton(onPressed: () {}, child: const Text('delete')),
                    ],
                    child: const Text('The rule allows every request.'),
                  ),
                ),
              ],
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      // Der Fokus liegt im Modal, nicht im Screen dahinter
      // (`docs/UX.md` 9, Punkt 16), und er bleibt dort.
      expect(find.byType(FocusScope), findsWidgets);
      final FocusScopeNode scope = FocusScope.of(
        tester.element(find.text('stay')),
      );
      expect(scope, isNot(FocusScope.of(tester.element(find.text('behind')))));
      for (int i = 0; i < 6; i++) {
        scope.nextFocus();
        await tester.pumpAndSettle();
        bool inside = false;
        FocusManager.instance.primaryFocus?.context?.visitAncestorElements((
          Element element,
        ) {
          if (element.widget is HModal) {
            inside = true;
            return false;
          }
          return true;
        });
        expect(inside, isTrue, reason: 'Sprung $i verlässt das Modal');
      }
      expect(behind, 0);
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(dismissed, 1);
      expect(tester.takeException(), isNull);
    });
  });

  group('HSheet', () {
    testWidgets('keeps the focus and answers Escape', (
      WidgetTester tester,
    ) async {
      int closed = 0;
      int behind = 0;
      await tester.pumpWidget(
        harness(
          keyboard(
            SizedBox(
              height: 300,
              child: Row(
                children: <Widget>[
                  // Der Bildschirm hinter dem Blatt: `Tab` darf ihn nicht
                  // erreichen (`docs/UX.md` 5.1 und 9).
                  HButton(
                    onPressed: () => behind++,
                    child: const Text('behind'),
                  ),
                  Expanded(
                    child: HSheet(
                      title: const Text('Rule from request'),
                      closeSemanticsLabel: 'close',
                      onClose: () => closed++,
                      width: 200,
                      child: Column(
                        children: <Widget>[
                          HButton(onPressed: () {}, child: const Text('a')),
                          HButton(onPressed: () {}, child: const Text('b')),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      // Der Bereich *im* Blatt, nicht der darum: `FocusScope.of` am Blatt
      // selbst liefert den umgebenden.
      final FocusScopeNode scope = FocusScope.of(
        tester.element(find.text('a')),
      );
      expect(scope, isNot(FocusScope.of(tester.element(find.text('behind')))));
      bool insideSheet(BuildContext? context) {
        if (context == null) {
          return false;
        }
        bool found = false;
        context.visitAncestorElements((Element element) {
          if (element.widget is HSheet) {
            found = true;
            return false;
          }
          return true;
        });
        return found;
      }

      // Drei Stopps im Blatt: das Schließkreuz und die beiden Knöpfe. Nach
      // mehr Sprüngen als es Stopps gibt steht der Fokus immer noch im Blatt
      // und nicht auf dem Knopf dahinter.
      for (int i = 0; i < 6; i++) {
        scope.nextFocus();
        await tester.pumpAndSettle();
        expect(
          insideSheet(FocusManager.instance.primaryFocus?.context),
          isTrue,
          reason: 'Sprung $i verlässt das Blatt',
        );
      }
      expect(behind, 0, reason: 'Tab läuft nicht durch den Screen dahinter');
      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pump();
      expect(closed, 1);
      expect(tester.takeException(), isNull);
    });
  });

  group('glyphs', () {
    testWidgets('every glyph paints, the five new ones included', (
      WidgetTester tester,
    ) async {
      for (final HGlyph glyph in <HGlyph>[
        HGlyph.grip,
        HGlyph.trash,
        HGlyph.plus,
        HGlyph.lock,
        HGlyph.redactBar,
      ]) {
        expect(HGlyph.values, contains(glyph));
      }
      await tester.pumpWidget(
        harness(
          keyboard(
            Wrap(
              children: <Widget>[
                for (final HGlyph glyph in HGlyph.values)
                  HGlyphIcon(glyph, size: 20),
              ],
            ),
          ),
        ),
      );
      await tester.pump();
      expect(find.byType(HGlyphIcon), findsNWidgets(HGlyph.values.length));
      expect(tester.takeException(), isNull);
    });
  });

  group('ein Glyph vor der Beschriftung', () {
    testWidgets(
      'bindet den Inhalt, statt ihn aus dem Kasten laufen zu lassen',
      (WidgetTester tester) async {
        // Ein blankes `Row` mit `MainAxisSize.min` gibt einem flexlosen Kind
        // waagerecht unbeschränkte Constraints: die Beschriftung bricht dann
        // nicht um, sondern läuft über. Die Bibliothek bindet ihre
        // `leading`-Reihe deshalb in `IntrinsicWidth`/`IntrinsicHeight` mit
        // einem `Expanded`, und dieses Paket tut es genauso.
        const String long =
            'Regel für registry.npmjs.org anlegen und dauerhaft merken';
        for (final TextScaler scaler in <TextScaler>[
          TextScaler.noScaling,
          const TextScaler.linear(2),
        ]) {
          await tester.pumpWidget(
            Directionality(
              textDirection: TextDirection.ltr,
              child: MediaQuery(
                data: MediaQueryData(textScaler: scaler),
                child: HTheme(
                  tokens: HTokens.dark,
                  child: Align(
                    alignment: Alignment.topLeft,
                    child: SizedBox(
                      width: 160,
                      child: SingleChildScrollView(
                        child: HButton(
                          leading: const HGlyphIcon(HGlyph.lock),
                          onPressed: () {},
                          child: const Text(long),
                        ),
                      ),
                    ),
                  ),
                ),
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(
            tester.takeException(),
            isNull,
            reason: 'Überlauf bei $scaler',
          );
          expect(
            tester.getSize(find.byType(HButton)).width,
            lessThanOrEqualTo(160),
            reason: 'der Knopf bleibt in seinem Kasten bei $scaler',
          );
        }
      },
    );
  });

  group('text scale 2.0', () {
    testWidgets('no control clips and no row overflows', (
      WidgetTester tester,
    ) async {
      final TextEditingController controller = TextEditingController(
        text: '**.npmjs.org',
      );
      addTearDown(controller.dispose);
      await tester.pumpWidget(
        Directionality(
          textDirection: TextDirection.ltr,
          child: MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(2)),
            child: HTheme(
              tokens: HTokens.dark,
              child: Align(
                alignment: Alignment.topLeft,
                child: SizedBox(
                  width: 480,
                  // Die Seite scrollt; was hier gemessen wird, ist der
                  // Überlauf *in* einem Control, nicht der der Testspalte.
                  child: SingleChildScrollView(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        HRow(
                          state: HFlowState.held,
                          onTap: () {},
                          stateGlyph: const HStateGlyph(
                            state: HFlowState.held,
                            progress: 0.4,
                          ),
                          leading: const HMethodBadge(method: 'DELETE'),
                          title: const Text('registry.npmjs.org'),
                          semanticsLabel: 'held flow',
                        ),
                        HButton(onPressed: () {}, child: const Text('Send')),
                        HButton(
                          size: HButtonSize.md,
                          onPressed: () {},
                          child: const Text('Block'),
                        ),
                        const HBadge(text: '3 findings'),
                        HTextField(
                          controller: controller,
                          semanticsLabel: 'host pattern',
                        ),
                        HCheckbox(
                          label: 'Remember for this session',
                          value: true,
                          onChanged: (bool value) {},
                        ),
                        // Die vier, die dieser Test ausließ (`docs/UX.md` 9).
                        HPill(
                          left: const Text('Allow'),
                          onLeft: () {},
                          onRight: () {},
                          leftSemanticsLabel: 'allow once',
                          rightSemanticsLabel: 'allow scope',
                        ),
                        HSegmented<int>(
                          selected: 1,
                          onSelect: (int value) {},
                          options: const <HSegmentOption<int>>[
                            HSegmentOption<int>(value: 1, label: 'Session'),
                            HSegmentOption<int>(value: 2, label: 'Forever'),
                          ],
                        ),
                        HIconButton(
                          glyph: HGlyph.close,
                          onPressed: () {},
                          semanticsLabel: 'close',
                        ),
                        SizedBox(
                          // Ein Modal füllt sonst das Fenster; bei doppelter
                          // Skalierung misst seine Karte 290 px.
                          height: 360,
                          child: HModal(
                            title: const Text('Block five flows?'),
                            onDismiss: () {},
                            scrimSemanticsLabel: 'dismiss',
                            actions: <Widget>[
                              HButton(
                                onPressed: () {},
                                child: const Text('Block all'),
                              ),
                            ],
                            child: const Text('This cannot be undone.'),
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      // Jede der drei festen Höhen von früher ist eine Untergrenze geworden;
      // bei doppelter Skalierung wächst sie mit (`docs/UX.md` 6).
      expect(tester.getSize(find.byType(HRow)).height, greaterThan(HSize.row));
      expect(
        tester.getSize(find.byType(HBadge).last).height,
        greaterThanOrEqualTo(HSize.hitMin),
      );
      for (final Element element in find.byType(HButton).evaluate()) {
        expect(
          (element.renderObject! as RenderBox).size.height,
          greaterThan(HSize.hitMin),
        );
      }
      // Auch die vier Nachzügler wachsen mit der Schrift, statt sie
      // abzuschneiden.
      for (final Type type in <Type>[HPill, HIconButton]) {
        expect(
          tester.getSize(find.byType(type)).height,
          greaterThanOrEqualTo(HSize.hitMin),
          reason: '$type',
        );
      }
      expect(
        tester.getSize(find.byType(HPill)).height,
        greaterThan(HSize.hitMin),
        reason: 'die Pille wächst mit ihrer Beschriftung',
      );
    });
  });
}
