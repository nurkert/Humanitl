// Der Sandbox-Bildschirm (HUM-040): der Satz, die Tabellen, die eine Modal,
// der Beleg. Jeder Test prüft eine Aussage, die der Bildschirm über die
// Sandbox macht, gegen das, was der Daemon geantwortet hat.

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/features/sandbox/widgets/env_tab.dart';
import 'package:humanitl/features/sandbox/widgets/sandbox_header.dart';

import 'harness.dart';

void main() {
  testWidgets('start_button_disabled_when_blocking_diagnostic', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = blockedClient();
    await pumpSandbox(tester, client: client);

    final HButtonFinder start = HButtonFinder(tester, 'sandbox-start');
    expect(start.enabled, isFalse, reason: 'a blocking finding forbids it');
    // Der Grund steht nicht nur auf der Karte, sondern auch am Control.
    expect(find.textContaining('bwrap not found'), findsWidgets);
    expect(client.starts, 0);
  });

  testWidgets('start_button_enabled_without_a_blocking_diagnostic', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpSandbox(tester, client: client);
    expect(HButtonFinder(tester, 'sandbox-start').enabled, isTrue);

    await tester.tap(find.byKey(const Key('sandbox-start')));
    await tester.pump();
    await tester.pump();
    expect(client.starts, 1);
    expect(statusOf(tester).state, SandboxState.running);
  });

  testWidgets('stop_shows_dialog_when_running', (WidgetTester tester) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-stop')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    expect(find.byKey(const Key('sandbox-stop-confirm')), findsOneWidget);
    // Nichts ist passiert, solange die Frage steht.
    expect(client.stops, 0);
    // Die Vorauswahl liegt auf „Abbrechen", nie auf dem Stopp.
    expect(
      HButtonFinder(tester, 'sandbox-stop-cancel').hasFocus,
      isTrue,
      reason: 'the destructive answer is never preselected',
    );

    await tester.tap(find.byKey(const Key('sandbox-stop-confirm')));
    await tester.pump();
    await tester.pump();
    expect(client.stops, 1);
    expect(statusOf(tester).state, SandboxState.stopped);
  });

  testWidgets('escape_cancels_the_stop_dialog', (WidgetTester tester) async {
    final SandboxTestClient client = runningClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-stop')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));
    expect(find.byKey(const Key('sandbox-stop-confirm')), findsOneWidget);

    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pump();
    expect(find.byKey(const Key('sandbox-stop-confirm')), findsNothing);
    expect(client.stops, 0);
  });

  testWidgets('stop_without_dialog_when_agent_exited', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = runningClient(agentRunning: false);
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-stop')));
    await tester.pump();
    await tester.pump();

    expect(find.byKey(const Key('sandbox-stop-confirm')), findsNothing);
    expect(client.stops, 1);
  });

  testWidgets('mounts_sentence_renders_host_path_and_mode', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());

    final Text sentence = tester.widget<Text>(
      find.byKey(const Key('sandbox-mounts-sentence')),
    );
    expect(sentence.data, contains(FakeDaemonClient.defaultWorkDir));
    expect(sentence.data, contains('/work'));
    expect(sentence.data, contains('read and write'));
  });

  testWidgets('a_link_target_is_never_shown_as_a_host_path', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());

    final SandboxStatus status = statusOf(tester);
    final MountEntry link = status.mounts.firstWhere(
      (MountEntry mount) => mount.mode == MountMode.symlink,
    );
    expect(link.linkTarget, isNotEmpty);
    // Das Ziel liegt in der Sandbox; die Spalte „auf diesem Rechner" bleibt
    // deshalb leer, und das Ziel steht neben dem Wort „Verweis"
    // (Review Codex, Befund 5).
    expect(link.hasHostPath, isFalse);
    expect(find.textContaining('link to ${link.linkTarget}'), findsOneWidget);
    expect(find.text(link.linkTarget), findsNothing);
  });

  testWidgets('mounts_table_lists_every_path_of_the_snapshot', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());

    final SandboxStatus status = statusOf(tester);
    expect(status.mounts, isNotEmpty);
    for (final MountEntry mount in status.mounts) {
      expect(
        find.byKey(ValueKey<String>('mount-${mount.dst}')),
        findsOneWidget,
        reason: 'a mount that is not shown is a broken promise: ${mount.dst}',
      );
    }
  });

  testWidgets('env_tab_masks_withheld_values', (WidgetTester tester) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpSandbox(tester, client: client);
    await openTab(tester, 'sandbox-tab-env');

    final SandboxStatus status = statusOf(tester);
    final List<EnvEntry> withheld = status.env
        .where((EnvEntry entry) => entry.withheld)
        .toList();
    expect(withheld, isNotEmpty);
    // Zurückgehalten steht als Punkte plus Wort da, nie als leerer Wert.
    expect(find.textContaining(envMask), findsNWidgets(withheld.length));
    expect(find.textContaining('withheld'), findsWidgets);
    // Und nicht wie eine wirklich leere Variable.
    expect(find.text('empty'), findsOneWidget);
    for (final EnvEntry entry in withheld) {
      expect(entry.value, isEmpty, reason: entry.key);
      // Die Namen, an denen eine Endungsregel scheitern würde: kein Fund
      // heißt hier, dass die Regel andersherum läuft (CONVENTIONS 4.17).
      for (final String suffix in <String>[
        '_TOKEN',
        '_KEY',
        '_SECRET',
        'PASSWORD',
      ]) {
        expect(
          entry.key.endsWith(suffix),
          isFalse,
          reason: '${entry.key} is withheld without looking suspicious',
        );
      }
    }
    // Kein Kopieren, kein Aufdecken: es gibt nichts aufzudecken.
    expect(find.byKey(const Key('sandbox-env-reveal')), findsNothing);
    expect(find.byKey(const Key('sandbox-env-copy')), findsNothing);
    expect(find.byKey(const Key('sandbox-env-masked-why')), findsOneWidget);
  });

  testWidgets('env_tab_filters_by_name', (WidgetTester tester) async {
    await pumpSandbox(tester, client: SandboxTestClient());
    await openTab(tester, 'sandbox-tab-env');

    await tester.enterText(find.byType(EditableText).first, 'proxy');
    await tester.pump();
    expect(find.text('HTTP_PROXY'), findsOneWidget);
    expect(find.text('HOME'), findsNothing);

    await tester.enterText(find.byType(EditableText).first, 'nothing');
    await tester.pump();
    expect(
      find.textContaining('No variable is called like that'),
      findsOneWidget,
    );
  });

  testWidgets('argv_sheet_shows_full_command', (WidgetTester tester) async {
    await pumpSandbox(tester, client: SandboxTestClient());

    await tester.tap(find.byKey(const Key('sandbox-show-argv')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 200));

    final Text argv = tester.widget<Text>(
      find.byKey(const Key('sandbox-argv-text')),
    );
    expect(argv.data, contains('--unshare-net'));
    expect(argv.data, contains('--cap-drop ALL'));
    expect(argv.data, contains(FakeDaemonClient.defaultWorkDir));
    // Der Beleg wird nicht umgebrochen; ein Pfad über zwei Zeilen lässt sich
    // nicht mit dem auf der Platte vergleichen.
    expect(argv.maxLines, 1);
    // Und er schwärzt dieselben Werte wie die Umgebungstabelle. Die Zeile ist
    // das eine Stück dieses Bildschirms, das man kopiert (CONVENTIONS 4.17).
    for (final EnvEntry entry in statusOf(tester).env) {
      if (entry.withheld) {
        expect(argv.data, contains("${entry.key} '<withheld>'"));
      }
    }
  });

  testWidgets('workdir_picker_disabled_while_running', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: runningClient());
    expect(HButtonFinder(tester, 'sandbox-workdir').enabled, isFalse);
  });

  testWidgets('workdir_picker_asks_the_daemon_instead_of_computing', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpSandbox(
      tester,
      client: client,
      chosenDirectory: '/home/nik/clients/beta',
    );

    await tester.tap(find.byKey(const Key('sandbox-workdir')));
    await tester.pump();
    await tester.pump();

    expect(client.plans, contains(('/home/nik/clients/beta', null)));
    final Text sentence = tester.widget<Text>(
      find.byKey(const Key('sandbox-mounts-sentence')),
    );
    expect(sentence.data, contains('/home/nik/clients/beta'));
    // Und die Kommandozeile kam mit; sie ist die Quelle der Tabelle.
    expect(statusOf(tester).argvPreview, contains('/home/nik/clients/beta'));
  });

  testWidgets('a_cancelled_picker_changes_nothing', (
    WidgetTester tester,
  ) async {
    final SandboxTestClient client = SandboxTestClient();
    await pumpSandbox(tester, client: client);

    await tester.tap(find.byKey(const Key('sandbox-workdir')));
    await tester.pump();
    await tester.pump();

    expect(client.plans, isEmpty);
    expect(statusOf(tester).workDirHost, FakeDaemonClient.defaultWorkDir);
  });

  testWidgets('the_state_is_never_colour_alone', (WidgetTester tester) async {
    await pumpSandbox(tester, client: runningClient());
    expect(
      find.descendant(
        of: find.byType(SandboxStateIndicator),
        matching: find.text('Running'),
      ),
      findsOneWidget,
    );
  });

  testWidgets('the_terminal_and_the_isolation_panel_say_what_is_coming', (
    WidgetTester tester,
  ) async {
    await pumpSandbox(tester, client: SandboxTestClient());
    expect(
      find.textContaining("The agent's terminal belongs here"),
      findsOneWidget,
    );

    await openTab(tester, 'sandbox-tab-isolation');
    expect(find.textContaining('the three guarantees'), findsOneWidget);
  });
}

/// Liest den Zustand eines Controls, das über seinen Schlüssel gefunden wird.
class HButtonFinder {
  /// Sucht das Control mit [key] im Baum von [tester].
  HButtonFinder(this.tester, this.key);

  /// Der Baum.
  final WidgetTester tester;

  /// Der Schlüssel des Controls.
  final String key;

  /// Ob das Control eine Handlung hat.
  bool get enabled {
    final Finder finder = find.byKey(Key(key));
    expect(finder, findsOneWidget, reason: 'the control $key must be there');
    final Widget widget = tester.widget(finder);
    return (widget as dynamic).onPressed != null;
  }

  /// Ob das Control den Fokus hat.
  bool get hasFocus {
    final Finder focus = find.descendant(
      of: find.byKey(Key(key)),
      matching: find.byType(Focus),
    );
    for (final Element element in focus.evaluate()) {
      final Focus widget = element.widget as Focus;
      final FocusNode? node = widget.focusNode;
      if (node != null && node.hasFocus) {
        return true;
      }
    }
    return false;
  }
}
