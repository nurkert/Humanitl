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
import '../../../core/ipc/connection.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../core/ui/work_dir_picker.dart';
import '../../../l10n/l10n.dart';
import '../providers/disposable_review.dart';
import '../providers/sandbox_status_provider.dart';
import '../sandbox_text.dart';

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
    final bool waiting = ref.watch(
      disposableReviewProvider.select(
        (DisposableReview review) => review.waiting,
      ),
    );
    // Die Kopfzeile des Screens, mit Höhe und Linie aus dem Token wie jede
    // andere (HUM-286).
    return ColoredBox(
      color: tokens.colors.bg1,
      child: HPaneHeader(
        key: const Key('sandbox-header'),
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
                child: WorkDirPicker(
                  workDir: status.workDirHost,
                  workMode: status.workMode,
                  chooseLabel: l10n.sandboxWorkDirChoose,
                  chooseButtonLabel: l10n.workDirChooseButton,
                  changeButtonLabel: l10n.workDirChangeButton,
                  noneLabel: l10n.sandboxWorkDirNone,
                  readOnlyLabel: l10n.sandboxWorkModeRo,
                  readWriteLabel: l10n.sandboxWorkModeRw,
                  // While the sandbox is up the mount cannot change; a picker
                  // that answered would show a folder the running agent does
                  // not have.
                  lockedLabel: status.isUp || status.isBusy
                      ? l10n.sandboxWorkDirLocked
                      : null,
                  onPick: (String dir) => unawaited(
                    ref.read(sandboxStatusProvider.notifier).chooseWorkDir(dir),
                  ),
                  onMode: (WorkMode mode) => unawaited(
                    ref
                        .read(sandboxStatusProvider.notifier)
                        .plan(workMode: mode),
                  ),
                ),
              ),
            ),
            SizedBox(width: tokens.spacing.x3),
            // Warten Änderungen eines Wegwerf-Laufs, stehen hier drei
            // Controls statt zwei (HUM-351). Bei großer Schrift passen sie
            // nicht mehr neben Zustand und Profil; dann rollen sie in sich,
            // statt über den Rand zu laufen, und der Fokus holt jedes in
            // Sicht.
            if (waiting)
              Flexible(
                child: SingleChildScrollView(
                  key: const Key('sandbox-header-trailing'),
                  scrollDirection: Axis.horizontal,
                  reverse: true,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      DisposableSwitch(status: status),
                      SandboxStartStop(status: status, onAskStop: onAskStop),
                    ],
                  ),
                ),
              )
            else ...<Widget>[
              DisposableSwitch(status: status),
              SandboxStartStop(status: status, onAskStop: onAskStop),
            ],
          ],
        ),
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
    // Solange die Änderungen eines Wegwerf-Laufs warten, lehnt der Daemon
    // nur einen weiteren Wegwerf-Start ab (`SANDBOX_030`); ein direkter Start
    // geht und lässt die Änderungen warten. Der Knopf sperrt deshalb nur im
    // Wegwerf-Modus und sagt es auf sich selbst; im direkten Modus sagt er,
    // dass die Änderungen weiter warten. Daneben steht in beiden Fällen der
    // Weg zum Panel (HUM-351).
    final DisposableReview review = ref.watch(disposableReviewProvider);
    final bool waiting = review.waiting;
    final bool refused =
        waiting && ref.watch(nextSandboxModeProvider) == SandboxMode.disposable;
    final HButton start = HButton(
      key: const Key('sandbox-start'),
      onPressed: status.isBusy || blocking != null || refused
          ? null
          : () => unawaited(ref.read(sandboxStatusProvider.notifier).start()),
      child: Text(l10n.sandboxStart),
    );
    if (waiting && blocking == null) {
      final HTokens tokens = HTheme.of(context);
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          HButton(
            key: const Key('sandbox-changes-open'),
            variant: HButtonVariant.secondary,
            onPressed: ref.read(disposableReviewProvider.notifier).show,
            child: Text(l10n.sandboxChangesReview(review.rows.length)),
          ),
          SizedBox(width: tokens.spacing.x2),
          HoverLabel(
            label: refused
                ? l10n.sandboxChangesStartBlocked
                : l10n.sandboxChangesStartDirect,
            child: start,
          ),
        ],
      );
    }
    if (blocking == null) {
      return start;
    }
    // A control that is off says why on itself, not only in the card below
    // it (`docs/UX.md` 5.3).
    return HoverLabel(
      label: l10n.sandboxStartBlocked(
        DiagnosticL10n.resolve(blocking, l10n).title,
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

/// Der Schalter „Wegwerf" vor dem Start (HUM-351).
///
/// Nur bei einem Daemon, der `sandbox.disposable` nennt; auf macOS nennt er
/// die Fähigkeit nicht, und der Schalter fehlt. Seine Vorgabe ist
/// `sandbox.mode` aus `GetConfig`; was er wählt, gilt für den nächsten Start
/// und geht als `Start.sandbox_mode` mit. Solange eine Sandbox steht, ändert
/// er nichts und ist aus.
class DisposableSwitch extends ConsumerWidget {
  /// Legt den Schalter für [status] an.
  const DisposableSwitch({required this.status, super.key});

  /// Was der Daemon zuletzt sagte.
  final SandboxStatus status;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final bool offered =
        ref.watch(daemonInfoProvider).value?.offersDisposable ?? false;
    if (!offered) {
      return const SizedBox.shrink();
    }
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final SandboxMode mode = ref.watch(nextSandboxModeProvider);
    return Padding(
      padding: EdgeInsets.only(right: tokens.spacing.x3),
      child: HoverLabel(
        label: l10n.sandboxDisposableHint,
        // Die Zeile des Kopfs gibt keine Breite vor; das Kästchen streckt
        // seine Beschriftung, also misst es sich an ihr.
        child: IntrinsicWidth(
          child: HCheckbox(
            key: const Key('sandbox-disposable'),
            label: l10n.sandboxDisposableLabel,
            value: mode == SandboxMode.disposable,
            enabled: !(status.isUp || status.isBusy),
            onChanged: (bool on) => ref
                .read(sandboxModeChoiceProvider.notifier)
                .choose(on ? SandboxMode.disposable : SandboxMode.direct),
          ),
        ),
      ),
    );
  }
}
