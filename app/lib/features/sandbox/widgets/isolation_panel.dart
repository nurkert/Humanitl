/// The three guarantees, each with the evidence it produced (HUM-041).
///
/// This is the place where the product shows its own central claim, so every
/// line on it is built to be doubted. A dot alone is decoration; a dot with
/// the line the shim wrote next to it is an argument, and that line names the
/// check, what it looked at and what came back. Nothing here is derived from
/// the state of the screen: the daemon measures inside the running sandbox
/// and the panel shows the answer (ADR-018).
///
/// Three states must never look alike, and the second and third are the ones
/// that matter:
///
/// - **passed** -- measured, and the guarantee holds.
/// - **failed** -- measured, and it does not. Red, with the daemon's own
///   finding under it, so that it says which guarantee stopped holding and
///   what to do, not "check failed".
/// - **not measured** -- no result arrived. Its own word, its own grey and a
///   hollow dot, and a sentence under the line saying it is not a pass.
///   Nothing measured is not the same as measured and good
///   (CONVENTIONS 4.13).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/fix_control.dart';
import '../../../core/ui/h_diagnostic_card.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../sandbox_text.dart';

/// Diameter of the dot in front of a guarantee.
const double isolationDotSize = 8;

/// How much of a line the sentence takes.
const int isolationSentenceFlex = 5;

/// How much of a line the evidence takes.
///
/// The sentence is the claim and the evidence is the proof; the proof gets
/// the wider half, because it is the longer text and the reason to be here.
const int isolationEvidenceFlex = 6;

/// How many lines of evidence one guarantee shows.
///
/// The daemon caps an evidence line at 500 characters; in the width this
/// column has, that is about six lines of monospace 11. Eight therefore never
/// cuts a real line and still bounds the box that a hostile file name from
/// `/work` could otherwise stretch.
const int isolationEvidenceMaxLines = 8;

/// The isolation tab.
class IsolationPanel extends StatelessWidget {
  /// Shows the guarantees of [status]. [onShowArgv] opens the command line.
  const IsolationPanel({
    required this.status,
    required this.onShowArgv,
    super.key,
  });

  /// What the daemon last said.
  final SandboxStatus status;

  /// Opens the command the daemon built.
  final VoidCallback onShowArgv;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // Nothing runs, so nothing was measured, and the panel says that once at
    // the top instead of three times over. Once something ran, a missing
    // result is a different matter and is called out on its own line.
    final bool nothingRuns = status.state == SandboxState.stopped;
    return SingleChildScrollView(
      key: const PageStorageKey<String>('sandbox-isolation'),
      padding: EdgeInsets.all(tokens.spacing.x3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // The measure of a sentence, not the width of the pane: a line of
          // prose that runs the whole way across is a line nobody reads to
          // the end (CONVENTIONS 4.13, "Handwerk sichtbar machen"). The
          // `Align` is what lets it be narrower than the stretched column.
          Align(
            alignment: Alignment.centerLeft,
            child: ConstrainedBox(
              constraints: BoxConstraints(
                maxWidth: HSize.measureWidth(tokens.typography.ui13.fontSize!),
              ),
              child: Text(
                nothingRuns ? l10n.isolationEmpty : l10n.isolationIntro,
                key: const Key('sandbox-isolation-intro'),
                style: tokens.typography.ui13.tinted(tokens.colors.fg1),
              ),
            ),
          ),
          SizedBox(height: tokens.spacing.x3),
          for (int i = 0; i < IsolationCheck.values.length; i++)
            _CheckLine(
              key: ValueKey<IsolationCheck>(IsolationCheck.values[i]),
              check: IsolationCheck.values[i],
              index: i,
              result: status.checkFor(IsolationCheck.values[i]),
              segment: status.segmentFor(IsolationCheck.values[i]),
              expected: !nothingRuns,
            ),
          _ExceptionLine(endpoint: status.llmEndpoint),
          SizedBox(height: tokens.spacing.x3),
          const HHairline(),
          SizedBox(height: tokens.spacing.x2),
          Align(
            alignment: Alignment.centerLeft,
            child: HButton(
              key: const Key('sandbox-isolation-argv'),
              variant: HButtonVariant.ghost,
              size: HButtonSize.sm,
              onPressed: onShowArgv,
              child: Text(l10n.isolationShowArgv),
            ),
          ),
        ],
      ),
    );
  }
}

/// One guarantee: dot, sentence, state word, and the evidence beside it.
class _CheckLine extends StatelessWidget {
  const _CheckLine({
    required this.check,
    required this.index,
    required this.result,
    required this.segment,
    required this.expected,
    super.key,
  });

  final IsolationCheck check;
  final int index;
  final IsolationCheckResult? result;
  final IsolationSegment segment;

  /// Whether a result should have arrived by now: something ran.
  final bool expected;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final IsolationCheckResult? result = this.result;
    final Diagnostic? diagnostic = result?.diagnostic;
    final bool missing =
        expected && result == null && segment == IsolationSegment.unknown;
    return Padding(
      padding: EdgeInsets.only(bottom: tokens.spacing.x2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Padding(
                // The dot sits on the text next to it, not on the top of its
                // line box.
                padding: EdgeInsets.only(top: tokens.spacing.x1),
                child: IsolationDot(segment: segment, index: index),
              ),
              SizedBox(width: tokens.spacing.x2),
              Expanded(
                flex: isolationSentenceFlex,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      isolationCheckSentence(l10n, check),
                      style: tokens.typography.ui13.tinted(tokens.colors.fg0),
                    ),
                    SizedBox(height: tokens.spacing.x1),
                    // The word next to the colour. Colour is never the only
                    // channel (`docs/UX.md` 3.3, rule 2).
                    Text(
                      isolationSegmentLabel(l10n, segment),
                      key: Key('isolation-state-${check.name}'),
                      style: tokens.typography.ui11.tinted(
                        isolationSegmentTextColor(tokens, segment),
                      ),
                    ),
                  ],
                ),
              ),
              SizedBox(width: tokens.spacing.x3),
              Expanded(
                flex: isolationEvidenceFlex,
                child: _Evidence(check: check, evidence: result?.evidence),
              ),
            ],
          ),
          if (missing)
            _Note(
              key: Key('isolation-missing-${check.name}'),
              text: l10n.isolationNotMeasured,
              color: tokens.stateText.blocked,
            ),
          if (result != null && result.walkStopped)
            _Note(
              key: Key('isolation-limit-${check.name}'),
              text: l10n.isolationWalkStopped(
                IsolationCheckResult.socketWalkEntries,
                IsolationCheckResult.socketWalkDepth,
              ),
              color: tokens.stateText.held,
            ),
          if (diagnostic != null)
            Padding(
              padding: EdgeInsets.only(top: tokens.spacing.x2),
              child: HDiagnosticCard(
                key: Key('isolation-diagnostic-${check.name}'),
                code: diagnostic.code,
                severityLabel: _severityLabel(l10n, diagnostic.severity),
                // The card keeps the hue every diagnostic wears in this
                // product; red belongs to the dot and to the ring, which say
                // "this guarantee does not hold", not "here is a finding"
                // (`docs/UX.md` 3.3, rule 6).
                color: tokens.state.error,
                title: diagnostic.title.isEmpty
                    ? diagnostic.code
                    : diagnostic.title,
                // The daemon's own sentence: it names the guarantee, not the
                // check that produced it (`docs/UX.md` 4.4).
                why: diagnostic.why,
                docsUrl: diagnostic.docsUrl,
                fix: FixControl(fix: diagnostic.fix),
                width: double.infinity,
              ),
            ),
        ],
      ),
    );
  }
}

/// The line the shim wrote, in monospace, in the right column.
///
/// Left to right whatever it contains. The daemon strips the bidi marks out
/// of it, and the base direction is pinned here as well, so a name in a
/// right-to-left script cannot reorder the evidence around it.
class _Evidence extends StatelessWidget {
  const _Evidence({required this.check, required this.evidence});

  final IsolationCheck check;
  final String? evidence;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final String? evidence = this.evidence;
    if (evidence == null || evidence.isEmpty) {
      return const SizedBox.shrink();
    }
    return ClipRect(
      // The daemon caps the evidence at 500 characters and drops the
      // invisible ones, but `is_combining` there knows six Unicode blocks and
      // not the whole `Mn` category: marks from another block are neither
      // counted nor removed, and a stack of them paints outside its own line
      // box, over the sentence next to it. The box is therefore bounded here
      // as well -- in lines and in ink -- the way the body views bound theirs.
      child: SelectableRegion(
        // The app is built on `WidgetsApp`, so `SelectableText` is out of
        // reach; `SelectableRegion` is the widgets-layer equivalent, and the
        // evidence has to be copyable to be checkable.
        selectionControls: emptyTextSelectionControls,
        child: Text(
          evidence,
          key: Key('isolation-evidence-${check.name}'),
          textDirection: TextDirection.ltr,
          maxLines: isolationEvidenceMaxLines,
          overflow: TextOverflow.ellipsis,
          style: tokens.typography.mono11.tinted(tokens.colors.fg2),
        ),
      ),
    );
  }
}

/// The fourth line: the one declared way out that is never held.
///
/// Not a check. The LLM passthrough is streamed and logged, and saying so
/// next to the three guarantees is what keeps the guarantees honest: a panel
/// that showed three green lines and kept quiet about the open path would be
/// claiming more than it proved (BACKLOG.md 4.2, `docs/THREAT-MODEL.md`
/// K-02).
class _ExceptionLine extends StatelessWidget {
  const _ExceptionLine({required this.endpoint});

  final String endpoint;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool configured = endpoint.isNotEmpty;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Padding(
          padding: EdgeInsets.only(top: tokens.spacing.x1),
          child: _Dot(
            color: configured ? tokens.state.held : tokens.colors.fg2,
            filled: configured,
          ),
        ),
        SizedBox(width: tokens.spacing.x2),
        Expanded(
          child: Text(
            configured
                ? l10n.isolationException(endpoint)
                : l10n.isolationExceptionNone,
            key: const Key('sandbox-isolation-exception'),
            style: tokens.typography.ui13.tinted(
              configured ? tokens.stateText.held : tokens.colors.fg2,
            ),
          ),
        ),
      ],
    );
  }
}

/// A sentence under a line, in the hue of what it is about.
class _Note extends StatelessWidget {
  const _Note({required this.text, required this.color, super.key});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Padding(
      padding: EdgeInsets.only(
        top: tokens.spacing.x1,
        left: isolationDotSize + tokens.spacing.x2,
      ),
      child: Align(
        alignment: Alignment.centerLeft,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: HSize.measureWidth(tokens.typography.ui12.fontSize!),
          ),
          child: Text(text, style: tokens.typography.ui12.tinted(color)),
        ),
      ),
    );
  }
}

/// The dot in front of one guarantee.
///
/// It fades to its colour over [HMotion.sweep] when the result arrives, and
/// the three dots do it one after another, [index] times
/// [HMotion.checkStagger] apart. The three results leave the same sandbox in
/// the same millisecond; turning all three at once reads as one state change
/// rather than three answers. Nothing else on the line moves, and the text is
/// readable throughout (`docs/UX.md` 2.8).
///
/// While the sandbox starts, the dot breathes in amber: a flag that says
/// "this is being measured", ending with the state it belongs to, and a
/// steady amber under reduced motion (2.7, 2.10).
class IsolationDot extends StatefulWidget {
  /// Creates the dot of the guarantee at [index].
  const IsolationDot({required this.segment, this.index = 0, super.key});

  /// What the guarantee looks like right now.
  final IsolationSegment segment;

  /// Position of the guarantee, zero based; it sets the delay.
  final int index;

  @override
  State<IsolationDot> createState() => _IsolationDotState();
}

class _IsolationDotState extends State<IsolationDot>
    with TickerProviderStateMixin {
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: HMotion.sweep,
    value: 1,
  );
  late final AnimationController _breath = AnimationController(
    vsync: this,
    duration: HMotion.breathe,
  );
  IsolationSegment _from = IsolationSegment.unknown;
  Timer? _delay;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncBreath();
  }

  @override
  void didUpdateWidget(IsolationDot old) {
    super.didUpdateWidget(old);
    if (old.segment != widget.segment) {
      _from = old.segment;
      _fade
        ..duration = HReducedMotion.displace(context, HMotion.sweep)
        ..value = 0;
      _startAfterDelay();
    }
    _syncBreath();
  }

  /// Starts the fade once the dots before this one have had their turn.
  ///
  /// The wait is a [Timer] the state owns and cancels: a delay that outlives
  /// its widget keeps a disposed tree alive and fires into nothing.
  void _startAfterDelay() {
    _delay?.cancel();
    final Duration delay = HReducedMotion.displace(
      context,
      HMotion.checkStagger * widget.index,
    );
    if (delay == Duration.zero) {
      _fade.forward();
      return;
    }
    _delay = Timer(delay, _fade.forward);
  }

  /// The breath runs only while the guarantee is being measured, and never
  /// under reduced motion; there the amber stands still and says the same
  /// thing.
  void _syncBreath() {
    final bool wanted =
        widget.segment == IsolationSegment.running &&
        !HReducedMotion.of(context);
    if (wanted && !_breath.isAnimating) {
      _breath.repeat(reverse: true);
    } else if (!wanted && _breath.isAnimating) {
      _breath
        ..stop()
        ..value = 0;
    }
  }

  @override
  void dispose() {
    _delay?.cancel();
    _fade.dispose();
    _breath.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final Color target = isolationSegmentColor(tokens, widget.segment);
    final Color from = isolationSegmentColor(tokens, _from);
    return AnimatedBuilder(
      animation: Listenable.merge(<Listenable>[_fade, _breath]),
      builder: (BuildContext context, Widget? _) => Opacity(
        opacity: widget.segment == IsolationSegment.running
            ? 1 - _breath.value * (1 - HMotion.breatheMinOpacity)
            : 1,
        child: _Dot(
          color: _colour(tokens, from, target),
          filled: isolationSegmentFilled(widget.segment),
        ),
      ),
    );
  }

  /// The colour of the dot at this point of the fade.
  ///
  /// Two state colours are never interpolated into one another: amber lerped
  /// towards red runs through the orange that means "error" and would show a
  /// state that never happened (`docs/UX.md` 3.3, rule 3). The fade therefore
  /// goes out through the grey of "unknown" and back in on the other side,
  /// and grey is the absence of a state rather than another one.
  Color _colour(HTokens tokens, Color from, Color target) {
    final Color grey = tokens.colors.fg2;
    if (_fade.value >= 1) {
      return target;
    }
    return _fade.value < 0.5
        ? Color.lerp(from, grey, _fade.value * 2) ?? target
        : Color.lerp(grey, target, (_fade.value - 0.5) * 2) ?? target;
  }
}

/// The dot itself: filled when something was measured, a ring when not.
///
/// The hollow shape is the second channel of "not measured", and it covers
/// both ways of not being measured: the guarantee nobody asked about and the
/// one whose sandbox is still coming up. Either differs from a proven one in
/// shape as well as in hue, so neither can be read as a paler green
/// ([isolationSegmentFilled]).
class _Dot extends StatelessWidget {
  const _Dot({required this.color, required this.filled});

  final Color color;
  final bool filled;

  @override
  Widget build(BuildContext context) => SizedBox.square(
    dimension: isolationDotSize,
    child: DecoratedBox(
      decoration: BoxDecoration(
        shape: BoxShape.circle,
        color: filled ? color : null,
        border: filled ? null : Border.all(color: color),
      ),
    ),
  );
}

/// The label of a severity, in the person's language.
String _severityLabel(AppLocalizations l10n, Severity severity) =>
    switch (severity) {
      Severity.info => l10n.diagSeverityInfo,
      Severity.warning => l10n.diagSeverityWarning,
      Severity.error => l10n.diagSeverityError,
      Severity.blocking => l10n.diagSeverityBlocking,
    };
