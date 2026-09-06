/// The project folder and how the agent may use it (HUM-040, HUM-044).
///
/// This is the one control of the product that changes what the agent gets, so
/// it says what it will do before it does it: choosing a folder starts
/// nothing. What it does with the choice is the caller's business -- the
/// sandbox screen asks the daemon what a start with that folder would mount,
/// the setup screen does the same for its project row. Nothing is computed
/// here (ADR-018).
///
/// It lives in `core/ui` and not in a feature because two features draw it,
/// and no feature imports another one (ARCHITECTURE 5, `tools/check-deps.sh`).
/// That is also why it takes callbacks instead of reaching into a provider:
/// a control in `core/ui` that knew the sandbox provider would have moved the
/// import rather than removed it.
library;

import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import 'hover_label.dart';
import 'ui.dart';

/// Asks the desktop for a directory. Replaced in tests.
typedef DirectoryChooser = Future<String?> Function();

/// The chooser the picker uses. `file_picker` speaks to the XDG desktop
/// portal on Linux; a widget test has no portal, so the test overrides this
/// provider instead of the whole picker.
final Provider<DirectoryChooser> directoryChooserProvider =
    Provider<DirectoryChooser>((Ref ref) => FilePicker.getDirectoryPath);

/// The compact project-folder control.
class WorkDirPicker extends ConsumerWidget {
  /// Creates the picker.
  ///
  /// [chooseLabel] and [noneLabel] come in already localised, like every other
  /// label in `core/ui`; [lockedLabel] is the sentence a locked control shows
  /// on hover, and passing it is what locks the control (`docs/UX.md` 5.3: a
  /// control that is off says why on itself).
  const WorkDirPicker({
    required this.workDir,
    required this.workMode,
    required this.chooseLabel,
    required this.noneLabel,
    required this.readOnlyLabel,
    required this.readWriteLabel,
    required this.onPick,
    required this.onMode,
    this.lockedLabel,
    this.buttonKey = const Key('sandbox-workdir'),
    super.key,
  });

  /// The folder the daemon last named, or null when it named none.
  final String? workDir;

  /// How that folder would be mounted.
  final WorkMode workMode;

  /// Screen-reader label of the folder button.
  final String chooseLabel;

  /// What the button reads while no folder is known.
  final String noneLabel;

  /// Label of the read-only segment.
  final String readOnlyLabel;

  /// Label of the read-write segment.
  final String readWriteLabel;

  /// Called with the folder somebody chose. A cancelled dialog calls nothing.
  final void Function(String workDir) onPick;

  /// Called with the mode somebody chose.
  final void Function(WorkMode mode) onMode;

  /// Why the control is locked, or null while it is not.
  final String? lockedLabel;

  /// Key of the folder button, for a screen that draws more than one.
  final Key buttonKey;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final bool locked = lockedLabel != null;
    final String? dir = workDir;
    final Widget row = Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        // Der Pfad ist das einzige Element der Zeile, das beliebig lang werden
        // kann. Er gibt deshalb als erstes nach; der ganze Pfad steht im Satz
        // des Einhänge-Reiters und in der Kommandozeile.
        Flexible(
          child: HButton(
            key: buttonKey,
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: locked ? null : () => unawaited(_choose(ref)),
            semanticsLabel: chooseLabel,
            child: Text(
              dir == null || dir.isEmpty ? noneLabel : dir,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        HSegmented<WorkMode>(
          options: <HSegmentOption<WorkMode>>[
            HSegmentOption<WorkMode>(value: WorkMode.ro, label: readOnlyLabel),
            HSegmentOption<WorkMode>(value: WorkMode.rw, label: readWriteLabel),
          ],
          selected: workMode,
          enabled: !locked,
          onSelect: onMode,
        ),
      ],
    );
    if (lockedLabel case final String label) {
      return HoverLabel(label: label, child: row);
    }
    return row;
  }

  Future<void> _choose(WidgetRef ref) async {
    final DirectoryChooser chooser = ref.read(directoryChooserProvider);
    final String? chosen = await chooser();
    // Under a Wayland portal a cancelled dialog answers null. That is an
    // answer, not a failure: nothing changes and nothing is reported
    // (HUM-040 Fallstricke).
    if (chosen == null || chosen.isEmpty) {
      return;
    }
    onPick(chosen);
  }
}
