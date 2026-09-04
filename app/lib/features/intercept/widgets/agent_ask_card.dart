/// The card that shows what the agent asked for (HUM-073, ADR-014).
///
/// The agent has exactly one channel out of the sandbox, and
/// `POST http://humanitl.internal/ask` is the one direction on it that carries
/// the agent's own words. Everything on this card follows from that:
///
/// * **The text is foreign text.** It is drawn as plain text and nothing else
///   — no Markdown, no link, no tap target, no rich span, no selection layer.
///   The daemon has already taken line breaks, control characters, invisible
///   characters and stacked combining marks out of it (`sanitize_note`,
///   HUM-072); the card adds the second half of the same promise by never
///   interpreting what is left, by bounding its lines and by clipping its box.
/// * **It never pretends to be us.** The card carries the agent's name and the
///   violet of the passthrough rail, the one hue in the palette that means
///   "this came from the model side", and its screen-reader label names the
///   source before the words. A request styled like a system message would be
///   the whole attack.
/// * **Nothing is truncated on the right.** A host cut to `pypi.org…` would be
///   domain deception by our own surface, at the exact moment somebody
///   decides. Names wrap; they are never shortened.
/// * **The button opens a form, it does not act, and the form is narrow.** The
///   sheet prefills host **and** path from the URL the daemon found, picks no
///   action at all, and says out loud when a rule would cover a whole host.
///   Nothing is written until the human confirms (ADR-014: a request is a
///   request, never an action).
library;

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ipc/client_providers.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/agent_asks.dart';

/// How much of the queue pane the open requests may take before they scroll.
const double agentAskMaxHeight = 220;

/// How many lines of the agent's text a card draws.
///
/// Five hundred characters — the daemon's cap — fit in far fewer than this at
/// any pane width. The bound exists for the text that is trying to take the
/// pane, not for the one that is trying to be read.
const int agentAskMaxLines = 8;

/// The requests of the agent, above the queue.
///
/// Draws nothing at all while there is none: an empty strip would take a line
/// of the queue for something that is not there.
class AgentAskStrip extends ConsumerWidget {
  /// Creates the strip.
  const AgentAskStrip({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final List<AgentAsk> asks = ref.watch(agentAsksProvider);
    if (asks.isEmpty) {
      return const SizedBox.shrink();
    }
    return ConstrainedBox(
      constraints: const BoxConstraints(maxHeight: agentAskMaxHeight),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            for (final AgentAsk ask in asks)
              _AskArrival(
                key: ValueKey<String>('agent-ask:${ask.id}'),
                child: AgentAskCard(ask: ask),
              ),
          ],
        ),
      ),
    );
  }
}

/// One request of the agent.
class AgentAskCard extends ConsumerWidget {
  /// Creates the card for [ask].
  const AgentAskCard({required this.ask, super.key});

  /// The request the card stands for.
  final AgentAsk ask;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    // The violet of the passthrough rail: the one hue in the palette that
    // stands for "this came from the model side". The rail takes the surface
    // colour, the word the text variant — a surface may measure 3:1, a word
    // may not (`docs/UX.md` 6).
    final Color rail = tokens.stateColor(HFlowState.passthroughLlm);
    final Color label = tokens.stateTextColor(HFlowState.passthroughLlm);
    return Semantics(
      container: true,
      label: l10n.interceptAgentAskSemantics(ask.text),
      // Der Rahmen wird hart beschnitten. Die Säuberung im Daemon begrenzt
      // kombinierende Zeichen nur in den verbreiteten Blöcken
      // (`humanitl_core::block::is_combining`), und eine unvollständige
      // Prüfung darf nicht das Einzige sein, was einen Buchstabenstapel davon
      // abhält, über die Bedienelemente daneben zu laufen. Das Clipping gilt
      // für jedes Zeichen, gleich aus welchem Block.
      child: ClipRect(
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: tokens.colors.bg2,
            border: Border(
              left: BorderSide(color: rail, width: HSize.stateRail),
              bottom: BorderSide(color: tokens.colors.line),
            ),
          ),
          child: Padding(
            padding: EdgeInsets.all(tokens.spacing.x2),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    HBadge(text: l10n.interceptAgentAskFrom, color: label),
                    SizedBox(width: tokens.spacing.x2),
                    Expanded(
                      child: Text(
                        l10n.interceptAgentAskTitle,
                        style: tokens.typography.ui13.semibold.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                    ),
                    HIconButton(
                      glyph: HGlyph.close,
                      semanticsLabel: l10n.interceptAgentAskDismiss,
                      onPressed: () =>
                          ref.read(agentAsksProvider.notifier).dismiss(ask.id),
                    ),
                  ],
                ),
                SizedBox(height: tokens.spacing.x1),
                // The agent's own words. `Text`, never `Text.rich`, never a
                // `Linkify`, never inside a `SelectionArea`: whatever the
                // model wrote stays a string on screen. The monospace face is
                // the second signal that this is quoted material and not a
                // sentence of the application's own.
                Text(
                  ask.text,
                  key: const Key('intercept-agent-ask-text'),
                  maxLines: agentAskMaxLines,
                  overflow: TextOverflow.fade,
                  style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                ),
                SizedBox(height: tokens.spacing.x2),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    HButton(
                      key: const Key('intercept-agent-ask-open-rule'),
                      onPressed: () => ref
                          .read(agentAskRuleDraftProvider.notifier)
                          .open(ask),
                      child: Text(l10n.interceptAgentAskCreateRule),
                    ),
                    if (ask.suggestedTarget.isNotEmpty) ...<Widget>[
                      SizedBox(width: tokens.spacing.x2),
                      // **Nie rechts abschneiden.** Eine Ellipse macht aus
                      // `pypi.org.attacker.com` ein `pypi.org…`, und das ist
                      // Domain-Täuschung durch die eigene Oberfläche, genau in
                      // dem Augenblick, in dem ein Mensch entscheidet. Der
                      // Name bricht deshalb um; sichtbar ist immer der ganze.
                      //
                      // Die registrierbare Domäne wird auch nicht
                      // hervorgehoben: `psl.dart` rät sie aus einer kurzen
                      // Tabelle, und ein falsch geratener Apex wäre dieselbe
                      // Täuschung mit umgekehrtem Vorzeichen
                      // (`backlog/CONVENTIONS.md` 4.13).
                      Flexible(
                        child: Text(
                          ask.suggestedTarget,
                          key: const Key('intercept-agent-ask-host'),
                          style: tokens.typography.mono12.tinted(
                            tokens.colors.fg2,
                          ),
                        ),
                      ),
                    ],
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// An arrival: [HMotion.arriveOffset] of travel plus a fade, over
/// [HMotion.arrive] on [HMotion.enter].
///
/// The card explains itself by coming from above, the direction the queue
/// grows from, and it stops moving the moment it is there: the wrapper takes
/// itself out of the tree when the animation is done (`docs/UX.md` 2.2 and 7).
/// Under reduced motion the travel is zero and the fade keeps its duration.
class _AskArrival extends StatefulWidget {
  const _AskArrival({required this.child, super.key});

  final Widget child;

  @override
  State<_AskArrival> createState() => _AskArrivalState();
}

class _AskArrivalState extends State<_AskArrival>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _controller,
    curve: HMotion.enter,
  );
  bool _done = false;

  @override
  void initState() {
    super.initState();
    _controller
      ..addStatusListener(_finish)
      ..forward();
  }

  void _finish(AnimationStatus status) {
    if (status == AnimationStatus.completed && mounted && !_done) {
      setState(() => _done = true);
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (_done) {
      return widget.child;
    }
    final double distance = HReducedMotion.distance(
      context,
      HMotion.arriveOffset,
    );
    final Widget faded = FadeTransition(opacity: _curve, child: widget.child);
    if (distance == 0) {
      return faded;
    }
    // The travel is a pixel count on a box whose height nobody knows in
    // advance, so it is a transform and not a `SlideTransition`; the child is
    // handed to the builder, so only the transform rebuilds per frame.
    return AnimatedBuilder(
      animation: _curve,
      child: faded,
      builder: (BuildContext context, Widget? child) => Transform.translate(
        offset: Offset(0, -distance * (1 - _curve.value)),
        child: child,
      ),
    );
  }
}

/// The draft the sheet edits, or null while no sheet is open.
@immutable
class AgentAskRuleDraft {
  /// Creates a draft.
  const AgentAskRuleDraft({
    required this.askId,
    required this.host,
    required this.path,
    this.action,
    this.forever = false,
    this.saving = false,
    this.failure,
  });

  /// The request this draft came from.
  final String askId;

  /// The host, prefilled from the request where the daemon found one.
  final String host;

  /// The path, prefilled from the same URL; empty means every path.
  final String path;

  /// What the rule does, or null while nobody has chosen.
  ///
  /// **There is no default.** The rule is born from a request written by a
  /// program nobody trusts; the one field that decides whether traffic flows
  /// is the one field that must be picked by hand. A preselected `allow` would
  /// turn one confirming click into network access, and a preselected `ask`
  /// would write a rule that changes nothing while looking like it did.
  final RuleAction? action;

  /// True for a permanent rule, false for one that ends with the session.
  final bool forever;

  /// True while the daemon is writing it.
  final bool saving;

  /// What the daemon said when it refused, or null.
  final String? failure;

  /// True when the rule would cover every path of [host].
  bool get coversWholeHost => path.trim().isEmpty;

  /// True when the daemon may be asked to write this rule.
  bool get ready => action != null && hostPatternProblem(host.trim()) == null;

  /// The same draft with single fields replaced.
  ///
  /// [failure] is cleared on every edit: a message about the last attempt
  /// would otherwise stand next to a field somebody has since corrected.
  AgentAskRuleDraft copyWith({
    String? host,
    String? path,
    RuleAction? action,
    bool? forever,
    bool? saving,
    String? failure,
  }) => AgentAskRuleDraft(
    askId: askId,
    host: host ?? this.host,
    path: path ?? this.path,
    action: action ?? this.action,
    forever: forever ?? this.forever,
    saving: saving ?? this.saving,
    failure: failure,
  );
}

/// The open rule sheet of the intercept screen, or null.
///
/// The rule editor of the rules screen is out of reach from here: a feature
/// never imports another feature (`docs/ARCHITECTURE.md` 5, enforced by
/// `tools/check-deps.sh`). What this sheet offers is therefore deliberately
/// small — action, host, path, how long — and everything else about the rule
/// is edited where rules live.
final NotifierProvider<AgentAskRuleDraftNotifier, AgentAskRuleDraft?>
agentAskRuleDraftProvider =
    NotifierProvider<AgentAskRuleDraftNotifier, AgentAskRuleDraft?>(
      AgentAskRuleDraftNotifier.new,
    );

/// Opens, edits and closes the draft of [agentAskRuleDraftProvider].
class AgentAskRuleDraftNotifier extends Notifier<AgentAskRuleDraft?> {
  @override
  AgentAskRuleDraft? build() => null;

  /// Opens the sheet for [ask], with host and path the daemon suggested.
  void open(AgentAsk ask) {
    state = AgentAskRuleDraft(
      askId: ask.id,
      host: ask.suggestedHost,
      path: ask.suggestedPath,
    );
  }

  /// Closes the sheet without writing anything.
  void close() => state = null;

  /// Replaces the host.
  void setHost(String host) => state = state?.copyWith(host: host);

  /// Replaces the path.
  void setPath(String path) => state = state?.copyWith(path: path);

  /// Replaces the action.
  void setAction(RuleAction action) => state = state?.copyWith(action: action);

  /// Switches between a session rule and a permanent one.
  void setForever({required bool forever}) =>
      state = state?.copyWith(forever: forever);

  /// Writes the rule and closes the sheet.
  ///
  /// The request itself goes off the queue with it: the human has answered it.
  /// A refusal keeps the sheet open with what was typed in it and puts the
  /// daemon's own sentence next to the buttons — a rule that silently fails to
  /// appear is worse than one that was never attempted, because the human
  /// walks away believing it exists.
  Future<void> submit() async {
    final AgentAskRuleDraft? draft = state;
    final RuleAction? action = draft?.action;
    if (draft == null || action == null || !draft.ready || draft.saving) {
      return;
    }
    state = draft.copyWith(saving: true);
    try {
      await ref
          .read(daemonClientProvider)
          .addRule(
            Rule(
              action: action,
              matcher: RuleMatcher(
                host: draft.host.trim(),
                path: draft.path.trim(),
              ),
              expires: draft.forever
                  ? const RuleExpiry.never()
                  : const RuleExpiry.session(),
            ),
          );
    } on DaemonException catch (error) {
      state = draft.copyWith(failure: error.diagnostic.why);
      return;
    } on Object catch (error) {
      state = draft.copyWith(failure: error.toString());
      return;
    }
    ref.read(agentAsksProvider.notifier).dismiss(draft.askId);
    state = null;
  }
}

/// The sheet that turns a request into a rule.
class AgentAskRuleSheet extends ConsumerStatefulWidget {
  /// Creates the sheet for [draft].
  const AgentAskRuleSheet({required this.draft, super.key});

  /// What is being edited.
  final AgentAskRuleDraft draft;

  @override
  ConsumerState<AgentAskRuleSheet> createState() => _AgentAskRuleSheetState();
}

class _AgentAskRuleSheetState extends ConsumerState<AgentAskRuleSheet> {
  late final TextEditingController _host = TextEditingController(
    text: widget.draft.host,
  );
  late final TextEditingController _path = TextEditingController(
    text: widget.draft.path,
  );

  @override
  void dispose() {
    _host.dispose();
    _path.dispose();
    super.dispose();
  }

  /// What is wrong with the host as it stands, already localised.
  String? _hostProblem(AppLocalizations l10n) =>
      switch (hostPatternProblem(widget.draft.host.trim())) {
        null => null,
        HostPatternProblem.empty => l10n.rulesHostEmpty,
        HostPatternProblem.wildcardInLabel => l10n.rulesHostWildcard,
        HostPatternProblem.emptyLabel => l10n.rulesHostEmptyLabel,
        HostPatternProblem.notAnAddress => l10n.rulesHostAddress,
        HostPatternProblem.notALabel => l10n.rulesHostLabel,
      };

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final AgentAskRuleDraftNotifier notifier = ref.read(
      agentAskRuleDraftProvider.notifier,
    );
    final AgentAskRuleDraft draft = widget.draft;
    final String? hostProblem = _hostProblem(l10n);
    return HSheet(
      title: Text(l10n.interceptAgentAskRuleTitle),
      closeSemanticsLabel: l10n.interceptAgentAskRuleClose,
      onClose: notifier.close,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          _Label(text: l10n.interceptAgentAskRuleHost),
          SizedBox(height: tokens.spacing.x1),
          HTextField(
            key: const Key('intercept-agent-ask-rule-host'),
            controller: _host,
            semanticsLabel: l10n.interceptAgentAskRuleHost,
            onChanged: notifier.setHost,
          ),
          if (hostProblem != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            _Note(
              key: const Key('intercept-agent-ask-rule-host-problem'),
              text: hostProblem,
              color: tokens.stateTextColor(HFlowState.blocked),
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          _Label(text: l10n.interceptAgentAskRulePath),
          SizedBox(height: tokens.spacing.x1),
          HTextField(
            key: const Key('intercept-agent-ask-rule-path'),
            controller: _path,
            semanticsLabel: l10n.interceptAgentAskRulePath,
            onChanged: notifier.setPath,
          ),
          // Der Satz, den ein Mensch lesen muss, bevor er einen ganzen Host
          // aufmacht. Er steht als Warnung da, nicht als Fußnote: Ohne Pfad
          // gilt die Regel für jede Methode und jeden Pfad, und der Agent hat
          // nach einer Adresse gefragt.
          if (draft.coversWholeHost && hostProblem == null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            _Note(
              key: const Key('intercept-agent-ask-rule-whole-host'),
              text: l10n.interceptAgentAskRuleWholeHost(draft.host.trim()),
              color: tokens.stateTextColor(HFlowState.held),
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          _Label(text: l10n.interceptAgentAskRuleAction),
          SizedBox(height: tokens.spacing.x1),
          // Nichts ist vorgewählt. Der Knopf bleibt aus, bis jemand wählt.
          HSegmented<RuleAction?>(
            options: <HSegmentOption<RuleAction?>>[
              HSegmentOption<RuleAction?>(
                value: RuleAction.allow,
                label: l10n.interceptAgentAskRuleAllow,
              ),
              HSegmentOption<RuleAction?>(
                value: RuleAction.block,
                label: l10n.interceptAgentAskRuleBlock,
              ),
            ],
            selected: draft.action,
            onSelect: (RuleAction? action) {
              if (action != null) {
                notifier.setAction(action);
              }
            },
          ),
          SizedBox(height: tokens.spacing.x2),
          HSegmented<bool>(
            options: <HSegmentOption<bool>>[
              HSegmentOption<bool>(
                value: false,
                label: l10n.interceptAgentAskRuleSession,
              ),
              HSegmentOption<bool>(
                value: true,
                label: l10n.interceptAgentAskRuleForever,
              ),
            ],
            selected: draft.forever,
            onSelect: (bool forever) => notifier.setForever(forever: forever),
          ),
          if (draft.action == null) ...<Widget>[
            SizedBox(height: tokens.spacing.x1),
            _Note(
              key: const Key('intercept-agent-ask-rule-choose'),
              text: l10n.interceptAgentAskRuleChoose,
              color: tokens.colors.fg2,
            ),
          ],
          if (draft.failure != null) ...<Widget>[
            SizedBox(height: tokens.spacing.x3),
            _Note(
              key: const Key('intercept-agent-ask-rule-failure'),
              text: l10n.interceptAgentAskRuleFailed(draft.failure ?? ''),
              color: tokens.stateTextColor(HFlowState.blocked),
            ),
          ],
          SizedBox(height: tokens.spacing.x3),
          Row(
            children: <Widget>[
              HButton(
                key: const Key('intercept-agent-ask-rule-create'),
                variant: HButtonVariant.primary,
                onPressed: draft.ready && !draft.saving
                    ? notifier.submit
                    : null,
                child: Text(l10n.interceptAgentAskRuleCreate),
              ),
              SizedBox(width: tokens.spacing.x2),
              HButton(
                variant: HButtonVariant.ghost,
                onPressed: notifier.close,
                child: Text(l10n.interceptAgentAskRuleCancel),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// The caption over a field of the sheet.
class _Label extends StatelessWidget {
  const _Label({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Text(text, style: tokens.typography.ui12.tinted(tokens.colors.fg1));
  }
}

/// A sentence under a field: a warning, a refusal, a hint.
class _Note extends StatelessWidget {
  const _Note({required this.text, required this.color, super.key});

  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    return Text(text, style: tokens.typography.ui12.tinted(color));
  }
}
