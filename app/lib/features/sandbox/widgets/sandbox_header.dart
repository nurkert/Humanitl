/// The header of the sandbox screen: state, profile, project folder, start
/// and stop (HUM-040).
///
/// It is one row and it never reflows: the state stands on the left, the two
/// controls that change something on the right, and the same action is always
/// in the same place (CONVENTIONS 4.13, "Vorhersagbarkeit").
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/sandbox_status_provider.dart';
import '../sandbox_text.dart';
import 'work_dir_picker.dart';

/// The header row.
class SandboxHeader extends ConsumerWidget {
  /// Creates the header for [status]. [onAskStop] is called when a stop needs
  /// the confirmation modal; the screen owns it, because a modal belongs over
  /// the whole section and not inside a row.
  const SandboxHeader({
    required this.status,
    required this.onAskStop,
    super.key,
  });

  /// What the daemon last said.
  final SandboxStatus status;

  /// Asks the screen to put the stop confirmation up.
  final VoidCallback onAskStop;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return Container(
      constraints: const BoxConstraints(minHeight: HSize.headerBar),
      color: tokens.colors.bg1,
      padding: EdgeInsets.symmetric(horizontal: tokens.spacing.x3),
      child: Row(
        children: <Widget>[
          SandboxStateIndicator(status: status),
          SizedBox(width: tokens.spacing.x3),
          HBadge(
            text: status.profile.isEmpty
                ? l10n.sandboxProfileUnknown
                : l10n.sandboxProfile(status.profile),
            color: tokens.colors.fg2,
            textColor: tokens.colors.fg1,
          ),
          // Der Picker bekommt den ganzen Rest und steht rechts. Ein `Spacer`
          // daneben teilte den Platz mit ihm, und der Pfad bekäme die Hälfte
          // dessen, was da ist.
          Expanded(
            child: Align(
              alignment: Alignment.centerRight,
              child: WorkDirPicker(status: status),
            ),
          ),
          SizedBox(width: tokens.spacing.x3),
          SandboxStartStop(status: status, onAskStop: onAskStop),
        ],
      ),
    );
  }
}

/// The dot and the word that say what the sandbox is doing.
///
/// The dot carries the colour, the word carries the same statement in text: a
/// state is never colour alone (`docs/UX.md` 3.3). The colour transition is
/// the one motion of this row, and it answers exactly one of the four
/// questions -- what changed here, in place (`docs/UX.md` 2.1).
class SandboxStateIndicator extends StatelessWidget {
  /// Shows the state of [status].
  const SandboxStateIndicator({required this.status, super.key});

  /// What the daemon last said.
  final SandboxStatus status;

  /// Diameter of the dot. Half a state glyph: this is a mark, not a symbol.
  static const double dotSize = HSize.glyph / 2;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final String label = sandboxStateLabel(l10n, status.state);
    final String spoken = status.agentExited
        ? '$label, ${l10n.sandboxAgentExited}'
        : label;
    return Semantics(
      label: l10n.sandboxStateSemantics(spoken),
      child: ExcludeSemantics(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            HAnimatedFill(
              color: sandboxStateColor(tokens, status.state),
              builder: (BuildContext context, Color color) => SizedBox(
                width: HSize.hitMin,
                height: HSize.hitMin,
                child: Center(
                  child: Container(
                    width: dotSize,
                    height: dotSize,
                    decoration: BoxDecoration(
                      color: color,
                      shape: BoxShape.circle,
                    ),
                  ),
                ),
              ),
            ),
            SizedBox(width: tokens.spacing.x1),
            Text(
              label,
              style: tokens.typography.ui13.semibold.tinted(tokens.colors.fg0),
            ),
            if (status.agentExited) ...<Widget>[
              SizedBox(width: tokens.spacing.x2),
              Text(
                l10n.sandboxAgentExited,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// One control, never two: start while nothing runs, stop while it does.
class SandboxStartStop extends ConsumerWidget {
  /// Creates the control for [status].
  const SandboxStartStop({
    required this.status,
    required this.onAskStop,
    super.key,
  });

  /// What the daemon last said.
  final SandboxStatus status;

  /// Asks the screen for the confirmation modal.
  final VoidCallback onAskStop;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final AppLocalizations l10n = context.l10n;
    final Diagnostic? blocking = status.blocking;
    final bool up = status.isUp || status.state == SandboxState.stopping;
    if (up) {
      return HButton(
        key: const Key('sandbox-stop'),
        variant: HButtonVariant.secondary,
        onPressed: status.state == SandboxState.stopping
            ? null
            : () => _stop(ref),
        child: Text(l10n.sandboxStop),
      );
    }
    final HButton start = HButton(
      key: const Key('sandbox-start'),
      onPressed: status.isBusy || blocking != null
          ? null
          : () => unawaited(ref.read(sandboxStatusProvider.notifier).start()),
      child: Text(l10n.sandboxStart),
    );
    if (blocking == null) {
      return start;
    }
    // A control that is off says why on itself, not only in the card below
    // it (`docs/UX.md` 5.3).
    return HoverLabel(
      label: l10n.sandboxStartBlocked(
        blocking.title.isEmpty ? blocking.code : blocking.title,
      ),
      child: start,
    );
  }

  /// A sandbox whose agent has already finished has nothing left to
  /// interrupt, so it is stopped without a question. Asking anyway would
  /// teach the person to click the question away (`docs/UX.md` 5.4).
  void _stop(WidgetRef ref) {
    if (status.agentRunning) {
      onAskStop();
      return;
    }
    unawaited(ref.read(sandboxStatusProvider.notifier).stop());
  }
}
