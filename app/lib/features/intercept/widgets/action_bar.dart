/// The action bar: the place of every decision (HUM-028, HUM-029, HUM-072).
///
/// The release valve on the left, the block control on the right, and between
/// them the grid that turns one decision into a rule. Allow and Block are
/// never adjacent (BACKLOG.md 5), the valve is the one filled control of the
/// screen (`docs/UX.md` 3.1), and both decisions are larger than everything
/// else around them ([HSize.hitDecision], `docs/UX.md` 5.4).
///
/// Two lines under the controls are always reserved, so that nothing on this
/// screen moves when they fill: the first says why the request waits or which
/// rule is about to be created, the second says what just happened -- a rule
/// with its undo, or a refused input with its reason.
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/gestures.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/announce.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../finding_text.dart';
import '../format.dart';
import '../providers/decision.dart';
import '../providers/note.dart';
import '../providers/now.dart';
import '../providers/selection.dart';
import '../rule_sentence.dart';
import 'block_button.dart';
import 'note_field.dart';
import 'release_valve.dart';
import 'remember_grid.dart';

/// The gap the layout keeps between the two decisions.
///
/// A hurried click that misses Block must not land on Allow: Allow cannot be
/// taken back, Block can be retried by the agent (`docs/UX.md` 5.4). Below
/// [actionBarWrapWidth] the two decisions stand on two lines instead, which is
/// the same rule with a different geometry.
const double decisionGap = 160;

/// Below this width the bar wraps into a column.
const double actionBarWrapWidth = 640;

/// Height reserved for each of the two lines under the controls.
const double actionBarLineHeight = 24;

/// The action bar.
class ActionBar extends ConsumerStatefulWidget {
  /// Creates the bar for [flow]; a null flow disables every control.
  const ActionBar({required this.flow, super.key});

  /// The selected flow, or null while nothing is selected.
  final Flow? flow;

  @override
  ConsumerState<ActionBar> createState() => _ActionBarState();
}

class _ActionBarState extends ConsumerState<ActionBar> {
  bool _blockHighlighted = false;
  bool _noteSlotFocused = false;

  /// Every decision goes through the notifier, from the pointer as from the
  /// keyboard: the arming, the refusals, the rule and the reach are decided in
  /// one place (`providers/decision.dart`). One request takes the single path
  /// of HUM-028; a selection of several takes the batch of HUM-029, and above
  /// [modalAboveReach] the notifier asks first.
  void _allow({
    required bool remember,
    required List<Flow> flows,
    bool confirmed = false,
  }) => unawaited(
    ref
        .read(interceptDecisionProvider.notifier)
        .allowMany(flows, remember: remember, confirmed: confirmed),
  );

  void _block(List<Flow> flows) =>
      unawaited(ref.read(interceptDecisionProvider.notifier).blockMany(flows));

  /// What a hold on the two decisions belongs to.
  ///
  /// The whole selection, not one flow: a hold that started on twelve requests
  /// must not finish on eleven (`docs/UX.md` 5.4).
  String _reachToken(List<Flow> flows) =>
      <String>[for (final Flow flow in flows) flow.id.value].join(',');

  /// The pointer or the focus is on the block half of the bar: the sentence
  /// above shows the rule Block would create, and the note slot uncovers
  /// itself (`docs/UX.md` 3.4).
  void _highlightBlock(bool value) {
    if (_blockHighlighted != value) {
      setState(() => _blockHighlighted = value);
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow? flow = widget.flow;
    final DecisionProgress progress = ref.watch(interceptDecisionProvider);
    final RememberState remember = ref.watch(rememberDraftProvider);
    final Refusal? refusal = ref.watch(lastRefusalProvider);
    final DateTime now = ref.watch(nowProvider);
    final NoteDraft note = ref.watch(blockNoteProvider);
    final FindingSet findings = ref.watch(selectedFindingsProvider);
    // The registrable domain is a scope only while the daemon has said what
    // it is (`backlog/CONVENTIONS.md` 4.13).
    final bool apexKnown = ref.watch(selectedApexProvider).isNotEmpty;
    // What the next decision covers: the members of the selection, or the
    // cursor alone (`docs/UX.md` 3.5).
    final List<Flow> chosen = ref.watch(selectedFlowsProvider).flows;
    final int reach = chosen.length;
    final bool enabled = flow != null && flow.isHeld;
    // The control that acted shows the same 120 ms fill as a mouse click,
    // whether a key or the pointer took the decision (`docs/UX.md` 5.3), and
    // keeps it while the answer is on its way (2.5). Nothing greys out: a
    // second press is refused with a reason, not with a dead control.
    final DecisionKind? acting = progress is DecisionSending
        ? progress.kind
        : null;

    // Listened to, not watched: the bar has to keep the arming alive -- it
    // starts its clock the moment a selection appears -- and it must not
    // rebuild when the arming falls due, because nothing on it changes then.
    // The control stays as it is and the refusal explains itself
    // (`docs/UX.md` 5.3, 5.4).
    ref.listen<bool>(allowArmedProvider, (bool? previous, bool next) {});

    _announce(l10n);

    final String countdown = flow == null
        ? ''
        : l10n.interceptRemaining(formatCountdown(flow.remainingAt(now)));
    final RememberDuration holdDuration = remember.open
        ? remember.effective
        : RememberDuration.session;

    // At a selection larger than one the valve relabels itself and stays the
    // one filled control of the screen; a second batch button beside it would
    // be the second (`docs/UX.md` 3.5).
    // A request with an unresolved finding must not look like a routine one:
    // amber, its own label, and the same hold that blocking asks for
    // (`docs/UX.md` 4.7). Over a group the row counts stand in for the details
    // nobody fetched.
    final int findingReach = reach > 1
        ? chosen.fold(0, (int sum, Flow flow) => sum + flow.findingCount)
        : findings.count;
    final bool anyFinding = findingReach > 0;
    final String sendLabel = anyFinding
        ? l10n.interceptSendWithFindings(findingReach)
        : reach > 1
        ? l10n.interceptAllowSelected(reach)
        : allowLabel(remember.effective, l10n);
    final Widget valve = ReleaseValve(
      key: const Key('intercept-allow'),
      label: sendLabel,
      holdLabel: anyFinding
          ? sendLabel
          : reach > 1
          ? l10n.interceptAllowSelected(reach)
          : allowLabel(holdDuration, l10n),
      accent: anyFinding ? tokens.state.held : null,
      holdRequired: anyFinding,
      holdToken: _reachToken(chosen),
      onShortPress: () => ref
          .read(lastRefusalProvider.notifier)
          .refuse(RefusalReason.holdToSend),
      shortcutHint: l10n.interceptKeyAllow,
      semanticsValue: countdown,
      optionsLabel: l10n.interceptAllowOptions,
      enabled: enabled,
      optionsOpen: remember.open,
      pressed:
          acting == DecisionKind.allow || acting == DecisionKind.allowEdited,
      refusals: refusal?.serial ?? 0,
      // A click sends; while the hold is required, the hold sends.
      onAllow: () => _allow(
        remember: remember.remembers,
        flows: chosen,
        confirmed: anyFinding,
      ),
      onAllowRemembered: () => _allow(remember: true, flows: chosen),
      onToggleOptions: () => ref.read(rememberDraftProvider.notifier).toggle(),
    );

    final Widget block = BlockButton(
      key: const Key('intercept-block'),
      // The label says what pressing it does right now: how many requests it
      // covers, or that a note travels with it (HUM-072).
      label: reach > 1
          ? (note.sanitized.isEmpty
                ? l10n.interceptBlockSelected(reach)
                : l10n.interceptBlockSelectedWithNote(reach))
          : note.sanitized.isEmpty
          ? l10n.interceptBlockButton
          : l10n.interceptBlockWithNote,
      shortcutHint: l10n.interceptKeyBlock,
      semanticsValue: countdown,
      enabled: enabled,
      pressed: acting == DecisionKind.block,
      refusals: refusal?.serial ?? 0,
      holdToken: _reachToken(chosen),
      onBlock: () => _block(chosen),
      onShortPress: () =>
          ref.read(lastRefusalProvider.notifier).refuse(RefusalReason.holdIt),
      onHighlight: _highlightBlock,
    );

    // The way to the note for a pointer: a slot beside Block that is always
    // reserved and empty at rest, so that showing it moves nothing
    // (`docs/UX.md` 3.4). Hover and focus uncover it, `N` does the same thing
    // from the keyboard (5.1).
    final Widget noteSlot = Focus(
      canRequestFocus: false,
      skipTraversal: true,
      onFocusChange: (bool value) => setState(() => _noteSlotFocused = value),
      child: Visibility(
        visible: _blockHighlighted || _noteSlotFocused || note.open,
        maintainSize: true,
        maintainAnimation: true,
        maintainState: true,
        maintainSemantics: true,
        child: HButton(
          key: const Key('intercept-note-open'),
          variant: HButtonVariant.ghost,
          semanticsLabel: l10n.interceptNoteOpen,
          onPressed: enabled
              ? () => ref.read(blockNoteProvider.notifier).open()
              : null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(l10n.interceptNoteAdd),
              SizedBox(width: tokens.spacing.x2),
              Text(
                l10n.interceptKeyNote,
                style: tokens.typography.mono11.tinted(tokens.colors.fg1),
              ),
            ],
          ),
        ),
      ),
    );

    // Hover over the pair reveals the note slot, and it is read here and not
    // in the block control: a `MouseRegion` reports an enter in every
    // environment, while the hover highlight of a `FocusableActionDetector`
    // needs a focus highlight mode that a widget test never reaches.
    final Widget decisions = MouseRegion(
      onEnter: (PointerEnterEvent _) => _highlightBlock(true),
      onExit: (PointerExitEvent _) => _highlightBlock(false),
      // A `Wrap`, not a `Row`: at twice the text scale "Block 4 selected" and
      // the note beside it do not fit on one line, and a bar that overflows
      // hides a decision instead of moving it (`docs/UX.md` 6).
      child: Wrap(
        alignment: WrapAlignment.end,
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: tokens.spacing.x2,
        runSpacing: tokens.spacing.x2,
        children: <Widget>[noteSlot, block],
      ),
    );

    final Widget? grid = remember.open
        ? RememberGrid(
            key: const Key('intercept-remember'),
            heading: l10n.interceptRemember,
            durationLabel: l10n.interceptRememberDuration,
            targetLabel: l10n.interceptRememberTarget,
            duration: remember.duration,
            target: remember.target,
            durationLabels: <String>[
              for (final RememberDuration d in RememberDuration.values)
                durationLabel(d, l10n),
            ],
            targetLabels: <String>[
              for (final RememberTarget t in RememberTarget.values)
                targetLabel(t, l10n),
            ],
            enabled: enabled,
            disabledTargets: <RememberTarget>{
              if (!apexKnown) RememberTarget.apex,
            },
            onDuration: ref.read(rememberDraftProvider.notifier).setDuration,
            onTarget: ref.read(rememberDraftProvider.notifier).setTarget,
          )
        : null;

    return DecoratedBox(
      decoration: BoxDecoration(
        color: tokens.colors.bg1,
        border: Border(top: BorderSide(color: tokens.colors.line)),
      ),
      child: Padding(
        padding: EdgeInsets.all(tokens.spacing.x3),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            LayoutBuilder(
              builder: (BuildContext context, BoxConstraints constraints) {
                // The threshold grows with the text: at twice the scale the
                // same three controls need twice the room, and the bar breaks
                // into a column instead of hiding one (`docs/UX.md` 6).
                final double scale = MediaQuery.textScalerOf(context).scale(1);
                // An open grid always takes a line of its own: eight segments
                // and their two frames measure around 800 px, which no pane of
                // this window ever has left beside the two decisions.
                return grid == null &&
                        constraints.maxWidth >= actionBarWrapWidth * scale
                    ? _wide(tokens, valve, decisions)
                    : _narrow(tokens, valve, grid, decisions);
              },
            ),
            // The note is temporary: hidden until `N`, gone on `Escape` and on
            // every decision, never a permanent row of the bar
            // (`docs/UX.md` 5.4).
            AnimatedSize(
              duration: HReducedMotion.displace(context, HMotion.arrive),
              curve: HMotion.enter,
              alignment: Alignment.topCenter,
              child: note.open
                  ? Padding(
                      padding: EdgeInsets.only(top: tokens.spacing.x3),
                      child: const NoteField(),
                    )
                  : const SizedBox(width: double.infinity),
            ),
            SizedBox(height: tokens.spacing.x2),
            // What sending would cost, while a finding is unresolved: the
            // secret and its destination, in one sentence (`docs/UX.md` 4.7).
            if (findings.isNotEmpty && flow != null)
              _FindingLine(flow: flow, findings: findings),
            _FirstLine(
              flow: flow,
              remember: remember,
              blockHighlighted: _blockHighlighted,
              findings: findings,
            ),
            _SecondLine(refusal: refusal),
            if (progress is DecisionFailed) ...<Widget>[
              SizedBox(height: tokens.spacing.x3),
              Align(
                alignment: Alignment.centerLeft,
                child: HDiagnosticCard(
                  key: const Key('intercept-decision-error'),
                  code: progress.diagnostic.code,
                  severityLabel: severityLabel(
                    l10n,
                    progress.diagnostic.severity,
                  ),
                  color: severityColor(tokens, progress.diagnostic.severity),
                  title: l10n.interceptDecisionFailedTitle,
                  // The `why` slot carries the daemon's sentence; the generic
                  // text of the app is the title, not the cause
                  // (`docs/UX.md` 4.4).
                  why: progress.diagnostic.why,
                  fix: FixControl(fix: progress.diagnostic.fix),
                  docsUrl: progress.diagnostic.docsUrl,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  /// The wide layout: both decisions on one line, [decisionGap] apart.
  ///
  /// The note slot stands inside that distance, on the Block side: it is the
  /// note of a block and belongs beside it, and the gap between the two
  /// decisions stays what it is.
  ///
  /// "Edit + Allow" is not here. It was a control that could not be pressed,
  /// and a dead state without a reason is worse than a missing one
  /// (`backlog/CONVENTIONS.md` 4.13); the request editor of HUM-047 brings it
  /// back with something behind it. Until then the body stands read-only in
  /// the card above.
  Widget _wide(HTokens tokens, Widget valve, Widget decisions) => Row(
    children: <Widget>[
      valve,
      const Spacer(),
      const SizedBox(width: decisionGap),
      decisions,
    ],
  );

  /// The narrow layout: Block moves to its own line, right aligned.
  ///
  /// What goes first when the pane gets narrow stands here and is not left to
  /// the layout: the edit control, which is the only one of the three that
  /// takes no decision. The two decisions keep their full size, and the
  /// distance between them becomes a line break instead of 160 px.
  Widget _narrow(
    HTokens tokens,
    Widget valve,
    Widget? grid,
    Widget decisions,
  ) => Column(
    crossAxisAlignment: CrossAxisAlignment.stretch,
    mainAxisSize: MainAxisSize.min,
    children: <Widget>[
      Align(alignment: Alignment.centerLeft, child: valve),
      if (grid != null) ...<Widget>[
        SizedBox(height: tokens.spacing.x2),
        Align(alignment: Alignment.centerLeft, child: grid),
      ],
      SizedBox(height: tokens.spacing.x3),
      Align(alignment: Alignment.centerRight, child: decisions),
    ],
  );

  /// Says what happened, once, and politely.
  ///
  /// A decision the person took themselves is announced politely with host and
  /// size; a refused key gets its reason. Nothing is announced assertively
  /// here: what the person did not cause -- a timeout -- is the only thing
  /// that interrupts (`docs/UX.md` 6).
  void _announce(AppLocalizations l10n) {
    ref.listen<DecisionOutcome>(lastDecisionProvider, (
      DecisionOutcome? previous,
      DecisionOutcome next,
    ) {
      if (next is! DecisionOutcomeDone) {
        return;
      }
      final String where = next.hostCount > 1
          ? l10n.interceptSeveralHosts(next.hostCount)
          : next.host;
      final String what = switch (next.kind) {
        DecisionKind.allow || DecisionKind.allowEdited =>
          next.count > 1
              ? l10n.interceptSentMany(next.count, where)
              : l10n.interceptSentTo(next.host, formatBytes(next.size)),
        DecisionKind.block =>
          next.count > 1
              ? l10n.interceptBlockedMany(next.count)
              : l10n.interceptBlockedRetry,
        DecisionKind.timedOut => l10n.interceptBlockedTimedOut,
      };
      final String rule = next.rule == null
          ? ''
          : ' ${l10n.interceptRuleSaved}';
      announcePolitely(context, '$what$rule');
    });
    ref.listen<Refusal?>(lastRefusalProvider, (
      Refusal? previous,
      Refusal? next,
    ) {
      if (next == null) {
        return;
      }
      announcePolitely(context, refusalText(next.reason, l10n));
    });
  }
}

/// The label of [severity] in the person's language.
String severityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };

/// The hue of [severity]. Never the blocked red: red means blocked.
Color severityColor(HTokens tokens, Severity severity) => switch (severity) {
  Severity.info => tokens.colors.accent,
  Severity.warning => tokens.state.held,
  Severity.error || Severity.blocking => tokens.state.error,
};

/// Why an input did nothing, in the person's language.
String refusalText(RefusalReason reason, AppLocalizations l10n) =>
    switch (reason) {
      RefusalReason.noFlow => l10n.interceptRefusedNoFlow,
      RefusalReason.notArmed => l10n.interceptRefusedNotArmed,
      RefusalReason.keyHeld => l10n.interceptRefusedKeyHeld,
      RefusalReason.sending => l10n.interceptRefusedSending,
      RefusalReason.holdIt => l10n.interceptHoldToBlock,
      RefusalReason.holdToSend => l10n.interceptHoldToSend,
      RefusalReason.apexUnknown => l10n.interceptRefusedApexUnknown,
    };

/// What sending this request would give away, and to whom.
///
/// Only there while a finding is unresolved: whoever knows what goes where
/// holds the control down; whoever does not, does not (`docs/UX.md` 4.7).
class _FindingLine extends StatelessWidget {
  const _FindingLine({required this.flow, required this.findings});

  final Flow flow;
  final FindingSet findings;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Finding? first = findings.first;
    final String what = first == null
        ? l10n.findingSecretCount(findings.count)
        : findingName(first, l10n);
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: actionBarLineHeight),
      child: Align(
        alignment: Alignment.centerLeft,
        child: Text(
          l10n.interceptFindingGoesTo(what, flow.host),
          key: const Key('intercept-finding-consequence'),
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.ui12.medium.tinted(tokens.colors.fg0),
        ),
      ),
    );
  }
}

/// The first reserved line: the rule that is about to be created, the warning
/// that the budget runs out, or why this request waits.
class _FirstLine extends ConsumerWidget {
  const _FirstLine({
    required this.flow,
    required this.remember,
    required this.blockHighlighted,
    required this.findings,
  });

  final Flow? flow;
  final RememberState remember;
  final bool blockHighlighted;
  final FindingSet findings;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final Flow? flow = this.flow;
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: actionBarLineHeight),
      child: Align(
        alignment: Alignment.centerLeft,
        child: flow == null
            ? const SizedBox.shrink()
            : _content(context, ref, tokens, l10n, flow),
      ),
    );
  }

  Widget _content(
    BuildContext context,
    WidgetRef ref,
    HTokens tokens,
    AppLocalizations l10n,
    Flow flow,
  ) {
    if (remember.remembers) {
      // Whoever reaches for Block reads the rule Block would create.
      final RuleAction action = blockHighlighted
          ? RuleAction.block
          : RuleAction.allow;
      // The request the rule is generalised from, not the cursor: with a
      // group of several hosts those are two different requests, and the
      // sentence has to name the one that ends up in the rule (major 2).
      final Flow source = ref.watch(ruleFlowProvider) ?? flow;
      return Text(
        ruleSentence(
          RuleDraft(
            duration: remember.effective,
            target: remember.target,
            flow: source,
            action: action,
          ),
          l10n,
          apexOf: ref.watch(apexResolverProvider),
        ),
        key: const Key('intercept-rule-sentence'),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: tokens.typography.mono12.tinted(tokens.colors.fg1),
      );
    }
    final DateTime now = ref.watch(nowProvider);
    final Duration left = flow.remainingAt(now);
    final Duration budget = flow.holdBudget;
    final bool nearTheEnd =
        flow.isHeld &&
        budget > Duration.zero &&
        left.inMilliseconds <= budget.inMilliseconds * HMotion.breatheBelow;
    if (nearTheEnd) {
      // The consequence is named, never a bare mm:ss (`docs/UX.md` 4.3, 4.8).
      return Text(
        l10n.interceptAutoBlocksIn(flow.host, formatCountdown(left)),
        key: const Key('intercept-timeout-warning'),
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: tokens.typography.ui12.medium.tinted(tokens.colors.fg0),
      );
    }
    return Text(
      _holdReason(flow, l10n),
      key: const Key('intercept-hold-reason'),
      maxLines: 1,
      overflow: TextOverflow.ellipsis,
      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
    );
  }

  /// Why this request waits, built from the flow and never from a constant
  /// (`docs/UX.md` 4.3).
  ///
  /// A finding names its kind and its place in plain words as soon as the
  /// daemon has described it; the identifier of the kind never reaches the
  /// screen (4.2). Until the description arrives, the number stands alone.
  String _holdReason(Flow flow, AppLocalizations l10n) {
    final RuleId? rule = flow.ruleId;
    if (rule != null) {
      return l10n.interceptHoldReasonRule(rule.value);
    }
    final Finding? finding = findings.first;
    if (finding != null) {
      return l10n.interceptHoldReasonFinding(
        findingName(finding, l10n),
        findingWhere(finding, l10n),
      );
    }
    if (findings.isNotEmpty) {
      return l10n.interceptHoldReasonFindings(findings.count);
    }
    return l10n.interceptHoldReason;
  }
}

/// The second reserved line: what just happened.
class _SecondLine extends ConsumerWidget {
  const _SecondLine({required this.refusal});

  final Refusal? refusal;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final DecisionOutcome outcome = ref.watch(lastDecisionProvider);
    final Refusal? refusal = this.refusal;
    return ConstrainedBox(
      constraints: const BoxConstraints(minHeight: actionBarLineHeight),
      child: Align(
        alignment: Alignment.centerLeft,
        child: refusal != null
            ? Text(
                refusalText(refusal.reason, l10n),
                key: const Key('intercept-refusal'),
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: tokens.typography.ui12.tinted(tokens.colors.fg1),
              )
            : _outcome(context, ref, tokens, l10n, outcome),
      ),
    );
  }

  Widget _outcome(
    BuildContext context,
    WidgetRef ref,
    HTokens tokens,
    AppLocalizations l10n,
    DecisionOutcome outcome,
  ) => switch (outcome) {
    DecisionOutcomeDone(:final Rule? rule, :final DecisionKind kind)
        when rule != null =>
      Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Flexible(
            child: Text(
              kind == DecisionKind.block
                  ? l10n.interceptRuleSavedBlock
                  : l10n.interceptRuleSaved,
              key: const Key('intercept-rule-saved'),
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: tokens.typography.ui12.tinted(tokens.colors.fg1),
            ),
          ),
          SizedBox(width: tokens.spacing.x2),
          HButton(
            key: const Key('intercept-undo'),
            variant: HButtonVariant.ghost,
            onPressed: () =>
                unawaited(ref.read(lastDecisionProvider.notifier).undo()),
            child: Text(l10n.interceptUndo),
          ),
        ],
      ),
    DecisionOutcomeUndone() => Text(
      l10n.interceptUndoDone,
      key: const Key('intercept-undo-done'),
      style: tokens.typography.ui12.tinted(tokens.colors.fg1),
    ),
    DecisionOutcomeUndoFailed(:final Diagnostic diagnostic) => HDiagnosticCard(
      key: const Key('intercept-undo-error'),
      code: diagnostic.code,
      severityLabel: severityLabel(l10n, diagnostic.severity),
      color: severityColor(tokens, diagnostic.severity),
      title: l10n.interceptUndoFailedTitle,
      why: diagnostic.why,
      fix: FixControl(fix: diagnostic.fix),
      docsUrl: diagnostic.docsUrl,
    ),
    _ => const SizedBox.shrink(),
  };
}
