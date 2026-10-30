/// Das Panel der Änderungen einer Wegwerf-Sandbox (HUM-351).
///
/// Nach dem Ende eines Wegwerf-Laufs ist das der Ort, an dem ein Mensch
/// sieht, was der Agent geändert hat, und entscheidet, was davon in den
/// Projektordner kommt. Die Liste, die Diffs, die Konflikte und die
/// Ablehnungen kommen aus dem Daemon; das Panel zeigt sie, hält die Auswahl
/// und schickt die gewählten Pfad-Hashes zurück (ADR-018).
///
/// Drei Wege hinaus, jeder mit Rückfrage: „Auswahl übernehmen", „Alle
/// übernehmen" (nur, wenn jede Zeile übernommen werden darf) und „Alle
/// verwerfen". Eine Datei mit Konflikt kommt nur über eine eigene
/// Bestätigung in die Auswahl; ein globaler Schalter zum Überschreiben
/// existiert nicht (siehe `disposable_review.dart`).
library;

import 'dart:math' as math;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/diagnostic_severity.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/disposable_review.dart';
import 'arrive.dart';
import 'change_row.dart';
import 'changes_confirm.dart';

/// Die größte Breite des Panels: genug für einen Diff mit 100 Spalten neben
/// Kästchen und Marken. Schmaler, wenn das Fenster schmaler ist.
const double changesPanelMaxWidth = 960;

/// Welche Rückfrage gerade steht.
sealed class _Ask {
  const _Ask();
}

/// Die Rückfrage hält, was sie zeigt: Geschickt wird genau diese Auswahl,
/// und hat sie sich bis zur Bestätigung geändert, fragt das Panel neu.
final class _AskApplySelected extends _Ask {
  const _AskApplySelected(this.chosen);

  final ChosenApply chosen;
}

final class _AskApplyAll extends _Ask {
  const _AskApplyAll(this.all);

  final ChosenApply all;
}

/// Hält den Lauf und jede Kennung, die die Rückfrage gezeigt hat.
final class _AskDiscard extends _Ask {
  const _AskDiscard(this.all);

  final ChosenApply all;
}

/// Hält die Zeile samt Satz des Daemons und den Lauf, zu dem sie gehört.
final class _AskOverwrite extends _Ask {
  const _AskOverwrite(this.change, this.sandboxId);

  final DisposableChange change;
  final String sandboxId;
}

/// Das Panel, rechts über der Sandbox-Seite, samt seinen Rückfragen.
class ChangesPanel extends ConsumerStatefulWidget {
  /// Legt das Panel an; es liest alles aus `disposableReviewProvider`.
  const ChangesPanel({super.key});

  @override
  ConsumerState<ChangesPanel> createState() => _ChangesPanelState();
}

class _ChangesPanelState extends ConsumerState<ChangesPanel> {
  _Ask? _ask;

  DisposableReviewNotifier get _review =>
      ref.read(disposableReviewProvider.notifier);

  void _cancel() => setState(() => _ask = null);

  void _confirm() {
    final _Ask? ask = _ask;
    setState(() => _ask = null);
    switch (ask) {
      case _AskApplySelected(:final ChosenApply chosen):
        if (!_review.applyChosen(chosen)) {
          // Die Auswahl hat sich geändert, seit die Frage aufging: neu fragen,
          // mit dem, was jetzt geschickt würde.
          final ChosenApply now = ref.read(disposableReviewProvider).chosen;
          if (now.ids.isNotEmpty) {
            setState(() => _ask = _AskApplySelected(now));
          }
        }
      case _AskApplyAll(:final ChosenApply all):
        if (!_review.applyAll(all)) {
          // Ein anderer Lauf oder andere Zeilen als gezeigt: neu fragen.
          final DisposableReview now = ref.read(disposableReviewProvider);
          if (now.canApplyAll) {
            setState(() => _ask = _AskApplyAll(now.all));
          }
        }
      case _AskDiscard(:final ChosenApply all):
        if (!_review.discard(all)) {
          // Ein anderer Lauf oder andere Zeilen als gezeigt: neu fragen.
          final DisposableReview now = ref.read(disposableReviewProvider);
          if (now.waiting) {
            setState(() => _ask = _AskDiscard(now.all));
          }
        }
      case _AskOverwrite(
        :final DisposableChange change,
        :final String sandboxId,
      ):
        if (!_review.confirmOverwrite(
          change.id,
          sawConflict: change.conflict,
          sandboxId: sandboxId,
        )) {
          // Der Konflikt ist ein anderer geworden oder weg, oder ein anderer
          // Lauf steht da: neu fragen, wenn es noch einen gibt.
          final DisposableReview now = ref.read(disposableReviewProvider);
          for (final DisposableChange row in now.rows) {
            if (row.id == change.id && row.hasConflict && !row.isRefused) {
              setState(() => _ask = _AskOverwrite(row, now.changes!.sandboxId));
            }
          }
        }
      case null:
        break;
    }
  }

  /// Ein Kästchen wurde angefasst. Eine Datei mit Konflikt fragt vor dem
  /// Anhaken; alles andere geht ohne Frage.
  void _toggle(DisposableChange change, bool on) {
    if (on && change.hasConflict && !ref.read(perChangeOverwriteProvider)) {
      return;
    }
    if (on && change.hasConflict) {
      final String sandboxId =
          ref.read(disposableReviewProvider).changes?.sandboxId ?? '';
      setState(() => _ask = _AskOverwrite(change, sandboxId));
      return;
    }
    _review.toggle(change.id, on: on);
  }

  @override
  Widget build(BuildContext context) {
    final DisposableReview review = ref.watch(disposableReviewProvider);
    final AppLocalizations l10n = context.l10n;
    final _Ask? ask = _ask;
    return Stack(
      fit: StackFit.expand,
      children: <Widget>[
        LayoutBuilder(
          builder: (BuildContext context, BoxConstraints constraints) => Align(
            alignment: Alignment.centerRight,
            child: SandboxArrive(
              from: ArriveFrom.right,
              child: FocusScope(
                autofocus: true,
                child: HSheet(
                  key: const Key('changes-panel'),
                  width: math.min(constraints.maxWidth, changesPanelMaxWidth),
                  title: Text(l10n.sandboxChangesTitle),
                  closeSemanticsLabel: l10n.sandboxChangesClose,
                  onClose: _review.hide,
                  child: _Body(
                    review: review,
                    perChange: ref.watch(perChangeOverwriteProvider),
                    onToggle: _toggle,
                    onAsk: (_Ask ask) => setState(() => _ask = ask),
                  ),
                ),
              ),
            ),
          ),
        ),
        if (ask != null) _confirmFor(context, ask, review),
      ],
    );
  }

  Widget _confirmFor(BuildContext context, _Ask ask, DisposableReview review) {
    final AppLocalizations l10n = context.l10n;
    final String folder = review.changes?.workDir ?? '';
    return switch (ask) {
      _AskApplySelected(:final ChosenApply chosen) => ChangesConfirm(
        title: l10n.sandboxChangesApplyTitle(chosen.ids.length),
        body: <String>[
          l10n.sandboxChangesApplyBody(folder),
          if (chosen.overwriteIds.isNotEmpty)
            l10n.sandboxChangesApplyOverwrite(chosen.overwriteIds.length),
        ],
        confirmLabel: l10n.sandboxChangesApplyConfirm,
        confirmKey: const Key('changes-confirm-apply'),
        onCancel: _cancel,
        onConfirm: _confirm,
      ),
      _AskApplyAll(:final ChosenApply all) => ChangesConfirm(
        title: l10n.sandboxChangesApplyTitle(all.ids.length),
        body: <String>[l10n.sandboxChangesApplyBody(folder)],
        confirmLabel: l10n.sandboxChangesApplyConfirm,
        confirmKey: const Key('changes-confirm-apply'),
        onCancel: _cancel,
        onConfirm: _confirm,
      ),
      _AskDiscard(:final ChosenApply all) => ChangesConfirm(
        title: l10n.sandboxChangesDiscardTitle,
        body: <String>[l10n.sandboxChangesDiscardBody(all.ids.length, folder)],
        confirmLabel: l10n.sandboxChangesDiscardConfirm,
        confirmKey: const Key('changes-confirm-discard'),
        danger: true,
        onCancel: _cancel,
        onConfirm: _confirm,
      ),
      _AskOverwrite(:final DisposableChange change) => ChangesConfirm(
        title: l10n.sandboxChangesOverwriteTitle(change.path),
        body: <String>[
          l10n.sandboxChangesConflictWhy(change.conflict),
          l10n.sandboxChangesOverwriteBody,
        ],
        confirmLabel: l10n.sandboxChangesOverwriteConfirm,
        confirmKey: const Key('changes-confirm-overwrite'),
        danger: true,
        onCancel: _cancel,
        onConfirm: _confirm,
      ),
    };
  }
}

/// Ergebnis, Befund und Liste untereinander.
class _Body extends StatelessWidget {
  const _Body({
    required this.review,
    required this.perChange,
    required this.onToggle,
    required this.onAsk,
  });

  final DisposableReview review;

  /// Ob der Daemon Überschreiben je Datei kennt (Minor 26).
  final bool perChange;
  final void Function(DisposableChange change, bool on) onToggle;
  final ValueChanged<_Ask> onAsk;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final DisposableOutcome? outcome = review.outcome;
    final Diagnostic? notice = review.notice;
    final DisposableChanges? changes = review.changes;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (outcome != null) ...<Widget>[
          _OutcomeLine(outcome: outcome),
          SizedBox(height: tokens.spacing.x3),
        ],
        if (notice != null) ...<Widget>[
          _Notice(diagnostic: notice),
          SizedBox(height: tokens.spacing.x3),
        ],
        if (changes != null)
          Expanded(
            child: _List(
              review: review,
              perChange: perChange,
              changes: changes,
              onToggle: onToggle,
              onAsk: onAsk,
            ),
          )
        else
          const _Closing(),
      ],
    );
  }
}

/// Die Liste mit Kopf und Knöpfen.
class _List extends StatelessWidget {
  const _List({
    required this.review,
    required this.perChange,
    required this.changes,
    required this.onToggle,
    required this.onAsk,
  });

  final DisposableReview review;

  /// Ob der Daemon Überschreiben je Datei kennt (Minor 26).
  final bool perChange;
  final DisposableChanges changes;
  final void Function(DisposableChange change, bool on) onToggle;
  final ValueChanged<_Ask> onAsk;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final int chosen = review.chosenIds.length;
    final bool busy = review.busy;
    final HButton applyAll = HButton(
      key: const Key('changes-apply-all'),
      variant: HButtonVariant.secondary,
      onPressed: busy || !review.canApplyAll
          ? null
          : () => onAsk(_AskApplyAll(review.all)),
      child: Text(l10n.sandboxChangesApplyAll),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          l10n.sandboxChangesFolder(changes.workDir),
          key: const Key('changes-folder'),
          style: tokens.typography.mono12.tinted(tokens.colors.fg1),
          overflow: TextOverflow.ellipsis,
        ),
        SizedBox(height: tokens.spacing.x1),
        Text(
          l10n.sandboxChangesSummary(changes.changes.length, chosen),
          key: const Key('changes-summary'),
          style: tokens.typography.ui12.tinted(tokens.colors.fg1),
        ),
        if (changes.truncated) ...<Widget>[
          SizedBox(height: tokens.spacing.x1),
          Text(
            l10n.sandboxChangesTruncated,
            key: const Key('changes-truncated'),
            style: tokens.typography.ui12.tinted(tokens.colors.warningText),
          ),
        ],
        SizedBox(height: tokens.spacing.x2),
        const HHairline(),
        Expanded(
          child: ListView.separated(
            key: const Key('changes-list'),
            itemCount: changes.changes.length,
            separatorBuilder: (BuildContext context, int index) =>
                const HHairline(),
            itemBuilder: (BuildContext context, int index) {
              final DisposableChange change = changes.changes[index];
              // Ein Daemon vor Minor 26 überschreibt keine einzelne Datei; die
              // App fällt nie auf den Schalter für alle zurück (HUM-351).
              final bool stuck = change.hasConflict && !perChange;
              return ChangeRow(
                change: change,
                selected: review.selected.contains(change.id),
                enabled: !change.isRefused && !busy && !stuck,
                overwriteUnavailable: stuck,
                onChanged: (bool on) => onToggle(change, on),
              );
            },
          ),
        ),
        const HHairline(),
        SizedBox(height: tokens.spacing.x3),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: tokens.spacing.x2,
          runSpacing: tokens.spacing.x2,
          children: <Widget>[
            HButton(
              key: const Key('changes-discard-all'),
              variant: HButtonVariant.ghost,
              onPressed: busy ? null : () => onAsk(_AskDiscard(review.all)),
              child: Text(l10n.sandboxChangesDiscardAll),
            ),
            if (review.canApplyAll)
              applyAll
            else
              // Ein Knopf, der aus ist, sagt auf sich selbst warum
              // (`docs/UX.md` 5.3).
              HoverLabel(
                label: l10n.sandboxChangesApplyAllOff,
                child: applyAll,
              ),
            HButton(
              key: const Key('changes-apply-selected'),
              onPressed: busy || chosen == 0
                  ? null
                  : () => onAsk(_AskApplySelected(review.chosen)),
              child: Text(l10n.sandboxChangesApplySelected(chosen)),
            ),
          ],
        ),
      ],
    );
  }
}

/// Die eine Zeile, die nach der Entscheidung an die Stelle der Liste tritt.
class _OutcomeLine extends StatelessWidget {
  const _OutcomeLine({required this.outcome});

  final DisposableOutcome outcome;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final (String text, Color color) = switch (outcome.outcome) {
      DisposableOutcomeKind.applied => (
        l10n.sandboxChangesOutcomeApplied(
          outcome.applied.length,
          outcome.discarded.length,
        ),
        tokens.stateText.allowed,
      ),
      DisposableOutcomeKind.discarded => (
        l10n.sandboxChangesOutcomeDiscarded(outcome.discarded.length),
        tokens.colors.fg0,
      ),
      DisposableOutcomeKind.partial => (
        l10n.sandboxChangesOutcomePartial(outcome.applied.length),
        tokens.colors.warningText,
      ),
      DisposableOutcomeKind.unknown => (
        l10n.sandboxChangesOutcomeUnknown,
        tokens.colors.fg1,
      ),
    };
    return Semantics(
      liveRegion: true,
      child: Text(
        text,
        key: const Key('changes-outcome'),
        style: tokens.typography.ui13.semibold.tinted(color),
      ),
    );
  }
}

/// Der Befund des Daemons zur letzten Entscheidung, mit seiner Abhilfe.
class _Notice extends StatelessWidget {
  const _Notice({required this.diagnostic});

  final Diagnostic diagnostic;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DiagnosticText text = DiagnosticL10n.resolve(diagnostic, l10n);
    return HDiagnosticCard(
      key: const Key('changes-notice'),
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: text.title,
      // Der Satz des Daemons; die App schreibt den Titel, nie den Grund
      // (`docs/UX.md` 4.4).
      why: text.cause,
      docsUrl: diagnostic.docsUrl,
      fix: FixControl(
        fix: diagnostic.fix,
        diagnosticCode: diagnostic.code,
        copyKey: const Key('changes-fix-copy'),
      ),
      width: double.infinity,
      dense: true,
    );
  }
}

/// Nichts wartet mehr: der Weg hinaus.
class _Closing extends ConsumerWidget {
  const _Closing();

  @override
  Widget build(BuildContext context, WidgetRef ref) => Align(
    alignment: Alignment.centerLeft,
    child: HButton(
      key: const Key('changes-done'),
      variant: HButtonVariant.secondary,
      onPressed: ref.read(disposableReviewProvider.notifier).hide,
      child: Text(context.l10n.sandboxChangesDone),
    ),
  );
}
