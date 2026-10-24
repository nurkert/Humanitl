/// The third row: the folder the agent works in (HUM-044, HUM-040).
///
/// Choosing a folder starts nothing. It stores the folder as
/// `sandbox.work_dir` in `config.toml` (`SetConfig`) and then asks the daemon
/// what a start with it would mount (`Sandbox(Plan)`); the answer decides this
/// row: a blocking finding forbids the start, and which findings are blocking
/// is decided in the daemon (`SANDBOX_006` for a folder it will not mount).
/// The screen judges nothing about the folder -- not whether it exists, not
/// whether it is writable, not where it lies (ADR-018).
///
/// The folder is chosen once (HUM-293): the app, the desktop launcher and
/// `humanitl sandbox run` all read it from `config.toml`, and `--work`
/// overrides it for one call only. The daemon checks the folder before it
/// writes it; a refused one is not stored, and its `SANDBOX_006` stands in
/// this row. No folder travels in `Sandbox(Plan)` or `Sandbox(Start)` any
/// more, so there is no second, in-memory copy that could disagree with the
/// file.
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/work_dir_picker.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The project row.
///
/// A folder the daemon refuses (`SANDBOX_006`) carries the fix "choose another
/// folder", and the card under this row turns it into a button that opens the
/// same chooser as the picker above it (HUM-262). A folder nobody has chosen
/// yet (`CONFIG_013`) carries the same fix as "choose folder" (HUM-282).
/// Which folder is refused, and why, is the daemon's sentence; the row only
/// offers the way out. A folder dialog that does not open stands as its own
/// card above the row's finding (`UI_003`), with the command line as the way
/// around it.
class ProjectCheck extends ConsumerWidget {
  /// Creates the row for [check].
  const ProjectCheck({
    required this.check,
    required this.workMode,
    required this.locked,
    required this.onPick,
    required this.onMode,
    this.view = SetupStepView.focused,
    super.key,
  });

  /// Open or one line, in front or not (HUM-287).
  final SetupStepView view;

  /// What the row says.
  final SetupCheck check;

  /// How the folder would be mounted.
  final WorkMode workMode;

  /// True while the mount cannot change any more, because a sandbox is up.
  final bool locked;

  /// Called with the folder somebody chose.
  final void Function(String workDir) onPick;

  /// Called with the mode somebody chose.
  final void Function(WorkMode mode) onMode;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    // Die Karte eines gescheiterten Ordnerdialogs steht nur, solange der
    // Ordner wählbar ist; mit einer laufenden Sitzung ist der Ausweg schon
    // genommen (HUM-282).
    final Diagnostic? dialogFailure = locked
        ? null
        : ref.watch(folderDialogFailureProvider);
    final VoidCallback? choose = locked
        ? null
        : () => unawaited(chooseWorkDir(ref, onPick));
    final Diagnostic? diagnostic = check.diagnostic;
    // Verlangt der Befund einen (anderen) Ordner, trägt sein Knopf die eine
    // Wahl dieses Schritts. Der Picker träte mit einem zweiten Knopf für
    // dieselbe Wahl daneben; er steht deshalb erst wieder, wenn ein Ordner
    // gilt (Owner, 2026-10-24).
    final bool fixChooses =
        diagnostic != null &&
        FixControl.offersButton(context, diagnostic.fix, chooseFolder: true);
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.project),
      view: view,
      // Der Ordnerknopf des Befunds (HUM-282) ist die eine Aktion dieses
      // Schritts, wenn der Befund einen anderen Ordner verlangt (HUM-287).
      primaryFix: true,
      // Steht der Schritt nicht vorn, bleibt seine Abhilfe trotzdem einen
      // Klick entfernt: derselbe Ordnerknopf auf der einen Zeile.
      lineAction: fixChooses
          ? FixControl(
              fix: diagnostic.fix,
              onChooseFolder: choose,
              chooseFolderLocked: locked ? l10n.sandboxWorkDirLocked : null,
              diagnosticCode: diagnostic.code,
            )
          : null,
      // Gesperrt wie der Picker: Solange eine Sandbox läuft, ändert sich der
      // Ordner nicht, und ein Knopf, der dann trotzdem wählen ließe, täte
      // nichts.
      onChooseFolder: choose,
      // Gesperrt bleibt der Knopf der Karte stehen und sagt beim Verweilen
      // denselben Satz wie der Picker (`docs/UX.md` 5.3).
      chooseFolderLocked: locked ? l10n.sandboxWorkDirLocked : null,
      notice: dialogFailure,
      // Der Pfad steht im Picker und ist zugleich der Beleg dieser Zeile; ihn
      // daneben ein zweites Mal zu zeigen, wäre dieselbe Aussage zweimal.
      // Deshalb ersetzt der Picker die Belegzeile, statt neben ihr zu stehen.
      body: fixChooses
          ? null
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                WorkDirPicker(
                  buttonKey: const Key('setup-workdir'),
                  reflow: true,
                  workDir: check.detail.isEmpty ? null : check.detail,
                  workMode: workMode,
                  chooseLabel: l10n.sandboxWorkDirChoose,
                  chooseButtonLabel: l10n.workDirChooseButton,
                  changeButtonLabel: l10n.workDirChangeButton,
                  noneLabel: l10n.sandboxWorkDirNone,
                  readOnlyLabel: l10n.sandboxWorkModeRo,
                  readWriteLabel: l10n.sandboxWorkModeRw,
                  lockedLabel: locked ? l10n.sandboxWorkDirLocked : null,
                  onPick: onPick,
                  onMode: onMode,
                ),
                // Ohne Ordner sperrt die Zeile den Start. Der Grund steht dann als
                // Karte unter der Zeile wie jeder andere Befund auch (`CONFIG_013`,
                // gebaut im Client wie `DAEMON_001`), und nicht als Satz daneben:
                // zwei Orte für dieselbe Aussage sind einer zu viel.
              ],
            ),
    );
  }
}
