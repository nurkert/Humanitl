/// The keyboard vocabulary of the Intercept section (CONVENTIONS 3.9).
///
/// The intents that other sections also know -- allow, block, move -- live in
/// `core/shortcuts` and are re-exported here, so a widget of this feature
/// imports one file. The intents that only this screen has (the remember grid)
/// are declared here.
///
/// [interceptShortcuts] is the whole map, and every activator in it has an
/// action in `InterceptScreen`. A binding without an action is deleted, not
/// silenced (`docs/UX.md` 5.3): `/` and `Ctrl+D` were bound and mute and are
/// therefore gone until the screen that owns them arrives -- the filter and
/// the domain pane, each with its own issue. `N` came back with HUM-072, and
/// the group keys came with HUM-029.
library;

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

import '../../core/shortcuts/intents.dart'
    show
        withCommandOnMac,
        AllowIntent,
        BlockIntent,
        EditIntent,
        NextFlowIntent,
        NoteIntent,
        PrevFlowIntent;
import '../../core/shortcuts/shortcut_catalog.dart';

export '../../core/shortcuts/intents.dart'
    show
        withCommandOnMac,
        AllowIntent,
        BlockIntent,
        EditIntent,
        NextFlowIntent,
        NoteIntent,
        PrevFlowIntent,
        isTextInputFocused;

/// Close the one-time hint above the action bar. `Esc` (HUM-044).
///
/// It is bound on the screen and not inside the hint: `Esc` arrives where the
/// focus is, and the hint takes no focus -- one that did would take the first
/// tab stop away from the decision it explains (`docs/UX.md` 5.2). The action
/// is disabled while no hint is open, so `Esc` falls through to whatever else
/// wants it.
class CloseCoachMarkIntent extends Intent {
  /// Creates the intent.
  const CloseCoachMarkIntent();
}

/// Open the remember grid without deciding anything. `Shift+Enter`.
class OpenRememberIntent extends Intent {
  /// Creates the intent.
  const OpenRememberIntent();
}

/// Choose how long a remembered decision holds. `1` to `4`.
class RememberDurationIntent extends Intent {
  /// Creates the intent for the segment at [index], zero based.
  const RememberDurationIntent(this.index);

  /// Index into `RememberDuration.values`.
  final int index;
}

/// Choose what a remembered decision covers. `Shift+1` to `Shift+4`.
class RememberTargetIntent extends Intent {
  /// Creates the intent for the segment at [index], zero based.
  const RememberTargetIntent(this.index);

  /// Index into `RememberTarget.values`.
  final int index;
}

/// Send every selected request, or the whole group under the cursor.
/// `Ctrl+Shift+F` (CONVENTIONS 3.9).
class AllowGroupIntent extends Intent {
  /// Creates the intent.
  const AllowGroupIntent();
}

/// Refuse every selected request, or the whole group under the cursor.
/// `Ctrl+Shift+L` (CONVENTIONS 3.9).
class BlockGroupIntent extends Intent {
  /// Creates the intent.
  const BlockGroupIntent();
}

/// Select every request of the group the cursor stands in. `Ctrl+A`.
class SelectGroupIntent extends Intent {
  /// Creates the intent.
  const SelectGroupIntent();
}

/// Fold the group the cursor stands in, or open it. `ArrowLeft`, `ArrowRight`.
///
/// The chevron of a group header is a pointer target; these two keys are its
/// keyboard half, so that folding never needs the mouse (`docs/UX.md` 5.1).
class ToggleGroupIntent extends Intent {
  /// Creates the intent; [open] says which of the two directions is meant.
  const ToggleGroupIntent({required this.open});

  /// True opens the group, false folds it.
  final bool open;
}

/// Take the arrivals that waited into the list. `Shift+J`.
///
/// Without a key the pill of `docs/UX.md` 2.8 would be reachable by pointer
/// only, and a keyboard user could never merge what arrived while they read.
class MergeArrivalsIntent extends Intent {
  /// Creates the intent.
  const MergeArrivalsIntent();
}

/// Send the request of the findings pause as it is. `S` (HUM-049).
class SendAnywayIntent extends Intent {
  /// Creates the intent.
  const SendAnywayIntent();
}

/// Open the editor from the findings pause with every open finding replaced.
/// `P` (HUM-049).
class PseudonymizeIntent extends Intent {
  /// Creates the intent.
  const PseudonymizeIntent();
}

/// Close the findings pause without deciding anything. `Esc` (HUM-049).
class CloseFindingsPauseIntent extends Intent {
  /// Creates the intent.
  const CloseFindingsPauseIntent();
}

/// The bindings that only live while the findings pause is open (HUM-049).
///
/// A map of their own, bound by a `Shortcuts` between the screen's actions and
/// its focus node: that one is asked first, and while no pause is open its
/// actions are disabled, so every key falls through to [interceptShortcuts].
/// `Esc` needs that order -- it also closes the one-time hint -- and `S` and
/// `P` stay free for the note field, which the actions give back as well
/// (`docs/UX.md` 5.2). `B` needs no binding here: the block key of the screen
/// already blocks the request the pause stands over.
Map<ShortcutActivator, Intent> findingsPauseShortcuts() =>
    // Keine Wiederholungen: Ein festgehaltenes `P` oder `Esc` öffnet den
    // Editor nicht mehrmals und schließt nach der Pause nicht auch noch den
    // Hinweis darunter. Das steht an den Aktivatoren des Katalogs.
    AppShortcut.bind<Intent>(<AppShortcut, Intent>{
      AppShortcut.sendAnyway: const SendAnywayIntent(),
      AppShortcut.pseudonymize: const PseudonymizeIntent(),
      AppShortcut.closeFindingsPause: const CloseFindingsPauseIntent(),
    });

/// The digits of the two segmented controls, in segment order. Read from
/// the catalogue ([AppShortcut.rememberDuration], HUM-420).
final List<LogicalKeyboardKey> rememberKeys = <LogicalKeyboardKey>[
  for (final ShortcutActivator activator
      in AppShortcut.rememberDuration.activators)
    (activator as SingleActivator).trigger,
];

/// The bindings of the Intercept section.
///
/// The two decision keys carry `includeRepeats: false`: a key repeat is the
/// keyboard telling us that a finger has not moved, and a finger that has not
/// moved has not read the next URL (`docs/UX.md` 5.4). The screen refuses a
/// repeat a second time, because a test harness -- and some platforms -- can
/// deliver a plain down event for what a person meant as one press. The
/// activators, and with them `includeRepeats`, come from the catalogue
/// (HUM-420); `Ctrl+Enter` blocks with the note from inside the field: a
/// chord works where a single letter must not (HUM-072). `Esc` carries no
/// repeats either: a held `Esc` that closes the pause with open findings must
/// not close the hint with its repeat as well (HUM-049).
Map<ShortcutActivator, Intent> interceptShortcuts() =>
    withCommandOnMac(<ShortcutActivator, Intent>{
      ...AppShortcut.bind<Intent>(<AppShortcut, Intent>{
        AppShortcut.allow: const AllowIntent(),
        AppShortcut.allowAnywhere: const AllowIntent(chord: true),
        AppShortcut.block: const BlockIntent(),
        AppShortcut.blockAnywhere: const BlockIntent(chord: true),
        AppShortcut.nextRequest: const NextFlowIntent(),
        AppShortcut.previousRequest: const PrevFlowIntent(),
        AppShortcut.openRemember: const OpenRememberIntent(),
        AppShortcut.note: const NoteIntent(),
        AppShortcut.edit: const EditIntent(),
        AppShortcut.closeHint: const CloseCoachMarkIntent(),
        AppShortcut.allowGroup: const AllowGroupIntent(),
        AppShortcut.blockGroup: const BlockGroupIntent(),
        AppShortcut.selectGroup: const SelectGroupIntent(),
        AppShortcut.mergeArrivals: const MergeArrivalsIntent(),
        AppShortcut.openGroup: const ToggleGroupIntent(open: true),
        AppShortcut.foldGroup: const ToggleGroupIntent(open: false),
      }),
      for (int i = 0; i < AppShortcut.rememberDuration.activators.length; i++)
        AppShortcut.rememberDuration.activators[i]: RememberDurationIntent(i),
      for (int i = 0; i < AppShortcut.rememberTarget.activators.length; i++)
        AppShortcut.rememberTarget.activators[i]: RememberTargetIntent(i),
    });
