// Gründe der Befunde in der Sprache der Oberfläche (HUM-569): In der
// deutschen Oberfläche steht der übersetzte Grundsatz im Grund-Platz, der
// englische Satz des Daemons nur zugeklappt unter „Originalmeldung
// (unübersetzt)". In der englischen bleibt alles, wie es war.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/untranslated_wording.dart';
import 'package:humanitl/core/ui/agents/agent_inset.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../goldens/support/golden_variants.dart';

const String _sentence =
    'the depot holds Claude Code and no login for it; the agent asks for one '
    'in the session';

const Diagnostic _finding = Diagnostic(
  code: 'AGENT_009',
  severity: Severity.warning,
  why: _sentence,
);

final AppLocalizations _de = lookupAppLocalizations(const Locale('de'));
final AppLocalizations _en = lookupAppLocalizations(const Locale('en'));

Future<void> _pump(
  WidgetTester tester,
  GoldenVariant variant,
  Widget child,
) async {
  await tester.pumpWidget(
    goldenFrame(
      variant: variant,
      child: Align(
        alignment: Alignment.topLeft,
        child: SizedBox(width: 520, child: child),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('the agent card speaks German and folds the daemon sentence', (
    WidgetTester tester,
  ) async {
    await _pump(
      tester,
      GoldenVariant.darkDe,
      const AgentFinding(diagnostic: _finding),
    );

    expect(find.text(_de.diagAGENT009Why), findsOneWidget);
    expect(find.text(_de.diagnosticOriginalWording), findsOneWidget);
    // Zugeklappt: der Satz liegt im Baum, aber nicht sichtbar.
    expect(find.text(_sentence), findsOneWidget);
    expect(find.text(_sentence).hitTestable(), findsNothing);

    await tester.tap(find.text(_de.diagnosticOriginalWording));
    await tester.pumpAndSettle();
    expect(find.text(_sentence).hitTestable(), findsOneWidget);
  });

  testWidgets('the English interface keeps the daemon sentence as the cause', (
    WidgetTester tester,
  ) async {
    await _pump(
      tester,
      GoldenVariant.darkEn,
      const AgentFinding(diagnostic: _finding),
    );

    expect(find.text(_sentence).hitTestable(), findsOneWidget);
    expect(find.text(_en.diagnosticOriginalWording), findsNothing);
    expect(find.text(_en.diagAGENT009Why), findsNothing);
  });

  testWidgets('an inline cause folds the same way', (
    WidgetTester tester,
  ) async {
    await _pump(
      tester,
      GoldenVariant.lightDe,
      DiagnosticCause(
        diagnostic: _finding,
        textKey: const Key('cause'),
        style: const TextStyle(),
      ),
    );

    expect(
      tester.widget<Text>(find.byKey(const Key('cause'))).data,
      _de.diagAGENT009Why,
    );
    expect(find.text(_de.diagnosticOriginalWording), findsOneWidget);
    expect(find.text(_sentence).hitTestable(), findsNothing);
  });

  testWidgets('a compact line keeps the measured sentence behind the hint', (
    WidgetTester tester,
  ) async {
    await _pump(
      tester,
      GoldenVariant.darkDe,
      DiagnosticCause(
        diagnostic: _finding,
        textKey: const Key('cause'),
        style: const TextStyle(),
        compact: true,
      ),
    );

    expect(
      tester.widget<Text>(find.byKey(const Key('cause'))).data,
      _de.diagnosticUntranslated(_sentence),
    );
    expect(find.text(_de.diagnosticOriginalWording), findsNothing);
  });

  testWidgets('an unknown code in German carries the hint in the line', (
    WidgetTester tester,
  ) async {
    const Diagnostic unknown = Diagnostic(
      code: 'NOPE_999',
      severity: Severity.error,
      why: 'the thing did not work',
    );
    await _pump(
      tester,
      GoldenVariant.darkDe,
      DiagnosticCause(
        diagnostic: unknown,
        textKey: const Key('cause'),
        style: const TextStyle(),
      ),
    );

    expect(
      tester.widget<Text>(find.byKey(const Key('cause'))).data,
      _de.diagnosticUntranslated('the thing did not work'),
    );
    expect(find.text(_de.diagnosticOriginalWording), findsNothing);
  });

  test('a raw line stays a detail in English and folds elsewhere', () {
    final (String? detailEn, Widget? foldEn) = UntranslatedWording.split(
      _en,
      'Permission denied (os error 13)',
    );
    expect(detailEn, 'Permission denied (os error 13)');
    expect(foldEn, isNull);

    final (String? detailDe, Widget? foldDe) = UntranslatedWording.split(
      _de,
      'Permission denied (os error 13)',
    );
    expect(detailDe, isNull);
    expect(foldDe, isA<UntranslatedWording>());
  });
}
