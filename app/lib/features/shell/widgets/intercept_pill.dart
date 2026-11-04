/// The intercept pill of the header and the question before a pause
/// (HUM-407).
///
/// The pill says whether every request no rule decides waits for a person.
/// With a daemon that can pause intercepting (`intercept.pause`), it is a
/// button: on "Intercept ON" a click asks first, in a modal whose preselected
/// answer is Cancel (`docs/UX.md` 5.4); on "Intercept PAUSED" a click
/// intercepts again at once, without a question, because going back to the
/// safe side never needs one. While paused it stands in the warning colour,
/// and the shell lays the caution tape across the window (3.6).
///
/// The pill only shows and asks; the daemon holds the pause and decides what
/// it means (ADR-018).
library;

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../../core/ipc/connection.dart';
import '../../../core/ipc/daemon_client.dart';
import '../../../core/shortcuts/shortcut_catalog.dart';
import '../../../core/ui/hover_label.dart';
import '../../../core/ui/ui.dart';
import '../../../l10n/l10n.dart';
import '../../tray/providers/notice.dart';
import '../providers/intercept_pause.dart';

/// The pill in the header.
class InterceptPill extends ConsumerWidget {
  /// Creates the pill; [onAskPause] opens the question before a pause. Without
  /// it the pill only shows.
  const InterceptPill({this.onAskPause, super.key});

  /// Opens [InterceptPauseDialog].
  final VoidCallback? onAskPause;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    final bool live = ref.watch(linkLiveProvider);
    final bool pausable = ref.watch(interceptPausableProvider);
    final bool paused = ref.watch(interceptPauseProvider).paused;
    final VoidCallback? ask = onAskPause;
    // Ohne Daemon faengt niemand etwas ab, und eine gruene Pille, die
    // „Intercept ON" sagt, behauptet dann das Gegenteil. Sie wird grau und
    // sagt, warum -- dieselbe Behandlung wie der Ring daneben und der Punkt
    // in der Statuszeile (`docs/UX.md` 3.3: jede Zustandsfarbe traegt ein
    // Klartext-Label). Ein Daemon, der nicht pausieren kann, bekommt die
    // Pille wie bisher, als Anzeige ohne Knopf.
    if (!live || !pausable || ask == null) {
      return IgnorePointer(
        child: HBadge(
          key: const Key('header-intercept-badge'),
          text: live ? l10n.shellInterceptOn : l10n.shellInterceptUnknown,
          color: live ? tokens.state.allowed : tokens.colors.fg1,
        ),
      );
    }
    final String text = paused
        ? l10n.shellInterceptPaused
        : l10n.shellInterceptOn;
    final String hint = paused
        ? l10n.shellInterceptResumeHint
        : l10n.shellInterceptPauseHint;
    return HoverLabel(
      label: hint,
      side: HoverLabelSide.below,
      child: HBadge(
        key: const Key('header-intercept-badge'),
        text: text,
        color: paused ? tokens.colors.warning : tokens.state.allowed,
        textColor: paused ? tokens.colors.warningText : null,
        semanticsLabel: '$text. $hint',
        onTap: paused ? () => unawaited(_resume(ref)) : ask,
      ),
    );
  }

  /// Intercepts again; a refusal of the daemon stands as a notice.
  static Future<void> _resume(WidgetRef ref) async {
    try {
      await ref.read(interceptPauseProvider.notifier).resume();
    } on DaemonException catch (error) {
      ref.read(attentionNoticeProvider.notifier).show(error.diagnostic);
    }
  }
}

/// The question before intercepting is paused.
///
/// One sentence on what happens, one on what stays. The focus opens on
/// Cancel, `Escape` and the scrim both cancel, and only a deliberate press
/// on the second button pauses anything (`docs/UX.md` 5.4).
class InterceptPauseDialog extends ConsumerStatefulWidget {
  /// Creates the dialog; [onClose] takes it off the screen, after a pause as
  /// after a cancel.
  const InterceptPauseDialog({required this.onClose, super.key});

  /// Closes the dialog.
  final VoidCallback onClose;

  @override
  ConsumerState<InterceptPauseDialog> createState() =>
      _InterceptPauseDialogState();
}

class _InterceptPauseDialogState extends ConsumerState<InterceptPauseDialog>
    with SingleTickerProviderStateMixin {
  late final AnimationController _fade = AnimationController(
    vsync: this,
    duration: HMotion.arrive,
  );
  late final CurvedAnimation _curve = CurvedAnimation(
    parent: _fade,
    curve: HMotion.enter,
  );
  final FocusScopeNode _scope = FocusScopeNode(debugLabel: 'intercept-pause');

  /// True while the pause is on its way; a second press does nothing.
  bool _sending = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Ein Modal erscheint mit einem Einblenden ohne Weg; unter reduzierter
    // Bewegung steht es sofort da (`docs/UX.md` 2.2, 2.10).
    if (_fade.isDismissed) {
      if (HReducedMotion.of(context)) {
        _fade.value = 1;
      } else {
        unawaited(_fade.forward());
      }
    }
  }

  @override
  void dispose() {
    _curve.dispose();
    _fade.dispose();
    _scope.dispose();
    super.dispose();
  }

  Future<void> _pause() async {
    if (_sending) {
      return;
    }
    setState(() => _sending = true);
    try {
      await ref.read(interceptPauseProvider.notifier).pause();
    } on DaemonException catch (error) {
      ref.read(attentionNoticeProvider.notifier).show(error.diagnostic);
    }
    if (mounted) {
      widget.onClose();
    }
  }

  @override
  Widget build(BuildContext context) {
    final HTokens tokens = HTheme.of(context);
    final AppLocalizations l10n = context.l10n;
    return CallbackShortcuts(
      bindings: AppShortcut.bind<VoidCallback>(<AppShortcut, VoidCallback>{
        AppShortcut.closeLayer: widget.onClose,
      }),
      child: FocusScope(
        node: _scope,
        child: FadeTransition(
          opacity: _curve,
          child: HModal(
            key: const Key('intercept-pause-modal'),
            // Breit genug, dass beide Knöpfe auch auf Deutsch nebeneinander
            // stehen, wie bei der Rückfrage über viele Anfragen.
            width: 480,
            title: Text(l10n.shellInterceptPauseTitle),
            onDismiss: widget.onClose,
            scrimSemanticsLabel: l10n.shellInterceptPauseCancel,
            actions: <Widget>[
              HButton(
                key: const Key('intercept-pause-cancel'),
                variant: HButtonVariant.secondary,
                autofocus: true,
                onPressed: widget.onClose,
                child: Text(l10n.shellInterceptPauseCancel),
              ),
              SizedBox(width: tokens.spacing.x2),
              HButton(
                key: const Key('intercept-pause-confirm'),
                variant: HButtonVariant.danger,
                onPressed: _sending ? null : () => unawaited(_pause()),
                child: Text(l10n.shellInterceptPauseConfirm),
              ),
            ],
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  l10n.shellInterceptPauseBody,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg1),
                ),
                SizedBox(height: tokens.spacing.x2),
                Text(
                  l10n.shellInterceptPauseStays,
                  style: tokens.typography.ui13.tinted(tokens.colors.fg2),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
