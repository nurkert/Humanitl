/// The one modal of this screen (HUM-029, HUM-028, `docs/UX.md` 5.4).
///
/// Two things reach far enough for it: a decision over more than
/// [modalAboveReach] requests, and a rule that outlives the session. A modal
/// per click would destroy the rhythm of the queue; these two cannot be taken
/// back and have to name what they cover. It catches the focus, starts on the
/// harmless action, closes on `Escape`, and takes the decision keys of the
/// screen out of service while it stands.
library;

import 'dart:async';

// `Flow` is a domain type here, not the Flutter layout widget of the same
// name; the widget is never used in this feature.
import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart' hide Flow;
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/domain/domain.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../providers/decision.dart';
import '../../../core/text/rule_sentence.dart' as sentence;
import '../rule_sentence.dart';

/// How many paths the modal lists before it counts the rest.
const int modalPathSample = 8;

/// The confirmation over a whole group.
class BatchModal extends ConsumerStatefulWidget {
  /// Creates the modal for [request].
  const BatchModal({required this.request, super.key});

  /// What is about to happen, and to how many requests.
  final BatchRequest request;

  @override
  ConsumerState<BatchModal> createState() => _BatchModalState();
}

class _BatchModalState extends ConsumerState<BatchModal>
    with SingleTickerProviderStateMixin {
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  )..forward();
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _fade,
    curve: HMotion.enter,
  );
  final FocusScopeNode _scope = FocusScopeNode(debugLabel: 'intercept-batch');

  /// Scrolls the body when it does not fit the window (HUM-249).
  final ScrollController _body = ScrollController();

  /// True once the body stood on screen to its end: it fits, or it was
  /// scrolled there.
  ///
  /// Until then confirming does nothing. The body carries the rule sentence
  /// and the paths, and a rule whose sentence was cut off by a short window
  /// would be saved without anybody having read it (`docs/UX.md` 4.6,
  /// `backlog/CONVENTIONS.md` 4.13). Once seen, it stays seen: scrolling
  /// back up does not take the confirmation away again.
  bool _seenAll = false;

  /// True while the body is taller than the room the window leaves it and
  /// was not yet seen to its end; the hint above it says why confirming
  /// waits.
  bool _overflows = false;

  @override
  void initState() {
    super.initState();
    // Erst nach dem ersten Layout weiß die Scrollposition, ob der Rumpf
    // passt.
    WidgetsBinding.instance.addPostFrameCallback((Duration _) => _check());
  }

  /// Marks the body as seen when its end is on screen, and says whether it
  /// was seen.
  ///
  /// Read from the scroll position as it is laid out now, so that a click in
  /// the very frame the modal appears in is judged by what stands on screen
  /// and not by a flag the frame after sets: a body that fits counts as seen
  /// at once, and its confirm button never flickers from off to on.
  bool _check() {
    if (!mounted) {
      return false;
    }
    if (_seenAll) {
      return true;
    }
    if (!_body.hasClients || !_body.position.hasContentDimensions) {
      return false;
    }
    final ScrollPosition position = _body.position;
    final bool atEnd = position.pixels >= position.maxScrollExtent - 0.5;
    if (atEnd || !_overflows) {
      setState(() {
        _seenAll = atEnd;
        _overflows = !atEnd;
      });
    }
    return atEnd;
  }

  late final Map<ShortcutActivator, VoidCallback> _bindings =
      <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.escape): _cancel,
      };

  @override
  void dispose() {
    _curve.dispose();
    _fade.dispose();
    _scope.dispose();
    _body.dispose();
    super.dispose();
  }

  void _cancel() => ref.read(batchConfirmProvider.notifier).cancel();

  void _confirm() {
    if (!_check()) {
      return;
    }
    unawaited(ref.read(interceptDecisionProvider.notifier).confirmBatch());
  }

  /// The rule the draft of the selection would save with [request].
  String _ruleSentence(BatchRequest request) {
    final RememberState draft = ref.watch(rememberDraftProvider);
    return ruleSentence(
      RuleDraft(
        duration: request.remember && !draft.open
            ? RememberDuration.session
            : draft.effective,
        target: draft.target,
        flow: request.flows.first,
        action: request.kind == DecisionKind.block
            ? RuleAction.block
            : RuleAction.allow,
      ),
      context.l10n,
      apexOf: ref.watch(apexResolverProvider),
    );
  }

  /// Der Satz zu [rule], dem Objekt, das das Bestätigen speichert.
  ///
  /// Eine Regel für eine Stunde läuft ab dem Öffnen des Modals; der Satz
  /// nennt deshalb die genaue Uhrzeit ihres Endes und nicht nur „1 Stunde",
  /// die beim Bestätigen schon weniger wäre (Review Runde 6).
  String _shownSentence(Rule rule, AppLocalizations l10n) {
    final String text = sentence.ruleSentence(rule, l10n, now: DateTime.now());
    final String exact = sentence.ruleExpiryExact(rule.expires, l10n);
    return exact.isEmpty ? text : '$text ($exact)';
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final BatchRequest request = widget.request;
    final bool blocking = request.kind == DecisionKind.block;
    final bool forever = request.reason == ConfirmReason.forever;
    final List<Flow> flows = request.flows;
    final int rest = flows.length - modalPathSample;
    // One host is named; several are counted. A registrable domain worked out
    // here would be a guess in the sentence that guards the send
    // (`backlog/CONVENTIONS.md` 4.13).
    final String where = request.hostCount == 1
        ? request.host
        : l10n.interceptSeveralHosts(request.hostCount);
    // The rule as a sentence, so that what is about to be saved can be read
    // before it is saved (`docs/UX.md` 4.6). Eine Frage, die nicht aus der
    // Auswahl kommt, speichert keine Regel, also nennt sie auch keine: Der
    // Merk-Entwurf gehört der ausgewählten Anfrage (HUM-218).
    // Der Satz der Regel, die das Bestätigen speichert: das Objekt aus der
    // Anfrage, beim Öffnen einmal gebaut (HUM-058, Review Runde 5). Nur die
    // Regel aus einer abgelaufenen Anfrage entsteht erst beim Bestätigen und
    // wird hier noch aus dem Entwurf gelesen.
    final ShownRule? shown = request.shownRule;
    final String rule = shown != null
        ? _shownSentence(shown.rule, l10n)
        : request.ruleOnly
        ? _ruleSentence(request)
        : '';
    return CallbackShortcuts(
      bindings: _bindings,
      child: FocusScope(
        node: _scope,
        child: FadeTransition(
          opacity: _curve,
          child: NotificationListener<Notification>(
            onNotification: (Notification notification) {
              if (notification is ScrollNotification ||
                  notification is ScrollMetricsNotification) {
                _check();
              }
              return false;
            },
            child: HModal(
              key: const Key('intercept-batch-modal'),
              width: 480,
              bodyScrollController: _body,
              onDismiss: _cancel,
              scrimSemanticsLabel: l10n.interceptCancel,
              title: Text(
                forever
                    ? l10n.interceptForeverTitle(where)
                    : blocking
                    ? l10n.interceptBlockManyTitle(flows.length, where)
                    : l10n.interceptAllowManyTitle(flows.length, where),
              ),
              actions: <Widget>[
                HButton(
                  key: const Key('intercept-batch-cancel'),
                  // The harmless action carries the focus: a modal that opens on
                  // the irreversible one turns `Enter` into the decision
                  // (`docs/UX.md` 5.4).
                  autofocus: true,
                  onPressed: _cancel,
                  child: Text(l10n.interceptCancel),
                ),
                HButton(
                  key: const Key('intercept-batch-confirm'),
                  variant: blocking
                      ? HButtonVariant.danger
                      : HButtonVariant.primary,
                  size: HButtonSize.md,
                  // Aus erst, wenn feststeht, dass der Rumpf nicht passt;
                  // bis dahin prüft `_confirm` selbst, was gerade dasteht.
                  onPressed: _overflows && !_seenAll ? null : _confirm,
                  child: Text(
                    forever
                        ? (blocking
                              ? l10n.interceptForeverConfirmBlock
                              // Eine Regel aus einer abgelaufenen Anfrage
                              // sendet nichts (HUM-058, Zeile 7).
                              : request.ruleOnly
                              ? l10n.interceptForeverConfirmRuleOnly
                              : l10n.interceptForeverConfirm)
                        : blocking
                        ? l10n.interceptBlockGroup(flows.length)
                        : l10n.interceptAllowGroup(flows.length),
                  ),
                ),
              ],
              // Solange der Rumpf nicht ganz zu sehen war, ist er ein Fokusstopp:
              // Pfeile und Bild-ab scrollen ihn dann auch ohne Maus
              // (`docs/UX.md` 5.1). Danach fällt der Stopp weg, und `Tab` geht
              // wie vorher nur über die beiden Knöpfe.
              child: Focus(
                canRequestFocus: !_seenAll,
                skipTraversal: _seenAll,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    if (!_seenAll && _overflows) ...<Widget>[
                      Text(
                        l10n.interceptBatchScrollToRead,
                        key: const Key('intercept-batch-scroll-hint'),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                      ),
                      SizedBox(height: tokens.spacing.x2),
                    ],
                    Text(
                      forever
                          ? l10n.interceptForeverBody
                          : blocking
                          ? l10n.interceptBlockManyBody
                          : l10n.interceptAllowManyBody,
                    ),
                    if (rule.isNotEmpty) ...<Widget>[
                      SizedBox(height: tokens.spacing.x2),
                      Text(
                        rule,
                        key: const Key('intercept-batch-rule'),
                        // Ganz, nie gekürzt: Bestätigt wird genau diese Regel,
                        // und ein abgeschnittener Satz ist einer, den niemand
                        // ganz gelesen hat (HUM-249, `docs/UX.md` 4.6).
                        style: tokens.typography.mono12.tinted(
                          tokens.colors.fg0,
                        ),
                      ),
                    ],
                    SizedBox(height: tokens.spacing.x3),
                    // The paths, so that nobody decides over a list they never
                    // saw (`backlog/CONVENTIONS.md` 4.13).
                    for (final Flow flow in flows.take(modalPathSample))
                      Padding(
                        padding: EdgeInsets.only(bottom: tokens.spacing.x1),
                        child: Text(
                          '${flow.methodLabel} ${flow.host}${flow.path}',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: tokens.typography.mono12.tinted(
                            tokens.colors.fg1,
                          ),
                        ),
                      ),
                    if (rest > 0)
                      Text(
                        l10n.interceptBatchMore(rest),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                      ),
                    // Was nicht mitgeht, steht hier: Ein Fund braucht das Ventil
                    // oder die Pause, kein Modal (HUM-207, `docs/UX.md` 4.7).
                    if (request.withheld > 0) ...<Widget>[
                      SizedBox(height: tokens.spacing.x2),
                      Text(
                        l10n.interceptBatchWithheld(request.withheld),
                        key: const Key('intercept-batch-withheld'),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                      ),
                    ],
                    // Was unter der harten Sperre steht, geht nur bearbeitet
                    // hinaus und bleibt deshalb stehen (HUM-159).
                    if (request.locked > 0) ...<Widget>[
                      SizedBox(height: tokens.spacing.x2),
                      Text(
                        l10n.interceptBatchLocked(request.locked),
                        key: const Key('intercept-batch-locked'),
                        style: tokens.typography.ui12.tinted(tokens.colors.fg0),
                      ),
                    ],
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
