// Was die App unter macOS sagt (HUM-372): Sie nennt launchd und Seatbelt, nie
// die Programme von Linux, und keine Aussage über die Sandbox, die dort nicht
// gilt (`docs/SECURITY.md` Abschnitt 12).

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/client_diagnostics.dart';
import 'package:humanitl/core/ipc/private_path.dart';
import 'package:humanitl/core/ui/shell_command.dart';
import 'package:humanitl/features/sandbox/sandbox_text.dart';
import 'package:humanitl/features/setup/setup_text.dart';
import 'package:humanitl/l10n/l10n.dart';

/// Was unter macOS nicht existiert und deshalb in keinem Text stehen darf.
final RegExp _linuxOnly = RegExp(
  r'systemd|systemctl|journalctl|bwrap|bubblewrap|seccomp|namespace|'
  r'namensr|XDG_|/run/user|network interface|Netzwerk-Interface|\bunits?\b',
  caseSensitive: false,
);

/// Die Codes des Registers.
List<String> _registerCodes() =>
    RegExp(r'^\s+([A-Z]+_\d{3}) =>', multiLine: true)
        .allMatches(
          File('../daemon/crates/core-types/src/diagnostics/codes.rs')
              .readAsStringSync(),
        )
        .map((RegExpMatch match) => match.group(1)!)
        .toList();

void main() {
  final AppLocalizations en = lookupAppLocalizations(const Locale('en'));
  final AppLocalizations de = lookupAppLocalizations(const Locale('de'));

  tearDown(() => debugDefaultTargetPlatformOverride = null);

  test('no diagnostic names a Linux mechanism on macOS', () {
    final List<String> codes = _registerCodes();
    expect(codes, isNotEmpty);
    final List<String> found = <String>[
      for (final String code in codes)
        for (final AppLocalizations l10n in <AppLocalizations>[en, de])
          if (diagnosticTexts(l10n, code, platform: TargetPlatform.macOS)
              case (final String title, final String why)
              when _linuxOnly.hasMatch(title) || _linuxOnly.hasMatch(why))
            '$code/${l10n.localeName}: $title / $why',
    ];
    expect(found, isEmpty);
  });

  test('Linux keeps its own diagnostic texts', () {
    for (final String code in _registerCodes()) {
      for (final TargetPlatform platform in <TargetPlatform>[
        TargetPlatform.linux,
        TargetPlatform.android,
      ]) {
        final (String, String)? linux = diagnosticTexts(
          en,
          code,
          platform: platform,
        );
        expect(linux, isNotNull, reason: code);
      }
    }
    expect(diagnosticTexts(en, 'DOCTOR_005', platform: TargetPlatform.linux), (
      en.diagDOCTOR005Title,
      en.diagDOCTOR005Why,
    ));
  });

  test('resolve follows the platform without being told', () {
    const Diagnostic diagnostic = Diagnostic(
      code: 'DOCTOR_005',
      severity: Severity.warning,
      title: '',
      why: '',
    );
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    expect(
      DiagnosticL10n.resolve(diagnostic, en).title,
      en.macosDiagDOCTOR005Title,
    );
    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    expect(DiagnosticL10n.resolve(diagnostic, en).title, en.diagDOCTOR005Title);
  });

  test('the guarantees read as docs/SECURITY.md section 12 says', () {
    for (final AppLocalizations l10n in <AppLocalizations>[en, de]) {
      for (final IsolationCheck check in IsolationCheck.values) {
        final String sentence = isolationCheckSentence(
          l10n,
          check,
          platform: TargetPlatform.macOS,
        );
        expect(sentence, isNot(contains('seccomp')));
        expect(sentence, isNot(contains('Kein Netzwerk-Interface')));
        expect(sentence, isNot(contains('No network interface')));
      }
    }
    expect(
      isolationCheckSentence(
        en,
        IsolationCheck.noNetworkInterface,
        platform: TargetPlatform.linux,
      ),
      en.isolationCheck1,
    );
    expect(
      isolationCheckSentence(
        en,
        IsolationCheck.seccompActive,
        platform: TargetPlatform.linux,
      ),
      en.isolationCheck3,
    );
  });

  test('the doctor headings name what was checked', () {
    expect(
      doctorCheckTitle(en, DoctorCheckId.bwrap, platform: TargetPlatform.macOS),
      en.doctorCheckBwrapMacos,
    );
    expect(
      doctorCheckTitle(
        de,
        DoctorCheckId.systemdUser,
        platform: TargetPlatform.macOS,
      ),
      de.doctorCheckSystemdUserMacos,
    );
    expect(
      doctorCheckTitle(en, DoctorCheckId.bwrap, platform: TargetPlatform.linux),
      en.doctorCheckBwrap,
    );
  });

  test('lines without a mechanism on macOS leave the list while green', () {
    const List<DoctorCheck> lines = <DoctorCheck>[
      DoctorCheck(id: DoctorCheckId.bwrap, status: DoctorStatus.ok),
      DoctorCheck(id: DoctorCheckId.userns, status: DoctorStatus.ok),
      DoctorCheck(id: DoctorCheckId.seccomp, status: DoctorStatus.fail),
    ];
    expect(
      shownDoctorChecks(
        lines,
        platform: TargetPlatform.macOS,
      ).map((DoctorCheck check) => check.id),
      <String>[DoctorCheckId.bwrap, DoctorCheckId.seccomp],
    );
    expect(shownDoctorChecks(lines, platform: TargetPlatform.linux), lines);
  });

  test('commands on macOS name the command line by its full path', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    expect(
      ClientDiagnostics.startUnitCommand,
      '$macCommandLinePath daemon install',
    );
    expect(
      ClientDiagnostics.restartUnitCommand,
      '$macCommandLinePath daemon restart',
    );
    expect(ClientDiagnostics.runInFolderCommand, '$macCommandLinePath run');
    expect(commandLineProgram(), macCommandLinePath);

    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    expect(
      ClientDiagnostics.restartUnitCommand,
      'systemctl --user restart humanitld',
    );
    expect(ClientDiagnostics.runInFolderCommand, 'humanitl run');
    expect(commandLineProgram(), 'humanitl');
  });

  test('the trust help names the restart of this system', () {
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    expect(
      en.settingsTrustSystemStoreHelp(ClientDiagnostics.restartUnitCommand),
      allOf(contains(macCommandLinePath), isNot(contains('systemctl'))),
    );
  });

  test('the runtime fallback on macOS leads to the macOS section', () {
    const PrivatePathProblem problem = PrivatePathProblem(
      '/tmp/h',
      'is mode 0755',
      open: true,
    );
    debugDefaultTargetPlatformOverride = TargetPlatform.macOS;
    final Diagnostic mac = ClientDiagnostics.runtimeUntrusted(
      problem,
      fallback: true,
    );
    expect(
      mac.fix,
      const FixAction.openUrl(url: ClientDiagnostics.macRuntimeDirDocUrl),
    );
    expect(mac.why, isNot(contains('XDG_RUNTIME_DIR')));

    debugDefaultTargetPlatformOverride = TargetPlatform.linux;
    expect(
      ClientDiagnostics.runtimeUntrusted(problem, fallback: true).fix,
      const FixAction.openUrl(url: ClientDiagnostics.ownRuntimeDirDocUrl),
    );
  });

  test('the macOS section the link names exists', () {
    expect(
      File('../docs/INSTALL.md').readAsStringSync(),
      contains('## Runtime directory on macOS'),
    );
    // Dieselben Literale prüft die Rust-Seite in `private_dir.rs`.
    expect(
      ClientDiagnostics.macRuntimeDirDocUrl,
      'https://github.com/nurkert/Humanitl/blob/main/docs/INSTALL.md#runtime-directory-on-macos',
    );
    expect(
      ClientDiagnostics.macRuntimeDirHint,
      'the path under ~/Library/Application Support is too long for a socket, so daemon, CLI and app use the temporary directory; the linked section says what to do about it',
    );
  });
}
