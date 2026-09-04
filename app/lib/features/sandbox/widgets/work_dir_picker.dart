/// The project folder and how the agent may use it (HUM-040).
///
/// This is the one control on the screen that changes what the agent gets, so
/// it says what it will do before it does it: choosing a folder does not
/// start anything, it asks the daemon what a start with that folder would
/// mount, and the table below answers. Nothing is computed here (ADR-018).
library;

import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/sandbox_status_provider.dart';
import '../sandbox_text.dart';

/// Asks the desktop for a directory. Replaced in tests.
typedef DirectoryChooser = Future<String?> Function();

/// The chooser the picker uses. `file_picker` speaks to the XDG desktop
/// portal on Linux; a widget test has no portal, so the test overrides this
/// provider instead of the whole picker.
final Provider<DirectoryChooser> directoryChooserProvider =
    Provider<DirectoryChooser>((Ref ref) => FilePicker.getDirectoryPath);

/// The compact project-folder control of the header.
class WorkDirPicker extends ConsumerWidget {
  /// Creates the picker for [status].
  const WorkDirPicker({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // While the sandbox is up the mount cannot change; a picker that answered
    // would show a folder the running agent does not have.
    final bool locked = status.isUp || status.isBusy;
    final String? dir = status.workDirHost;
    final Widget row = Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        // Der Pfad ist das einzige Element der Kopfzeile, das beliebig lang
        // werden kann. Er gibt deshalb als erstes nach; der ganze Pfad steht
        // im Satz des Einhänge-Reiters und in der Kommandozeile.
        Flexible(
          child: HButton(
            key: const Key('sandbox-workdir'),
            variant: HButtonVariant.ghost,
            size: HButtonSize.sm,
            onPressed: locked ? null : () => _choose(context, ref),
            semanticsLabel: l10n.sandboxWorkDirChoose,
            child: Text(
              dir == null || dir.isEmpty ? l10n.sandboxWorkDirNone : dir,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        HSegmented<WorkMode>(
          options: <HSegmentOption<WorkMode>>[
            HSegmentOption<WorkMode>(
              value: WorkMode.ro,
              label: l10n.sandboxWorkModeRo,
            ),
            HSegmentOption<WorkMode>(
              value: WorkMode.rw,
              label: l10n.sandboxWorkModeRw,
            ),
          ],
          selected: status.workMode,
          enabled: !locked,
          onSelect: (WorkMode mode) => unawaited(
            ref.read(sandboxStatusProvider.notifier).plan(workMode: mode),
          ),
        ),
      ],
    );
    if (!locked) {
      return row;
    }
    return HoverLabel(label: l10n.sandboxWorkDirLocked, child: row);
  }

  Future<void> _choose(BuildContext context, WidgetRef ref) async {
    final DirectoryChooser chooser = ref.read(directoryChooserProvider);
    final String? chosen = await chooser();
    // Under a Wayland portal a cancelled dialog answers null. That is an
    // answer, not a failure: nothing changes and nothing is reported
    // (HUM-040 Fallstricke).
    if (chosen == null || chosen.isEmpty) {
      return;
    }
    await ref.read(sandboxStatusProvider.notifier).plan(workDir: chosen);
  }
}

/// The sentence the mounts tab opens with, built from [status].
///
/// It lives here and not in the tab because the picker and the sentence make
/// the same claim about the same folder, and two places that phrase it
/// differently would be two claims.
String workDirSentence(AppLocalizations l10n, SandboxStatus status) {
  final MountEntry? work = status.workMount;
  final String? dir = work?.src ?? status.workDirHost;
  if (dir == null || dir.isEmpty) {
    return l10n.sandboxMountsSentenceNoWork;
  }
  final WorkMode mode = work == null
      ? status.workMode
      : (work.mode == MountMode.ro ? WorkMode.ro : WorkMode.rw);
  return l10n.sandboxMountsSentence(dir, sandboxWorkModeLabel(l10n, mode));
}
