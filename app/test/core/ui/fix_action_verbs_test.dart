// Die sieben Verben der Abhilfe in beiden Sprachen (HUM-058, Schritt 2 und
// Test `diagnostic_card_all_fix_actions`).
//
// Jede `FixAction` steht in einer `HDiagnosticCard`, und ihr Verb kommt aus
// ARB. Gegen welche Umkehrung der Test gerichtet ist: Ein Zweig in
// `FixControl.build`, der eine Variante auf `SizedBox.shrink` oder auf das
// Verb einer anderen legt, macht ihn rot, in der Sprache, in der er es tut.
//
// Die Schlüssel heißen anders als in der Spezifikation (`fix_set_env` …):
// HUM-044 hat sie als `setupFix…` angelegt, in der Schreibweise, die
// `backlog/CONVENTIONS.md` 4.11 für ARB-Schlüssel vorschreibt.

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/h_diagnostic_card.dart';
import 'package:humanitl/l10n/l10n.dart';

import '../../goldens/support/golden_variants.dart';

/// Die sieben Varianten, je mit dem Verb, das auf dem Schirm stehen muss.
List<(String, FixAction, String Function(AppLocalizations))> get actions =>
    <(String, FixAction, String Function(AppLocalizations))>[
      (
        'set_env',
        const FixAction.setEnv(key: 'NODE_EXTRA_CA_CERTS', value: '/tmp/ca'),
        (AppLocalizations l10n) => l10n.setupFixSetEnv('NODE_EXTRA_CA_CERTS'),
      ),
      (
        'add_rule',
        FixAction.addRule(
          rule: Rule(
            id: const RuleId('018f0000-0000-7000-8000-0000000000b1'),
            action: RuleAction.ask,
            matcher: const RuleMatcher(host: 'api.example.com'),
            expires: const RuleExpiry.never(),
            createdAt: DateTime.utc(2026, 1, 1),
          ),
        ),
        (AppLocalizations l10n) => l10n.setupFixAddRule,
      ),
      (
        'install_service',
        const FixAction.installService(),
        (AppLocalizations l10n) => l10n.setupFixInstallService,
      ),
      (
        'change_setting',
        const FixAction.changeSetting(key: 'llm.endpoint', value: ''),
        (AppLocalizations l10n) => l10n.setupFixChangeSetting('llm.endpoint'),
      ),
      (
        'copy_command',
        const FixAction.copyCommand(
          command: 'systemctl --user restart humanitld',
        ),
        (AppLocalizations l10n) => l10n.setupFixCopyCommand,
      ),
      (
        'open_url',
        const FixAction.openUrl(url: 'https://example.invalid/upgrade'),
        (AppLocalizations l10n) => l10n.setupFixCopyLink,
      ),
      (
        'remount_ro',
        const FixAction.remountReadOnly(path: '/work'),
        (AppLocalizations l10n) => l10n.setupFixRemountReadOnly,
      ),
    ];

void main() {
  for (final GoldenVariant variant in <GoldenVariant>[
    GoldenVariant.darkEn,
    GoldenVariant.darkDe,
  ]) {
    final AppLocalizations l10n = lookupAppLocalizations(variant.locale);
    for (final (
          String name,
          FixAction fix,
          String Function(AppLocalizations) verb,
        )
        in actions) {
      testWidgets(
        'diagnostic_card_all_fix_actions $name ${variant.language.name}',
        (WidgetTester tester) async {
          await tester.binding.setSurfaceSize(const Size(800, 600));
          addTearDown(() => tester.binding.setSurfaceSize(null));
          await tester.pumpWidget(
            goldenFrame(
              variant: variant,
              child: Align(
                alignment: Alignment.topLeft,
                child: HDiagnosticCard(
                  code: 'DAEMON_001',
                  severityLabel: 'Error',
                  color: const Color(0xFFF0784F),
                  title: 'Title',
                  why: 'Why',
                  fix: FixControl(fix: fix),
                ),
              ),
            ),
          );
          await tester.pump();

          expect(find.text(verb(l10n)), findsOneWidget);
          // Das Verb ist das der Sprache und nicht das der anderen.
          final AppLocalizations other = lookupAppLocalizations(
            variant.language == AppLanguage.en
                ? const Locale('de')
                : const Locale('en'),
          );
          if (verb(other) != verb(l10n)) {
            expect(find.text(verb(other)), findsNothing);
          }
        },
      );
    }
  }

  test('the seven verbs are seven different words in each language', () {
    for (final Locale locale in const <Locale>[Locale('en'), Locale('de')]) {
      final AppLocalizations l10n = lookupAppLocalizations(locale);
      final Set<String> verbs = <String>{
        for (final (_, _, String Function(AppLocalizations) verb) in actions)
          verb(l10n),
      };
      expect(verbs, hasLength(7), reason: '$locale');
    }
  });
}
