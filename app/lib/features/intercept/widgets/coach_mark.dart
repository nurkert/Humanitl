/// The one sentence that explains the action bar, once (HUM-044).
///
/// # Four things it has to get right
///
/// - **It appears at the first held request and never again.** The flag is
///   written the first time it is on screen, not when it is dismissed
///   (`providers/coach_mark.dart`): somebody who decides instead of clicking
///   the hint away has read it too.
/// - **It cannot cover the decision it explains.** It is a sibling of the
///   action bar in the same column, not a layer over the screen, so there is
///   no geometry that could go wrong -- the bar is simply somewhere else.
/// - **It is closed by a click and by `Esc`,** and by nothing being decided:
///   it never blocks a key, and it takes no focus of its own
///   (`packages/ui/src/widgets/h_popover.dart`).
/// - **It does not come back after a restart.** The flag lives in a file, not
///   in memory.
library;

import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/coach_mark.dart';

/// The hint above the action bar.
class CoachMark extends ConsumerStatefulWidget {
  /// Creates the hint for [flow]; a flow that is not held shows none.
  const CoachMark({required this.flow, super.key});

  /// The selected request, or null while nothing is selected.
  final Flow? flow;

  @override
  ConsumerState<CoachMark> createState() => _CoachMarkState();
}

class _CoachMarkState extends ConsumerState<CoachMark> {
  /// True once this run has decided to show the hint.
  ///
  /// It stays true after the flag has been written, which is what keeps the
  /// hint on screen instead of vanishing in the same frame it appeared.
  /// Whether it is still *open* is [coachMarkVisibleProvider], because `Esc`
  /// closes it and that key arrives where the focus is: at the screen.
  bool _shown = false;

  void _close() => ref.read(coachMarkVisibleProvider.notifier).close();

  @override
  Widget build(BuildContext context) {
    final bool seen = ref.watch(coachMarkSeenProvider);
    final bool held = widget.flow?.isHeld ?? false;
    if (!_shown && !seen && held) {
      _shown = true;
      // Nicht im Aufbau: Beide Aufrufe ändern einen Provider, und ein
      // Provider, der sich während des Baus ändert, baut den Baum ein zweites
      // Mal.
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) {
          ref.read(coachMarkVisibleProvider.notifier).show();
          ref.read(coachMarkSeenProvider.notifier).markShown();
        }
      });
    }
    if (!ref.watch(coachMarkVisibleProvider)) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Padding(
      padding: EdgeInsets.fromLTRB(
        tokens.spacing.x3,
        0,
        tokens.spacing.x3,
        tokens.spacing.x2,
      ),
      child: Align(
        alignment: Alignment.centerLeft,
        child: HPopover(
          key: const Key('intercept-coach-mark'),
          title: l10n.setupCoachFirstHoldTitle,
          body: l10n.setupCoachFirstHold,
          closeLabel: l10n.setupCoachClose,
          onClose: _close,
        ),
      ),
    );
  }
}
