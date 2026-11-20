// The agent switch with six agents at the smallest window (HUM-515,
// HUM-517).
//
// With Codex and the Antigravity CLI the segmented choice of
// `agent.adapter` holds six options. In
// the sheet of a window at its minimum size (`appMinimumLayoutSize`) every
// option must stay whole inside its row, in English and German, with Roboto
// (`harness/layout_fonts.dart`): with the square glyphs of the test font the
// choice would be twice as wide as on screen. An overflow of the row is a
// Flutter error and fails the test by itself. Mutation proof: the Codex
// label in `AgentRow` three times as long turns all four cases red
// ("A RenderFlex overflowed by 223 pixels"). With six options (HUM-517)
// the Antigravity label ten times as long turns all four cases red
// ("overflowed by 113 pixels"); eight times as long still fits, so about
// 450 pixels stay free at the minimum size.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/core/ui/ui_scale.dart';

import '../../harness/layout_fonts.dart';
import '../../harness/settings_harness.dart';
import 'settings_overlay_test.dart' show open;

void main() {
  setUpAll(loadLayoutFonts);

  final TargetPlatformVariant desktops = TargetPlatformVariant(<TargetPlatform>{
    TargetPlatform.linux,
    TargetPlatform.macOS,
  });

  for (final bool german in <bool>[false, true]) {
    final String language = german ? 'de' : 'en';

    testWidgets('at the minimum window size every agent option stays whole '
        'inside its row ($language)', (WidgetTester tester) async {
      await open(tester, german: german, size: appMinimumLayoutSize);
      await showSettingsCategory(tester, SettingsCategory.agent);
      final Finder control = find.byKey(const Key('settings-agent'));
      await tester.ensureVisible(control);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      final HSegmented<String> choice = tester.widget<HSegmented<String>>(
        control,
      );
      expect(
        choice.options.map((HSegmentOption<String> option) => option.value),
        FormKeys.agentAdapters,
      );
      final Rect row = tester.getRect(
        find.byKey(
          const ValueKey<String>('setting-row-${FormKeys.agentAdapter}'),
        ),
      );
      for (final HSegmentOption<String> option in choice.options) {
        final Finder label = find.descendant(
          of: control,
          matching: find.text(option.label),
        );
        expect(label, findsOneWidget, reason: option.value);
        final Rect rect = tester.getRect(label);
        expect(
          rect.left >= row.left - 0.5 && rect.right <= row.right + 0.5,
          isTrue,
          reason: '${option.value} leaves its row: $rect in $row',
        );
        final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
          label,
        );
        expect(
          paragraph.didExceedMaxLines,
          isFalse,
          reason: '${option.value} is cut off',
        );
      }
    }, variant: desktops);
  }
}
