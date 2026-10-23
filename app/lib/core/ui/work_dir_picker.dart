/// The project folder and how the agent may use it (HUM-040, HUM-044).
///
/// This is the one control of the product that changes what the agent gets, so
/// it says what it will do before it does it: choosing a folder starts
/// nothing. What it does with the choice is the caller's business -- the
/// sandbox screen asks the daemon what a start with that folder would mount,
/// the setup screen does the same for its project row. Nothing is computed
/// here (ADR-018).
///
/// The path is a label and the dialog has a button of its own (HUM-282). On
/// v0.0.6 the path itself was a ghost button, and nobody recognised it as the
/// way to choose a folder: a control has to look like one.
///
/// It lives in `core/ui` and not in a feature because two features draw it,
/// and no feature imports another one (ARCHITECTURE 5, `tools/check-deps.sh`).
/// That is also why it takes callbacks instead of reaching into a provider:
/// a control in `core/ui` that knew the sandbox provider would have moved the
/// import rather than removed it.
library;

import 'dart:async';
import 'dart:math' as math;

import 'package:file_picker/file_picker.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../domain/domain.dart';
import '../ipc/client_diagnostics.dart';
import 'hover_label.dart';
import 'ui.dart';

/// Asks the desktop for a directory. Replaced in tests.
typedef DirectoryChooser = Future<String?> Function();

/// The chooser the picker uses. `file_picker` speaks to the XDG desktop
/// portal on Linux; a widget test has no portal, so the test overrides this
/// provider instead of the whole picker.
final Provider<DirectoryChooser> directoryChooserProvider =
    Provider<DirectoryChooser>((Ref ref) => FilePicker.getDirectoryPath);

/// What went wrong the last time the folder dialog was asked for, or null.
///
/// [chooseWorkDir] sets it when the platform throws instead of answering, and
/// clears it on the next attempt. The screens that draw a chooser show it as
/// a card (`UI_003`, HUM-282): a button that does nothing when pressed, with
/// no word why, would be the dead end this issue removes.
final NotifierProvider<FolderDialogFailure, Diagnostic?>
folderDialogFailureProvider =
    NotifierProvider<FolderDialogFailure, Diagnostic?>(FolderDialogFailure.new);

/// The notifier behind [folderDialogFailureProvider].
class FolderDialogFailure extends Notifier<Diagnostic?> {
  @override
  Diagnostic? build() => null;

  /// Records that the dialog failed with [error].
  void failed(Object error) =>
      state = ClientDiagnostics.folderDialogFailed(error);

  /// Forgets an earlier failure.
  void clear() => state = null;
}

/// The compact project-folder control.
class WorkDirPicker extends ConsumerWidget {
  /// Creates the picker.
  ///
  /// The labels come in already localised, like every other label in
  /// `core/ui`; [lockedLabel] is the sentence a locked control shows on hover,
  /// and passing it is what locks the control (`docs/UX.md` 5.3: a control
  /// that is off says why on itself).
  const WorkDirPicker({
    required this.workDir,
    required this.workMode,
    required this.chooseLabel,
    required this.chooseButtonLabel,
    required this.changeButtonLabel,
    required this.noneLabel,
    required this.readOnlyLabel,
    required this.readWriteLabel,
    required this.onPick,
    required this.onMode,
    this.lockedLabel,
    this.buttonKey = const Key('sandbox-workdir'),
    this.reflow = false,
    super.key,
  });

  /// The folder the daemon last named, or null when it named none.
  final String? workDir;

  /// How that folder would be mounted.
  final WorkMode workMode;

  /// Screen-reader label of the folder button.
  final String chooseLabel;

  /// What the button reads while no folder is known ("Choose folder…").
  final String chooseButtonLabel;

  /// What the button reads once a folder is known ("Change…").
  final String changeButtonLabel;

  /// What the path label reads while no folder is known.
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

  /// Whether the row may break into lines.
  ///
  /// The sandbox header is one row that never reflows, so there the path
  /// gives way and is cut. The setup row has room below it: there button and
  /// switch move under the path when they do not fit beside it, at double
  /// text size for instance, instead of running over the edge
  /// (`docs/UX.md` 6).
  final bool reflow;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final String? locked = lockedLabel;
    final String? dir = workDir == null || workDir!.isEmpty ? null : workDir;
    final Widget text = Text(
      dir ?? noneLabel,
      key: ValueKey<String>('${_keyName(buttonKey)}-path'),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tokens.typography.ui13.tinted(
        dir == null ? tokens.colors.fg1 : tokens.colors.fg0,
      ),
    );
    // Der ganze Pfad steht beim Verweilen da, auch wenn die Zeile ihn kürzt.
    final Widget path = dir == null
        ? text
        : HoverLabel(label: dir, side: HoverLabelSide.below, child: text);
    final String label = dir == null ? chooseButtonLabel : changeButtonLabel;
    final Widget glyph = HGlyphIcon(
      HGlyph.folder,
      size: HSize.glyph,
      color: locked != null ? tokens.colors.fg2 : tokens.colors.fg0,
    );
    final VoidCallback? choose = locked != null
        ? null
        : () => unawaited(chooseWorkDir(ref, onPick));
    final Widget button = HButton(
      key: buttonKey,
      onPressed: choose,
      semanticsLabel: chooseLabel,
      leading: glyph,
      child: Text(label, maxLines: 1),
    );
    final Widget mode = HSegmented<WorkMode>(
      options: <HSegmentOption<WorkMode>>[
        HSegmentOption<WorkMode>(value: WorkMode.ro, label: readOnlyLabel),
        HSegmentOption<WorkMode>(value: WorkMode.rw, label: readWriteLabel),
      ],
      selected: workMode,
      enabled: locked == null,
      onSelect: onMode,
    );
    final Widget controls = reflow
        ? Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: tokens.spacing.x2,
            runSpacing: tokens.spacing.x1,
            children: <Widget>[button, mode],
          )
        : Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              button,
              SizedBox(width: tokens.spacing.x2),
              mode,
            ],
          );
    // Gesperrt sagt der Knopf mit dem Schalter, warum; der Pfad bleibt
    // lesbar und behält sein eigenes Hover.
    final Widget lockable = locked == null
        ? controls
        : HoverLabel(label: locked, child: controls);
    if (reflow) {
      return Wrap(
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: tokens.spacing.x2,
        runSpacing: tokens.spacing.x1,
        children: <Widget>[path, lockable],
      );
    }
    return LayoutBuilder(
      builder: (BuildContext context, BoxConstraints constraints) {
        // Eine Kopfzeile bricht nicht um. Wird sie so schmal, dass Pfad,
        // beschrifteter Knopf und Schalter nicht mehr nebeneinander passen,
        // bleibt vom Knopf das Ordnersymbol; das Wort steht dann beim
        // Verweilen daneben, und der Bildschirmleser liest es weiter.
        final bool compact =
            constraints.maxWidth <
            MediaQuery.textScalerOf(context).scale(compactBelow);
        Widget row = lockable;
        if (compact) {
          final Widget iconButton = HButton(
            key: buttonKey,
            onPressed: choose,
            semanticsLabel: chooseLabel,
            child: glyph,
          );
          row = Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              if (locked == null)
                HoverLabel(
                  label: label,
                  side: HoverLabelSide.below,
                  child: iconButton,
                )
              else
                iconButton,
              SizedBox(width: tokens.spacing.x2),
              mode,
            ],
          );
          row = locked == null ? row : HoverLabel(label: locked, child: row);
          // Letzter Ausweg bei doppelter Textgröße in einem schmalen Kopf:
          // Passt selbst Symbol und Schalter nicht mehr, wird die Gruppe
          // kleiner gezeichnet, statt über den Rand zu laufen
          // (`docs/UX.md` 6). Solange sie passt, ändert `scaleDown` nichts.
          row = ConstrainedBox(
            constraints: BoxConstraints(
              maxWidth: math.max(0, constraints.maxWidth - tokens.spacing.x2),
            ),
            child: FittedBox(
              fit: BoxFit.scaleDown,
              alignment: Alignment.centerRight,
              child: row,
            ),
          );
        }
        return Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            // Der Pfad ist das einzige Element der Zeile, das beliebig lang
            // werden kann. Er gibt deshalb als erstes nach; Knopf und
            // Schalter behalten ihre Breite.
            Flexible(child: path),
            SizedBox(width: tokens.spacing.x2),
            row,
          ],
        );
      },
    );
  }

  /// Below this width, at normal text size, the header row shows the folder
  /// button as its glyph alone.
  ///
  /// Measured on the sandbox header: the switch takes about 355 px and the
  /// labelled button about 140 px, so 600 px leave the path at least 100 px
  /// before the label goes. At a window of 900 px the header gives the
  /// picker 459 px, and the labelled row ran 51 px over the edge.
  static const double compactBelow = 600;

  static String _keyName(Key key) => switch (key) {
    ValueKey<String>(:final String value) => value,
    _ => key.toString(),
  };
}

/// Asks the desktop for a folder and hands a chosen one to [onPick].
///
/// The one way this client chooses a project folder. The button of
/// [WorkDirPicker] takes it, and so do the fixes of a missing or refused
/// folder (`CONFIG_013`, `SANDBOX_006`, HUM-262, HUM-282): a second chooser
/// beside it would be a second place where a cancelled dialog could be read
/// differently.
///
/// A dialog that throws instead of answering -- no desktop portal, a portal
/// that fails -- lands in [folderDialogFailureProvider], and the screen shows
/// it with the command line as the way around (`UI_003`).
Future<void> chooseWorkDir(
  WidgetRef ref,
  void Function(String workDir) onPick,
) async {
  final DirectoryChooser chooser = ref.read(directoryChooserProvider);
  // Vor dem `await` gelesen: Nach dem Dialog kann das Widget, das gefragt
  // hat, schon fort sein, und ein `ref` danach wirft.
  final FolderDialogFailure failure = ref.read(
    folderDialogFailureProvider.notifier,
  )..clear();
  final String? chosen;
  try {
    chosen = await chooser();
  } on Object catch (error) {
    failure.failed(error);
    return;
  }
  // Under a Wayland portal a cancelled dialog answers null. That is an
  // answer, not a failure: nothing changes and nothing is reported
  // (HUM-040 Fallstricke).
  if (chosen == null || chosen.isEmpty) {
    return;
  }
  onPick(chosen);
}
