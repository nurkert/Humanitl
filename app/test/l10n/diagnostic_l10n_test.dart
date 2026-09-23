// Übersetzung der Diagnosen anhand ihres Codes (HUM-052).

import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/features/setup/setup_text.dart';
import 'package:humanitl/l10n/l10n.dart';

final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

/// The codes of the register, read where the daemon keeps them.
List<String> registerCodes() =>
    RegExp(r'^\s+([A-Z]+_\d{3}) =>', multiLine: true)
        .allMatches(
          File('../daemon/crates/core-types/src/diagnostics/codes.rs')
              .readAsStringSync(),
        )
        .map((RegExpMatch m) => m.group(1)!)
        .toList();

void main() {
  test('diagnostic_fallback_to_daemon_text', () {
    const Diagnostic unknown = Diagnostic(
      code: 'NOPE_999',
      severity: Severity.error,
      title: 'Etwas ist schiefgegangen',
      why: 'the thing did not work',
    );
    final DiagnosticText text = DiagnosticL10n.resolve(unknown, de);

    expect(text.title, 'Etwas ist schiefgegangen');
    expect(text.why, 'the thing did not work');
    expect(text.cause, 'the thing did not work');
  });

  test('an unknown code without a title shows the code', () {
    const Diagnostic bare = Diagnostic(
      code: 'NOPE_999',
      severity: Severity.error,
    );
    expect(DiagnosticL10n.resolve(bare, en).title, 'NOPE_999');
  });

  test('a known code takes its title from ARB in both languages', () {
    const Diagnostic diagnostic = Diagnostic(
      code: 'CONFIG_001',
      severity: Severity.blocking,
      title: 'Config-Datei ungültig',
      why: 'config.toml is not valid TOML',
    );

    final DiagnosticText english = DiagnosticL10n.resolve(diagnostic, en);
    final DiagnosticText german = DiagnosticL10n.resolve(diagnostic, de);

    expect(english.title, en.diagCONFIG001Title);
    expect(german.title, de.diagCONFIG001Title);
    expect(english.title, isNot(german.title));
    expect(english.why, en.diagCONFIG001Why);
    expect(german.why, de.diagCONFIG001Why);
    // Der gemessene Satz des Daemons bleibt der Grund (docs/UX.md 4.4).
    expect(english.cause, 'config.toml is not valid TOML');
    expect(german.cause, 'config.toml is not valid TOML');
  });

  test('without a sentence of the daemon the cause is the ARB one', () {
    const Diagnostic diagnostic = Diagnostic(
      code: 'CONFIG_013',
      severity: Severity.warning,
    );
    expect(DiagnosticL10n.resolve(diagnostic, de).cause, de.diagCONFIG013Why);
  });

  test('the setup screen shows the translated title and the daemon cause', () {
    const Diagnostic diagnostic = Diagnostic(
      code: 'CONFIG_001',
      severity: Severity.blocking,
      title: 'Config-Datei ungültig',
      why: 'config.toml is not valid TOML',
    );
    expect(setupDiagnosticText(en, diagnostic), (
      en.diagCONFIG001Title,
      'config.toml is not valid TOML',
    ));
  });

  test('every code of the register resolves in both languages', () {
    final List<String> codes = registerCodes();
    expect(codes, isNotEmpty);
    final List<String> missing = <String>[
      for (final String code in codes)
        for (final AppLocalizations l10n in <AppLocalizations>[en, de])
          if (diagnosticTexts(l10n, code) == null) '$code/${l10n.localeName}',
    ];
    expect(missing, isEmpty);
  });
}
