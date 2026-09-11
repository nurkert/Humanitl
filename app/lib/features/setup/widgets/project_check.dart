/// The third row: the folder the agent works in (HUM-044, HUM-040).
///
/// Choosing a folder starts nothing. It asks the daemon what a start with that
/// folder would mount (`Sandbox(Plan)`), and the answer decides this row: a
/// blocking finding forbids the start, and which findings are blocking is
/// decided in the daemon (`SANDBOX_006` for a folder it will not mount). The
/// screen judges nothing about the folder -- not whether it exists, not
/// whether it is writable, not where it lies (ADR-018).
///
/// The chosen folder travels in `Sandbox(Plan)` and `Sandbox(Start)` and is
/// not written into `config.toml`: `SetConfig` accepts only a CA variable
/// under `sandbox.env` since HUM-151 and refuses `sandbox.work_dir` with
/// `CONFIG_014` until HUM-069 (CONVENTIONS 4.17).
library;

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/work_dir_picker.dart';
import '../../../l10n/l10n.dart';
import '../providers/setup_provider.dart';
import '../setup_text.dart';
import 'setup_check_row.dart';

/// The project row.
class ProjectCheck extends StatelessWidget {
  /// Creates the row for [check].
  const ProjectCheck({
    required this.check,
    required this.workMode,
    required this.locked,
    required this.onPick,
    required this.onMode,
    super.key,
  });

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
  Widget build(BuildContext context) {
    final AppLocalizations l10n = context.l10n;
    return SetupCheckRow(
      check: check,
      title: setupCheckTitle(l10n, SetupCheckKind.project),
      // Der Pfad steht im Knopf des Pickers und ist zugleich der Beleg dieser
      // Zeile; ihn daneben ein zweites Mal zu zeigen, wäre dieselbe Aussage
      // zweimal. Deshalb ersetzt der Picker die Belegzeile, statt neben ihr
      // zu stehen.
      detail: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          WorkDirPicker(
            buttonKey: const Key('setup-workdir'),
            workDir: check.detail.isEmpty ? null : check.detail,
            workMode: workMode,
            chooseLabel: l10n.sandboxWorkDirChoose,
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
