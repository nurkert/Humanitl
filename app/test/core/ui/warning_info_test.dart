// Warnung und Information haben seit HUM-260 eigene Töne: `warning` ist das
// Gelb der Marke, `info` das Blau, das vorher der Akzent war. `held` heißt
// ausschließlich „gehalten" und taucht in keiner dieser Zuordnungen mehr auf.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/body/body_marks.dart';
import 'package:humanitl/core/body/body_span.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/diagnostic_severity.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/history/history_filter_bar.dart';
import 'package:humanitl/features/sandbox/sandbox_text.dart';
import 'package:humanitl/features/setup/providers/setup_provider.dart';
import 'package:humanitl/features/setup/setup_text.dart';

void main() {
  for (final Brightness brightness in Brightness.values) {
    final HTokens tokens = HTokens.forBrightness(brightness);
    final String theme = brightness.name;

    test('$theme: a diagnostic says info in blue and warning in yellow', () {
      expect(severityColor(tokens, Severity.info), tokens.colors.info);
      expect(severityColor(tokens, Severity.warning), tokens.colors.warning);
      expect(severityColor(tokens, Severity.info), isNot(tokens.colors.accent));
      // Die History schreibt den Grad als Wort und nimmt die Textvarianten.
      expect(
        historySeverityColor(tokens, Severity.info),
        tokens.colors.infoText,
      );
      expect(
        historySeverityColor(tokens, Severity.warning),
        tokens.colors.warningText,
      );
      expect(
        historySeverityColor(tokens, Severity.info),
        isNot(historySeverityColor(tokens, Severity.warning)),
      );
    });

    test('$theme: what is not settled wears the warning, never held', () {
      expect(
        setupStateColor(tokens, SetupCheckState.warn),
        tokens.colors.warning,
      );
      expect(
        setupStateColor(tokens, SetupCheckState.checking),
        tokens.colors.warning,
      );
      expect(
        setupStateTextColor(tokens, SetupCheckState.warn),
        tokens.colors.warningText,
      );
      expect(
        sandboxStateColor(tokens, SandboxState.starting),
        tokens.colors.warning,
      );
      expect(
        sandboxStateColor(tokens, SandboxState.stopping),
        tokens.colors.warning,
      );
      expect(
        isolationSegmentColor(tokens, IsolationSegment.running),
        tokens.colors.warning,
      );
      expect(
        isolationSegmentTextColor(tokens, IsolationSegment.running),
        tokens.colors.warningText,
      );
      expect(
        sandboxMountModeColor(tokens, MountMode.rw),
        tokens.colors.warningText,
      );
      expect(
        bodyFindingColor(tokens, BodyFindingTone.personal),
        tokens.colors.warning,
      );
      expect(
        HEditorDecorations.colorOf(tokens, HEditorDecorationKind.pii),
        tokens.colors.warning,
      );
      // Keine dieser Stellen trägt mehr das Cyan von `held`.
      final Color held = tokens.state.held;
      final Color heldText = tokens.stateText.held;
      for (final Color used in <Color>[
        setupStateColor(tokens, SetupCheckState.warn),
        setupStateColor(tokens, SetupCheckState.checking),
        sandboxStateColor(tokens, SandboxState.starting),
        isolationSegmentColor(tokens, IsolationSegment.running),
        sandboxMountModeColor(tokens, MountMode.rw),
        bodyFindingColor(tokens, BodyFindingTone.personal),
        severityColor(tokens, Severity.warning),
      ]) {
        expect(used, isNot(held));
        expect(used, isNot(heldText));
      }
    });
  }
}
