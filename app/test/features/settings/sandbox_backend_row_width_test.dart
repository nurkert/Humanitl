// Grund und Befehl eines fehlenden Backends in voller Breite (HUM-494).
//
// Mit vier Segmenten (automatisch, das Backend des Systems, die Container-Engine und das
// VM-Backend) blieb der Textspalte neben der Wahl etwa ein Drittel der Zeile:
// Der Grund einer fehlenden Voraussetzung lief über ein Dutzend Zeilen, der
// Befehl zum Kopieren neben seinem Knopf schrumpfte auf einen Streifen. Seit
// HUM-494 stehen die Zeilen der Backends unter Hilfe und Wahl, so breit wie
// die ganze Zeile, und ein langer Befehl bricht um, statt mit einer Ellipse
// zu enden.
//
// Gemessen wird im Sheet eines 900 px breiten Fensters, auf Englisch und
// Deutsch, mit Roboto (`harness/layout_fonts.dart`): Mit den Quadraten der
// Testschrift wäre die Wahl doppelt so breit wie auf dem Bildschirm und
// rutschte schon allein unter den Text, die alte Anordnung fiele dann nicht
// auf. Mutationsbeweise: `SandboxBackendLines` zurück in `status` der
// `SettingRow` (die Textspalte neben der Wahl) macht den ersten Test rot,
// die Ellipse zurück am Befehl (`wrapText: false`) den zweiten.

import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:humanitl/core/domain/domain.dart';
import 'package:humanitl/core/ipc/fake_daemon_client.dart';
import 'package:humanitl/core/ui/fix_control.dart';
import 'package:humanitl/core/ui/ui.dart';
import 'package:humanitl/features/settings/widgets/sandbox_backend_row.dart';

import '../../harness/layout_fonts.dart';
import '../../harness/settings_harness.dart';
import 'settings_overlay_test.dart' show everything, open;

/// Der Anteil der Zeilenbreite, den Grund und Befehl mindestens bekommen.
/// Neben der Wahl waren es gut die Hälfte, darunter fast alles; der Abzug ist
/// die Einrückung unter der Zeile des Backends und der Knopf vor dem Befehl.
const double fullWidth = 0.85;

/// Ein Befehl, länger als jede Zeile bei 900 px: ein Pfad zu einem
/// Kernel-Abbild, wie ihn eine fehlende Leseberechtigung nennen könnte.
const String longCommand =
    'sudo chmod 0644 /usr/lib/humanitl/kernel/'
    'vmlinuz-6.12.9-humanitl-vm-x86_64-with-a-rather-long-build-suffix';

const String kernel = FakeSandboxBackends.kernelBackendName;

/// Die Container-Engine, wie sie die Liste des Fakes nennt; ihr Name steht
/// so nur an einer Stelle der App (HUM-145).
final String engine =
    FakeSandboxBackends.defaultSandboxBackends.backends[1].name;

/// Das Backend des Systems, die Container-Engine ohne Programm und das VM-Backend ohne
/// Zugriff auf das KVM-Gerät: vier Segmente, zwei Gründe mit Befehl. Mit
/// [kernelCommand] trägt der Grund des VM-Backends diesen Befehl.
FakeDaemonClient client({String? kernelCommand}) =>
    everything()
      ..sandboxBackendList = SandboxBackendList(
        minIsolation: IsolationLevel.namespaces,
        backends: <SandboxBackendInfo>[
          FakeSandboxBackends.defaultSandboxBackends.backends.first,
          SandboxBackendInfo(
            name: engine,
            isolation: IsolationLevel.namespaces,
            diagnostics: <Diagnostic>[
              Diagnostic(
                code: 'SANDBOX_034',
                severity: Severity.blocking,
                why: 'the container engine was not found on PATH.',
                fix: FixAction.copyCommand(
                  command: 'sudo apt install container-engine',
                ),
              ),
            ],
          ),
          if (kernelCommand == null)
            FakeSandboxBackends.kernelBackendUnavailable
          else
            SandboxBackendInfo(
              name: kernel,
              isolation: IsolationLevel.kernel,
              diagnostics: <Diagnostic>[
                Diagnostic(
                  code: FakeSandboxBackends.kernelPrerequisiteMissing.code,
                  severity: Severity.blocking,
                  why: FakeSandboxBackends.kernelPrerequisiteMissing.why,
                  fix: FixAction.copyCommand(command: kernelCommand),
                ),
              ],
            ),
        ],
      );

double widthOf(WidgetTester tester, Finder finder) =>
    tester.renderObject<RenderBox>(finder).size.width;

/// Öffnet die Kategorie Sandbox im Sheet eines 900 px breiten Fensters und
/// liefert die Breite der Zeile `sandbox.backend` ohne ihren Innenabstand.
Future<double> openBackendRow(
  WidgetTester tester, {
  required bool german,
  String? kernelCommand,
}) async {
  await open(
    tester,
    client: client(kernelCommand: kernelCommand),
    german: german,
    size: const Size(900, 800),
  );
  await showSettingsCategory(tester, SettingsCategory.sandbox);
  final Finder row = find.byKey(SandboxBackendRow.rowKey);
  await tester.ensureVisible(row);
  await tester.pumpAndSettle();
  // Vier Segmente: Ohne sie gäbe es nichts, das den Text verdrängt.
  expect(
    tester
        .widget<HSegmented<String>>(
          find.byKey(const Key('settings-sandbox-backend')),
        )
        .options,
    hasLength(4),
  );
  return widthOf(
    tester,
    find.byKey(
      const ValueKey<String>('setting-row-${FormKeys.sandboxBackend}'),
    ),
  );
}

/// Der Text des Befehls neben dem Knopf des VM-Backends.
Finder commandOf(String command) => find.descendant(
  of: find.ancestor(
    of: find.byKey(const Key('settings-sandbox-backend-fix-$kernel')),
    matching: find.byType(FixControl),
  ),
  matching: find.text(command),
);

void main() {
  setUpAll(loadLayoutFonts);

  final TargetPlatformVariant desktops = TargetPlatformVariant(<TargetPlatform>{
    TargetPlatform.linux,
    TargetPlatform.macOS,
  });

  for (final bool german in <bool>[false, true]) {
    final String language = german ? 'de' : 'en';

    testWidgets('at 900 px the reason and the command of a missing '
        'prerequisite take the whole width of the row ($language)', (
      WidgetTester tester,
    ) async {
      final double rowWidth = await openBackendRow(tester, german: german);
      final Finder reason = find.byKey(
        const Key('settings-sandbox-backend-reason-$kernel'),
      );
      final String command =
          switch (FakeSandboxBackends.kernelPrerequisiteMissing.fix) {
            FixActionCopyCommand(:final String command) => command,
            _ => throw StateError('the fake reason carries no command'),
          };
      final Finder line = find.ancestor(
        of: commandOf(command),
        matching: find.byType(Row),
      );
      expect(
        widthOf(tester, reason),
        greaterThanOrEqualTo(rowWidth * fullWidth),
        reason: 'the reason is a strip beside the choice',
      );
      expect(
        widthOf(tester, line.first),
        greaterThanOrEqualTo(rowWidth * fullWidth),
        reason: 'the button and its command are a strip beside the choice',
      );
      // Breit heißt hier auch kurz: Der Grund des VM-Backends steht in
      // höchstens drei Zeilen, statt über ein Dutzend zu laufen.
      final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
        reason,
      );
      final double lineHeight = paragraph.getFullHeightForCaret(
        const TextPosition(offset: 0),
      );
      expect(paragraph.size.height, lessThanOrEqualTo(lineHeight * 3 + 1));
    }, variant: desktops);

    testWidgets('a command longer than the row wraps beside its button and '
        'is never cut off ($language)', (WidgetTester tester) async {
      final double rowWidth = await openBackendRow(
        tester,
        german: german,
        kernelCommand: longCommand,
      );
      final Finder command = commandOf(longCommand);
      expect(command, findsOneWidget);
      final RenderParagraph paragraph = tester.renderObject<RenderParagraph>(
        command,
      );
      expect(
        paragraph.didExceedMaxLines,
        isFalse,
        reason: 'the command ends in an ellipsis',
      );
      // Der Befehl ist länger als eine Zeile, steht also auf mehreren, und
      // die Zeile, in der er umbricht, ist die volle Breite neben dem Knopf.
      final double lineHeight = paragraph.getFullHeightForCaret(
        const TextPosition(offset: 0),
      );
      expect(paragraph.size.height, greaterThan(lineHeight * 1.5));
      expect(
        widthOf(tester, command),
        greaterThanOrEqualTo(rowWidth * 0.6),
        reason: 'the command is a strip beside its button',
      );
      // Das letzte Zeichen ist gelegt und sichtbar, nicht hinter einer
      // Ellipse verschwunden.
      final List<TextBox> last = paragraph.getBoxesForSelection(
        TextSelection(
          baseOffset: longCommand.length - 1,
          extentOffset: longCommand.length,
        ),
      );
      expect(last, isNotEmpty);
      expect(last.single.right, lessThanOrEqualTo(paragraph.size.width + 0.5));
      expect(
        last.single.bottom,
        lessThanOrEqualTo(paragraph.size.height + 0.5),
      );
    }, variant: desktops);
  }
}
