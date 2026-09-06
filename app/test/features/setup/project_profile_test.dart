// Die Oberflaechen-Haelfte von HUM-066: Ein Projekt-Profil, das `extra_rw`
// setzt, bekommt keinen Start.
//
// Die Kommandozeile ist in `daemon/bin/humanitl/tests/cli.rs` gemessen; hier
// steht, was ein Mensch davon sieht. Der Daemon entscheidet, die Oberflaeche
// zeigt: Sie erfindet die Weigerung nicht und sie versteckt sie nicht, und der
// Bildschirm bleibt stehen, statt in eine leere Warteschlange zu wechseln.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/setup/setup_screen.dart';
import 'package:humanitl/features/shell/providers/navigation.dart';
import 'package:humanitl/features/shell/section.dart';

import '../../harness/app_harness.dart';

/// Der Befund, mit dem der Daemon einen Start ablehnt, dessen Projekt-Profil
/// eine vertrauensrelevante Einstellung setzt (`CONFIG_003`, HUM-066).
///
/// Der Wortlaut stammt aus `daemon/crates/config/src/load.rs`
/// (`project_scope_denied`): Er nennt den Schluessel, die Ebene und wohin die
/// Einstellung gehoert — und ausdruecklich keinen Knopf, der sie uebernimmt.
const Diagnostic extraRwRefused = Diagnostic(
  code: 'CONFIG_003',
  severity: Severity.blocking,
  title: 'Wert außerhalb des Bereichs',
  why:
      'sandbox.extra_rw (from the project profile .humanitl/profile.toml) may '
      'not be set by a project profile: the file is part of the repository and '
      'cannot decide trust-relevant settings; move this setting to the global '
      'config or profile',
);

void main() {
  testWidgets('a_project_profile_with_extra_rw_gets_no_sandbox', (
    WidgetTester tester,
  ) async {
    final FakeDaemonClient client = FakeDaemonClient()
      ..doctorReport = fakeDoctorOk()
      ..sandboxStartFailure = extraRwRefused;
    await pumpApp(tester, client: client);
    await pressCtrl(tester, LogicalKeyboardKey.digit6);
    await tester.pump();

    await tester.tap(find.byKey(const Key('setup-start')));
    await tester.pumpAndSettle();

    // Der Bildschirm bleibt stehen: Ohne Sandbox gibt es nichts, wohin er
    // wechseln koennte, und eine leere Warteschlange saehe aus wie ein Lauf
    // ohne Verkehr.
    expect(find.byType(SetupScreen), findsOneWidget);
    expect(
      ProviderScope.containerOf(tester.element(find.byType(SetupScreen)))
          .read(navigationProvider),
      Section.setup,
    );

    // Und der Grund steht darauf, mit dem Code: `CONFIG_003` ist der kuerzeste
    // Weg von dem, was ein Mensch sieht, zu der Stelle, die es entschieden hat.
    expect(find.byKey(const Key('setup-start-failure')), findsOneWidget);
    expect(find.textContaining('CONFIG_003'), findsWidgets);
    expect(
      find.textContaining('cannot decide trust-relevant settings'),
      findsOneWidget,
    );

    // Die Sandbox laeuft nicht, und die Oberflaeche behauptet es auch nicht.
    expect(client.sandbox.state, SandboxState.failed);
    expect(client.sandbox.agentRunning, isFalse);
  });
}
