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
  late final Map<ShortcutActivator, VoidCallback> _bindings =
      <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.escape): _cancel,
      };

  @override
  void dispose() {
    _curve.dispose();
    _fade.dispose();
    _scope.dispose();
    super.dispose();
  }

  void _cancel() => ref.read(batchConfirmProvider.notifier).cancel();

  void _confirm() =>
      unawaited(ref.read(interceptDecisionProvider.notifier).confirmBatch());

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
    final RememberState draft = ref.watch(rememberDraftProvider);
    // The rule as a sentence, so that what is about to be saved can be read
    // before it is saved (`docs/UX.md` 4.6).
    final String rule = ruleSentence(
      RuleDraft(
        duration: request.remember && !draft.open
            ? RememberDuration.session
            : draft.effective,
        target: draft.target,
        flow: flows.first,
        action: blocking ? RuleAction.block : RuleAction.allow,
      ),
      l10n,
      apexOf: ref.watch(apexResolverProvider),
    );
    return CallbackShortcuts(
      bindings: _bindings,
      child: FocusScope(
        node: _scope,
        child: FadeTransition(
          opacity: _curve,
          child: HModal(
            key: const Key('intercept-batch-modal'),
            width: 480,
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
                onPressed: _confirm,
                child: Text(
                  forever
                      ? (blocking
                            ? l10n.interceptForeverConfirmBlock
                            : l10n.interceptForeverConfirm)
                      : blocking
                      ? l10n.interceptBlockGroup(flows.length)
                      : l10n.interceptAllowGroup(flows.length),
                ),
              ),
            ],
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
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
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: tokens.typography.mono12.tinted(tokens.colors.fg0),
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
                      style: tokens.typography.mono12.tinted(tokens.colors.fg1),
                    ),
                  ),
                if (rest > 0)
                  Text(
                    l10n.interceptBatchMore(rest),
                    style: tokens.typography.ui12.tinted(tokens.colors.fg1),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
